use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// libssh2 的 LIBSSH2_ERROR_EAGAIN：非阻塞模式下表示"暂时不可读写，稍后重试"
const LIBSSH2_EAGAIN: i32 = -37;
/// 非阻塞读的轮询间隔，兼顾输入回显延迟与 CPU 占用
const IO_POLL_INTERVAL: Duration = Duration::from_millis(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PTY_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// SFTP 会话走阻塞模式，超时兜底避免服务器无响应时线程永久挂起
const SFTP_IO_TIMEOUT_MS: u32 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConnectConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    pub cols: u32,
    pub rows: u32,
}

pub struct ActiveSession {
    /// Shell 会话：握手完成后切非阻塞，避免读阻塞锁死写入。
    /// 同时持有该 Arc 以保证底层连接不会被提前释放。
    #[allow(dead_code)]
    session: Arc<Mutex<ssh2::Session>>,
    channel: Arc<Mutex<ssh2::Channel>>,
    /// SFTP 专用会话（首次使用 SFTP 时懒加载的独立连接）
    sftp_session: Arc<Mutex<Option<Arc<Mutex<ssh2::Session>>>>>,
    config: SshConnectConfig,
    is_alive: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct SshManager {
    sessions: Arc<Mutex<HashMap<String, Arc<ActiveSession>>>>,
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
        let sess = connect_and_auth(&config)?;

        // 关键：切非阻塞后再申请 PTY/Shell，这样 read/write 都只做短暂持锁，
        // 不会出现"读线程持锁阻塞、写线程永远拿不到锁"的死锁。
        sess.set_blocking(false);

        let mut channel = wait_retry(|| sess.channel_session(), PTY_WAIT_TIMEOUT)
            .map_err(|e| format!("打开 Shell 通道失败: {}", e))?;

        let cols = config.cols.max(1);
        let rows = config.rows.max(1);
        wait_retry(
            || channel.request_pty("xterm-256color", None, Some((cols, rows, 0, 0))),
            PTY_WAIT_TIMEOUT,
        )
        .map_err(|e| format!("申请 PTY 失败: {}", e))?;
        wait_retry(|| channel.shell(), PTY_WAIT_TIMEOUT)
            .map_err(|e| format!("启动 Shell 失败: {}", e))?;

        let session_id = format!("ssh-{}", Uuid::new_v4());
        let channel_arc = Arc::new(Mutex::new(channel));
        let is_alive = Arc::new(AtomicBool::new(true));

        // 后台读取远端输出并广播给前端（非阻塞读 + 短轮询）
        let channel_reader = Arc::clone(&channel_arc);
        let is_alive_reader = Arc::clone(&is_alive);
        let app = app_handle.clone();
        let stdout_event = format!("ssh-stdout-{}", session_id);
        let closed_event = format!("ssh-closed-{}", session_id);

        std::thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            let mut pending: Vec<u8> = Vec::new();
            let mut exit_reason: Option<&str> = None;

            while is_alive_reader.load(Ordering::Relaxed) {
                let read_result = {
                    let mut ch = channel_reader.lock();
                    match ch.read(&mut buf) {
                        Ok(n) => Ok((n, ch.eof())),
                        Err(e) => Err(e),
                    }
                };

                match read_result {
                    Ok((n, _)) if n > 0 => {
                        // 按 UTF-8 边界解码，避免多字节字符被分片截断出现乱码
                        let text = decode_stream(&mut pending, &buf[..n]);
                        if !text.is_empty() {
                            let _ = app.emit(&stdout_event, text);
                        }
                    }
                    Ok((_, eof)) => {
                        if eof {
                            exit_reason = Some("远端已关闭 Shell 会话");
                            break;
                        }
                        std::thread::sleep(IO_POLL_INTERVAL);
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(IO_POLL_INTERVAL);
                    }
                    Err(_) => {
                        exit_reason = Some("SSH 连接已中断");
                        break;
                    }
                }
            }

            if !pending.is_empty() {
                let _ = app.emit(&stdout_event, String::from_utf8_lossy(&pending).to_string());
            }
            if let Some(reason) = exit_reason {
                is_alive_reader.store(false, Ordering::Relaxed);
                let _ = app.emit(&closed_event, reason.to_string());
            }
        });

        let active = Arc::new(ActiveSession {
            session: Arc::new(Mutex::new(sess)),
            channel: channel_arc,
            sftp_session: Arc::new(Mutex::new(None)),
            config,
            is_alive,
        });
        self.sessions.lock().insert(session_id.clone(), active);
        Ok(session_id)
    }

    fn get_active(&self, session_id: &str) -> Result<Arc<ActiveSession>, String> {
        self.sessions
            .lock()
            .get(session_id)
            .map(Arc::clone)
            .ok_or_else(|| "会话不存在或已关闭".to_string())
    }

    /// 非阻塞写入：写不进（窗口满/远端未消费）时短暂退让重试，并保证不长时间持锁
    pub fn write_session(&self, session_id: &str, data: &str) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let bytes = data.as_bytes();
        let mut written = 0usize;
        let start = Instant::now();

        while written < bytes.len() {
            let step = {
                let mut ch = active.channel.lock();
                ch.write(&bytes[written..])
            };
            match step {
                Ok(0) => {}
                Ok(n) => {
                    written += n;
                    continue;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == ErrorKind::BrokenPipe => {
                    return Err("会话已断开，无法写入".into())
                }
                Err(e) => return Err(format!("写入失败: {}", e)),
            }

            if start.elapsed() > WRITE_TIMEOUT {
                return Err("写入超时（远端未消费输入）".into());
            }
            std::thread::sleep(IO_POLL_INTERVAL);
        }
        Ok(())
    }

    pub fn resize_session(&self, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
        let active = self.get_active(session_id)?;
        let mut ch = active.channel.lock();
        // 窗口尺寸属于尽力而为的调整，失败不影响终端可用性；
        // 超时取值较短，避免拖拽窗口时长时间占用通道锁卡住回显
        let _ = wait_retry(
            || ch.request_pty_size(cols.max(1), rows.max(1), None, None),
            Duration::from_millis(300),
        );
        Ok(())
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        let active = self.sessions.lock().remove(session_id);
        if let Some(active) = active {
            active.is_alive.store(false, Ordering::Relaxed);
            // 尽力关闭通道；SFTP 会话随 Arc 释放（进行中的传输结束后自动断开）
            if let Some(mut ch) = active.channel.try_lock() {
                let _ = wait_retry(|| ch.close(), Duration::from_secs(2));
            }
        }
        Ok(())
    }

    /// 获取（必要时懒加载）该会话专属的 SFTP 连接，与终端 Shell 互不干扰
    pub fn get_sftp_session(&self, session_id: &str) -> Result<Arc<Mutex<ssh2::Session>>, String> {
        let active = self.get_active(session_id)?;

        // 快路径：已建立则直接复用
        if let Some(existing) = active.sftp_session.lock().as_ref() {
            return Ok(Arc::clone(existing));
        }

        // 建连期间不持有会话表锁，避免阻塞其他会话操作
        let config = active.config.clone();
        let sess = connect_and_auth(&config)?;
        sess.set_timeout(SFTP_IO_TIMEOUT_MS);
        let created = Arc::new(Mutex::new(sess));

        let mut slot = active.sftp_session.lock();
        if let Some(existing) = slot.as_ref() {
            // 并发进入时复用先建立好的连接
            return Ok(Arc::clone(existing));
        }
        *slot = Some(Arc::clone(&created));
        Ok(created)
    }
}

/// 建立 TCP 连接、完成握手与认证（此阶段保持阻塞模式，语义简单且带超时）
fn connect_and_auth(config: &SshConnectConfig) -> Result<ssh2::Session, String> {
    let addr = resolve_addr(&config.host, config.port)?;
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .map_err(|e| format!("TCP 连接超时/失败: {}", e))?;
    // 关闭 Nagle，降低交互式输入的往返延迟
    let _ = stream.set_nodelay(true);

    let mut sess = ssh2::Session::new().map_err(|e| e.to_string())?;
    sess.set_tcp_stream(stream);
    sess.handshake().map_err(|e| format!("SSH 握手失败: {}", e))?;

    authenticate(&sess, config)?;

    if !sess.authenticated() {
        return Err("身份认证未通过，请检查账号密码或密钥".into());
    }
    Ok(sess)
}

fn authenticate(sess: &ssh2::Session, config: &SshConnectConfig) -> Result<(), String> {
    match config.auth_type.as_str() {
        "password" => sess
            .userauth_password(&config.username, config.password.as_deref().unwrap_or(""))
            .map_err(|e| format!("密码认证失败: {}", e)),
        "key" => {
            let key_input = config.private_key.as_deref().unwrap_or("").trim();
            if key_input.is_empty() {
                return Err("未配置私钥内容或文件路径".into());
            }

            // 私钥既可能是文件路径，也可能是粘贴的密钥内容
            let key_is_file = Path::new(key_input).is_file();
            let key_path = if key_is_file {
                PathBuf::from(key_input)
            } else {
                let dir = std::env::temp_dir().join("snail-ssh-keys");
                std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {}", e))?;
                let path = dir.join(format!("id_{}", Uuid::new_v4()));
                std::fs::write(&path, key_input.as_bytes())
                    .map_err(|e| format!("写入临时私钥失败: {}", e))?;
                path
            };

            let result = sess.userauth_pubkey_file(
                &config.username,
                None,
                &key_path,
                config.passphrase.as_deref(),
            );
            if !key_is_file {
                let _ = std::fs::remove_file(&key_path);
            }
            result.map_err(|e| format!("私钥认证失败: {}", e))
        }
        "agent" => sess
            .userauth_agent(&config.username)
            .map_err(|e| format!("SSH Agent 认证失败: {}", e)),
        other => Err(format!("不支持的认证方式: {}", other)),
    }
}

fn resolve_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("无法解析主机地址 {}: {}", host, e))?
        .next()
        .ok_or_else(|| format!("主机 {} 无可用地址", host))
}

/// 重试 libssh2 调用直到成功或超时（非阻塞模式下 EAGAIN 属于正常态，需要重试）
fn wait_retry<T, F>(mut f: F, timeout: Duration) -> Result<T, ssh2::Error>
where
    F: FnMut() -> Result<T, ssh2::Error>,
{
    let start = Instant::now();
    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                let retryable = matches!(e.code(), ssh2::ErrorCode::Session(code) if code == LIBSSH2_EAGAIN);
                if !retryable || start.elapsed() > timeout {
                    return Err(e);
                }
                std::thread::sleep(IO_POLL_INTERVAL);
            }
        }
    }
}

/// 增量 UTF-8 解码：只输出完整字符，被切断的多字节序列留待下一批数据
fn decode_stream(pending: &mut Vec<u8>, chunk: &[u8]) -> String {
    pending.extend_from_slice(chunk);

    // 兜底：异常数据持续堆积时直接宽松解码，避免 pending 无上限增长
    if pending.len() > 64 * 1024 {
        let out = String::from_utf8_lossy(pending).to_string();
        pending.clear();
        return out;
    }

    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(text) => {
                out.push_str(text);
                pending.clear();
                break;
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    out.push_str(std::str::from_utf8(&pending[..valid_up_to]).unwrap_or_default());
                    pending.drain(..valid_up_to);
                }
                match e.error_len() {
                    // 尾部是被截断的多字节字符，等待下一批数据
                    None => break,
                    Some(len) => {
                        out.push('\u{FFFD}');
                        pending.drain(..len);
                    }
                }
            }
        }
    }
    out
}
