//! 端口转发的运行时状态与字节桥。
//!
//! 本地 / 动态转发在本机监听，再经已认证的 SSH 连接开 direct-tcpip。
//! 远端转发由服务端监听，连入时 Handler 查这张表，决定接到本机哪个地址。
//! 停止只发一个 watch 信号：监听循环和桥接都看这个标志退出，避免 JoinHandle 和连接互相持有。

use crate::model::PortForward;
use parking_lot::Mutex;
use russh::ChannelMsg;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;

#[derive(Clone)]
pub struct RemoteTarget {
    pub listen_host: String,
    pub target_host: String,
    pub target_port: u16,
}

pub struct ForwardControl {
    stop_tx: watch::Sender<bool>,
    remotes: Mutex<HashMap<u32, RemoteTarget>>,
}

impl ForwardControl {
    pub fn new() -> Arc<Self> {
        let (stop_tx, _) = watch::channel(false);
        Arc::new(Self {
            stop_tx,
            remotes: Mutex::new(HashMap::new()),
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.stop_tx.subscribe()
    }

    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }

    pub fn register_remote(&self, port: u32, target: RemoteTarget) {
        self.remotes.lock().insert(port, target);
    }

    pub fn lookup_remote(&self, connected_port: u32) -> Option<RemoteTarget> {
        self.remotes.lock().get(&connected_port).cloned()
    }

    pub fn remote_binds(&self) -> Vec<(String, u32)> {
        self.remotes
            .lock()
            .iter()
            .map(|(port, target)| (target.listen_host.clone(), *port))
            .collect()
    }
}

pub fn validate_forward(spec: &PortForward) -> Result<(), String> {
    if spec.bind_port == 0 {
        return Err("转发端口不能为 0".into());
    }
    if spec.bind_host.trim().is_empty() {
        return Err("监听地址不能为空".into());
    }
    match spec.kind.as_str() {
        "local" | "remote" => {
            if spec.target_host.trim().is_empty() || spec.target_port == 0 {
                return Err("本地/远端转发需要填写目标主机和端口".into());
            }
            Ok(())
        }
        "dynamic" => Ok(()),
        other => Err(format!("未知转发类型: {}", other)),
    }
}

/// 把 SSH 通道和一条 TCP 连接对拷，直到任一侧关闭。
pub async fn bridge_channel_tcp(
    mut channel: russh::Channel<russh::client::Msg>,
    mut tcp: TcpStream,
) {
    let mut buf = vec![0u8; 32 * 1024];
    let mut tcp_closed = false;
    loop {
        tokio::select! {
            result = tcp.read(&mut buf), if !tcp_closed => {
                match result {
                    Ok(0) => {
                        tcp_closed = true;
                        let _ = channel.eof().await;
                    }
                    Ok(n) => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if tcp.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if tcp.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                }
            }
        }
    }
    let _ = tcp.shutdown().await;
    let _ = channel.close().await;
}

/// 读一条 SOCKS5 无认证请求，返回目标主机和端口。失败时调用方应关闭连接。
pub async fn read_socks5_request(stream: &mut TcpStream) -> Result<(String, u16), String> {
    let mut hello = [0u8; 2];
    stream.read_exact(&mut hello).await.map_err(|e| e.to_string())?;
    if hello[0] != 0x05 {
        return Err("不是 SOCKS5 请求".into());
    }
    let methods = hello[1] as usize;
    let mut skip = vec![0u8; methods];
    stream.read_exact(&mut skip).await.map_err(|e| e.to_string())?;
    // 动态转发只接受无认证，避免把本机 SOCKS 口变成开放代理的口令入口
    stream.write_all(&[0x05, 0x00]).await.map_err(|e| e.to_string())?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await.map_err(|e| e.to_string())?;
    if head[1] != 0x01 {
        return Err("只支持 SOCKS5 CONNECT".into());
    }
    let host = match head[3] {
        0x01 => {
            let mut ip = [0u8; 4];
            stream.read_exact(&mut ip).await.map_err(|e| e.to_string())?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        0x04 => {
            let mut ip = [0u8; 16];
            stream.read_exact(&mut ip).await.map_err(|e| e.to_string())?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await.map_err(|e| e.to_string())?;
            let mut name = vec![0u8; len[0] as usize];
            stream.read_exact(&mut name).await.map_err(|e| e.to_string())?;
            String::from_utf8(name).map_err(|_| "SOCKS5 主机名不是 UTF-8".to_string())?
        }
        other => return Err(format!("不支持的 SOCKS5 地址类型: {}", other)),
    };
    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await.map_err(|e| e.to_string())?;
    Ok((host, u16::from_be_bytes(port_buf)))
}

pub async fn write_socks5_reply(stream: &mut TcpStream, ok: bool) -> Result<(), String> {
    let status = if ok { 0x00 } else { 0x05 };
    // 回复绑定地址用 0.0.0.0:0，客户端只看状态码
    stream
        .write_all(&[0x05, status, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| e.to_string())
}

/// 本进程内避免两个会话抢同一个本地监听端口。
static LOCAL_BINDS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn reserve_local_bind(addr: &str, owner: &str) -> Result<(), String> {
    let mut map = LOCAL_BINDS.lock();
    if let Some(existing) = map.get(addr) {
        if existing != owner {
            return Err(format!("{} 已被另一个隧道占用", addr));
        }
    }
    map.insert(addr.to_string(), owner.to_string());
    Ok(())
}

pub fn release_bind(addr: &str) {
    LOCAL_BINDS.lock().remove(addr);
}

pub fn release_owner(owner: &str) {
    LOCAL_BINDS.lock().retain(|_, held_by| held_by != owner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_missing_target_for_local() {
        let spec = PortForward {
            id: "1".into(),
            kind: "local".into(),
            bind_host: "127.0.0.1".into(),
            bind_port: 8080,
            target_host: String::new(),
            target_port: 0,
            enabled: true,
        };
        assert!(validate_forward(&spec).is_err());
    }

    #[test]
    fn validate_accepts_dynamic_without_target() {
        let spec = PortForward {
            id: "1".into(),
            kind: "dynamic".into(),
            bind_host: "127.0.0.1".into(),
            bind_port: 1080,
            target_host: String::new(),
            target_port: 0,
            enabled: true,
        };
        assert!(validate_forward(&spec).is_ok());
    }
}
