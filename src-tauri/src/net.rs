//! 第一跳 TCP：直连，或经 SOCKS5 / HTTP CONNECT 代理。
//! 跳板机之后的每一跳走 SSH direct-tcpip，不再经过这里。

use crate::model::ProxyConfig;
use base64::Engine as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn dial(host: &str, port: u16, proxy: Option<&ProxyConfig>) -> Result<TcpStream, String> {
    let Some(proxy) = proxy.filter(|p| p.is_active()) else {
        return timeout(DIAL_TIMEOUT, TcpStream::connect((host, port)))
            .await
            .map_err(|_| "TCP 连接超时（10s）".to_string())?
            .map_err(|e| format!("TCP 连接失败: {}", e))
            .and_then(nodelay);
    };

    let mut stream = timeout(DIAL_TIMEOUT, TcpStream::connect((proxy.host.as_str(), proxy.port)))
        .await
        .map_err(|_| format!("代理 {}:{} 连接超时（10s）", proxy.host, proxy.port))?
        .map_err(|e| format!("代理 {}:{} 连接失败: {}", proxy.host, proxy.port, e))?;
    stream = nodelay(stream)?;

    match proxy.kind.to_ascii_lowercase().as_str() {
        "socks5" | "socks" => socks5_connect(&mut stream, host, port, proxy).await?,
        "http" | "https" => http_connect(&mut stream, host, port, proxy).await?,
        other => return Err(format!("不支持的代理类型: {}", other)),
    }
    Ok(stream)
}

fn nodelay(stream: TcpStream) -> Result<TcpStream, String> {
    stream
        .set_nodelay(true)
        .map_err(|e| format!("设置 TCP_NODELAY 失败: {}", e))?;
    Ok(stream)
}

async fn socks5_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &ProxyConfig,
) -> Result<(), String> {
    let user = proxy.username.as_deref().unwrap_or("").trim();
    let pass = proxy.password.as_deref().unwrap_or("");
    let use_auth = !user.is_empty();
    if use_auth {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await.map_err(io_err)?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00]).await.map_err(io_err)?;
    }
    let mut greeting = [0u8; 2];
    stream.read_exact(&mut greeting).await.map_err(io_err)?;
    if greeting[0] != 0x05 {
        return Err("SOCKS5 代理返回了无法识别的握手".into());
    }
    match greeting[1] {
        0x00 => {}
        0x02 => {
            if !use_auth {
                return Err("SOCKS5 代理要求用户名密码，但未填写".into());
            }
            socks5_userpass(stream, user, pass).await?;
        }
        0xFF => return Err("SOCKS5 代理拒绝了可用的认证方式".into()),
        other => return Err(format!("SOCKS5 代理要求不支持的认证方式: {}", other)),
    }

    let mut req = vec![0x05, 0x01, 0x00];
    append_socks_addr(&mut req, host, port)?;
    stream.write_all(&req).await.map_err(io_err)?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await.map_err(io_err)?;
    if head[1] != 0x00 {
        return Err(format!("SOCKS5 连接 {}:{} 失败（状态 {}）", host, port, head[1]));
    }
    discard_socks_addr(stream, head[3]).await?;
    Ok(())
}

async fn socks5_userpass(stream: &mut TcpStream, user: &str, pass: &str) -> Result<(), String> {
    if user.len() > 255 || pass.len() > 255 {
        return Err("SOCKS5 用户名或密码过长".into());
    }
    let mut pkt = vec![0x01, user.len() as u8];
    pkt.extend_from_slice(user.as_bytes());
    pkt.push(pass.len() as u8);
    pkt.extend_from_slice(pass.as_bytes());
    stream.write_all(&pkt).await.map_err(io_err)?;
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await.map_err(io_err)?;
    if reply[1] != 0x00 {
        return Err("SOCKS5 代理拒绝了用户名或密码".into());
    }
    Ok(())
}

fn append_socks_addr(buf: &mut Vec<u8>, host: &str, port: u16) -> Result<(), String> {
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        buf.push(0x01);
        buf.extend_from_slice(&ip.octets());
    } else if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        buf.push(0x04);
        buf.extend_from_slice(&ip.octets());
    } else {
        let bytes = host.as_bytes();
        if bytes.len() > 255 {
            return Err("目标主机名过长，无法通过 SOCKS5 连接".into());
        }
        buf.push(0x03);
        buf.push(bytes.len() as u8);
        buf.extend_from_slice(bytes);
    }
    buf.extend_from_slice(&port.to_be_bytes());
    Ok(())
}

async fn discard_socks_addr(stream: &mut TcpStream, atyp: u8) -> Result<(), String> {
    let n = match atyp {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await.map_err(io_err)?;
            len[0] as usize
        }
        other => return Err(format!("SOCKS5 返回了未知地址类型: {}", other)),
    };
    let mut skip = vec![0u8; n + 2];
    stream.read_exact(&mut skip).await.map_err(io_err)?;
    Ok(())
}

async fn http_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &ProxyConfig,
) -> Result<(), String> {
    let target = format!("{}:{}", host, port);
    let mut req = format!(
        "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Connection: keep-alive\r\n"
    );
    if let Some(user) = proxy.username.as_deref().filter(|u| !u.is_empty()) {
        let pass = proxy.password.as_deref().unwrap_or("");
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        req.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).await.map_err(io_err)?;

    let mut buf = Vec::new();
    let mut tmp = [0u8; 256];
    loop {
        let n = stream.read(&mut tmp).await.map_err(io_err)?;
        if n == 0 {
            return Err("HTTP 代理在返回状态前关闭了连接".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 8192 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let status = text.lines().next().unwrap_or("");
    if !(status.contains(" 200 ") || status.ends_with(" 200")) {
        return Err(format!("HTTP 代理拒绝 CONNECT: {}", status.trim()));
    }
    Ok(())
}

fn io_err(err: std::io::Error) -> String {
    format!("代理握手失败: {}", err)
}
