use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConnectConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub cols: u32,
    pub rows: u32,
}

pub struct ActiveSession {
    pub session: Arc<Mutex<ssh2::Session>>,
    pub channel: Arc<Mutex<ssh2::Channel>>,
    pub is_alive: Arc<AtomicBool>,
}

pub struct SshManager {
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(
        &self,
        config: SshConnectConfig,
        app_handle: AppHandle,
    ) -> Result<String, String> {
        let addr = format!("{}:{}", config.host, config.port);
        let stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| format!("无效主机地址: {}", e))?,
            Duration::from_secs(10),
        )
        .map_err(|e| format!("TCP 连接超时/失败: {}", e))?;

        let mut sess = ssh2::Session::new().map_err(|e| e.to_string())?;
        sess.set_tcp_stream(stream);
        sess.handshake().map_err(|e| format!("SSH 握手失败: {}", e))?;

        // 认证判断
        if config.auth_type == "password" {
            let pwd = config.password.unwrap_or_default();
            sess.userauth_password(&config.username, &pwd)
                .map_err(|e| format!("密码认证失败: {}", e))?;
        } else if config.auth_type == "key" {
            let key_input = config.private_key.unwrap_or_default();
            let key_path = if Path::new(&key_input).exists() {
                PathBuf::from(key_input)
            } else {
                // 如果是私钥内容，写入临时文件进行认证
                let temp_dir = std::env::temp_dir().join(".snail_keys");
                let _ = std::fs::create_dir_all(&temp_dir);
                let tmp_key_file = temp_dir.join(format!("id_{}", Uuid::new_v4()));
                std::fs::write(&tmp_key_file, key_input.as_bytes())
                    .map_err(|e| format!("写入临时私钥失败: {}", e))?;
                tmp_key_file
            };

            let auth_res = sess.userauth_pubkey_file(&config.username, None, &key_path, None);
            // 认证后如果是在 temp 目录生成的临时文件，立即安全删除
            if key_path.starts_with(std::env::temp_dir()) {
                let _ = std::fs::remove_file(&key_path);
            }
            auth_res.map_err(|e| format!("私钥认证失败: {}", e))?;
        } else {
            sess.userauth_agent(&config.username)
                .map_err(|e| format!("SSH Agent 认证失败: {}", e))?;
        }

        if !sess.authenticated() {
            return Err("身份认证未通过，请检查账号密码或密钥".into());
        }

        // 打开交互式 Shell 通道
        let mut channel = sess.channel_session().map_err(|e| e.to_string())?;
        channel
            .request_pty(
                "xterm-256color",
                None,
                Some((config.cols, config.rows, 0, 0)),
            )
            .map_err(|e| format!("申请 PTY 失败: {}", e))?;
        channel.shell().map_err(|e| format!("启动 Shell 失败: {}", e))?;

        let session_id = format!("ssh-{}", Uuid::new_v4());
        let session_arc = Arc::new(Mutex::new(sess));
        let channel_arc = Arc::new(Mutex::new(channel));
        let is_alive = Arc::new(AtomicBool::new(true));

        // 后台持续读取 stdout/stderr 并向前端广播
        let channel_reader = Arc::clone(&channel_arc);
        let is_alive_reader = Arc::clone(&is_alive);
        let app = app_handle.clone();
        let event_name = format!("ssh-stdout-{}", session_id);

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while is_alive_reader.load(Ordering::Relaxed) {
                let res = {
                    let mut ch = channel_reader.lock();
                    ch.read(&mut buf)
                };

                match res {
                    Ok(n) if n > 0 => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = app.emit(&event_name, text);
                    }
                    Ok(_) => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        let active = ActiveSession {
            session: session_arc,
            channel: channel_arc,
            is_alive,
        };

        self.sessions.lock().insert(session_id.clone(), active);
        Ok(session_id)
    }

    pub fn write_session(&self, session_id: &str, data: &str) -> Result<(), String> {
        let guard = self.sessions.lock();
        if let Some(active) = guard.get(session_id) {
            let mut ch = active.channel.lock();
            ch.write_all(data.as_bytes())
                .map_err(|e| format!("写入失败: {}", e))?;
            ch.flush().map_err(|e| format!("刷新失败: {}", e))?;
            Ok(())
        } else {
            Err("会话不存在或已关闭".into())
        }
    }

    pub fn resize_session(&self, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
        let guard = self.sessions.lock();
        if let Some(active) = guard.get(session_id) {
            let mut ch = active.channel.lock();
            let _ = ch.request_pty_size(cols, rows, None, None);
            Ok(())
        } else {
            Err("会话不存在".into())
        }
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        let mut guard = self.sessions.lock();
        if let Some(active) = guard.remove(session_id) {
            active.is_alive.store(false, Ordering::Relaxed);
            let mut ch = active.channel.lock();
            let _ = ch.close();
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn get_raw_session(&self, session_id: &str) -> Option<Arc<Mutex<ssh2::Session>>> {
        let guard = self.sessions.lock();
        guard.get(session_id).map(|s| Arc::clone(&s.session))
    }
}
