use base64::Engine as _;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ssh2::{CheckResult, KnownHostFileKind};
use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
/// 工作目录探测的整体超时，避免远端命令挂住时阻塞自动跟随
const CWD_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Shell 启动后的首屏输出（Last login / MOTD / 首个提示符）抓取窗口。
/// 这几行由远端在 shell 刚起来时立刻发出，而前端要等 Vue 渲染完才注册 stdout 监听，
/// 若仍走事件广播会在注册前被丢弃（表现为看不到欢迎信息），因此建会话时先同步扣下。
/// 已经读到数据后，静默这么久即认为首屏结束；始终没有数据时只等这么久。
const BANNER_QUIET_GAP: Duration = Duration::from_millis(60);
const BANNER_FIRST_BYTE_TIMEOUT: Duration = Duration::from_millis(400);
/// 首屏抓取的总上限，避免远端持续刷输出导致建连迟迟不返回
const BANNER_MAX_WAIT: Duration = Duration::from_millis(800);

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
    /// 是否信任首次见到的主机密钥（TOFU）。为 false 时未知密钥会拒绝连接，
    /// 由前端确认指纹后再带 true 重试。
    #[serde(default)]
    pub trust_host_key: bool,
    pub cols: u32,
    pub rows: u32,
}

/// 主机密钥校验结果，随连接/测试结果返回给前端展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostKeyInfo {
    /// SHA256 base64 指纹，形如 SHA256:xxxx
    pub fingerprint: String,
    /// matched（与 known_hosts 一致）/ trusted（本次新信任并写入）/ mismatch
    pub status: String,
}

/// 连通性测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    #[serde(rename = "latencyMs")]
    pub latency_ms: u64,
    #[serde(rename = "hostKeyFingerprint")]
    pub host_key_fingerprint: String,
    #[serde(rename = "hostKeyStatus")]
    pub host_key_status: String,
}

/// 远端主机实时指标采集结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostProbeResult {
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default, rename = "osName")]
    pub os_name: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    #[serde(default)]
    pub mem: Option<u32>,
    #[serde(default)]
    pub disk: Option<u32>,
    #[serde(default, rename = "pingMs")]
    pub ping_ms: Option<u64>,
}

/// 远端服务器实时系统监控指标（MobaXterm 风格底部监控条）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteSystemMetrics {
    #[serde(default)]
    pub hostname: String,
    #[serde(default, rename = "osName")]
    pub os_name: Option<String>,
    #[serde(default)]
    pub uptime: String,
    #[serde(default)]
    pub users: Vec<String>,
    #[serde(default)]
    pub cpu: u32,
    #[serde(default, rename = "memUsedBytes")]
    pub mem_used_bytes: u64,
    #[serde(default, rename = "memTotalBytes")]
    pub mem_total_bytes: u64,
    #[serde(default, rename = "memUsagePercent")]
    pub mem_usage_percent: u32,
    #[serde(default, rename = "netUploadSpeed")]
    pub net_upload_speed: f64,
    #[serde(default, rename = "netDownloadSpeed")]
    pub net_download_speed: f64,
    #[serde(default)]
    pub disks: Vec<RemoteDiskUsage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteDiskUsage {
    #[serde(default, rename = "mountPoint")]
    pub mount_point: String,
    #[serde(default, rename = "usePercent")]
    pub use_percent: u32,
    #[serde(default)]
    pub total: String,
    #[serde(default)]
    pub used: String,
}

#[derive(Clone, Debug)]
struct RemoteMetricsSnapshot {
    sampled_at: Instant,
    cpu_fields: Vec<u64>,
    net_rx_bytes: u64,
    net_tx_bytes: u64,
}

pub struct ActiveSession {
    /// Shell 会话：握手完成后切非阻塞，避免读阻塞锁死写入。
    /// 同时持有该 Arc 以保证底层连接不会被提前释放。
    /// 工作目录探测也在它上面开瞬态通道：必须与本会话的 shell 同属一条连接，
    /// 才能按“同父兄弟进程”精确认出本会话，而不是靠 pts 活跃度去猜。
    session: Arc<Mutex<ssh2::Session>>,
    channel: Arc<Mutex<ssh2::Channel>>,
    /// SFTP 专用会话（首次使用 SFTP 时懒加载的独立连接）
    sftp_session: Arc<Mutex<Option<Arc<Mutex<ssh2::Session>>>>>,
    config: SshConnectConfig,
    is_alive: Arc<AtomicBool>,
    /// 建连时扣下的首屏输出，等前端注册好监听后由 take_initial_output 取走（只取一次）
    banner: Arc<Mutex<String>>,
    /// 会话日志槽：开着时读线程把脱敏后的输出 try_send 给专用 writer 线程
    log_sink: Arc<Mutex<Option<LogSink>>>,
}

/// 会话日志的写入句柄：路径用于前端展示，tx 交给 writer 线程落盘。
/// 直接 drop 即关闭 channel，writer 线程随之 flush 退出。
struct LogSink {
    path: PathBuf,
    tx: SyncSender<String>,
}

/// 会话日志缓冲上限：写线程跟不上时丢弃而不是阻塞终端读线程
const LOG_CHANNEL_CAPACITY: usize = 256;

/// 落盘前脱敏的敏感键（小写匹配，要求词边界）
const SECRET_KEYS: [&str; 9] = [
    "password",
    "passphrase",
    "passwd",
    "pwd",
    "token",
    "secret",
    "api_key",
    "apikey",
    "access_key",
];

/// 把文件名中的非法字符替换掉（Windows 限制较严）
fn sanitize_host(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if cleaned.trim().is_empty() {
        "unknown-host".to_string()
    } else {
        cleaned.chars().take(80).collect()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unix 时间戳 → UTC 的 `YYYYMMDD_HHMMSS`（不引入日期库）
fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        year,
        month,
        day,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 civil-from-days 算法
fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 找到下一个敏感键的 (键起始, 值起始)；值起始越界或未构成键值对时跳过。
/// 全部为线性扫描、长度有界，避免在读线程里触发正则回溯。
fn find_next_secret(text: &str) -> Option<(usize, usize)> {
    let mut best = find_next_key_secret(text);
    if let Some(flag) = find_password_flag(text) {
        if best.map_or(true, |(b, _)| flag.0 < b) {
            best = Some(flag);
        }
    }
    best
}

fn find_next_key_secret(text: &str) -> Option<(usize, usize)> {
    let lower = text.to_lowercase();
    let bytes = text.as_bytes();
    let mut best: Option<(usize, usize)> = None;
    for key in SECRET_KEYS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(key) {
            let abs = from + pos;
            let boundary_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
            if boundary_ok {
                if let Some(value_start) = separator_value_start(text, abs + key.len()) {
                    if best.map_or(true, |(b, _)| abs < b) {
                        best = Some((abs, value_start));
                    }
                    break;
                }
            }
            from = abs + key.len();
            if from >= lower.len() {
                break;
            }
        }
    }
    best
}

/// `-p<secret>` / `-p <secret>`：值为纯数字时视为端口号（`-p22`）不脱敏
fn find_password_flag(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'p' {
            // 前缀也是 `-` 说明是 `--pretty` 这类长选项，跳过
            let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-');
            if prev_ok {
                let mut value_start = i + 2;
                while value_start < bytes.len()
                    && (bytes[value_start] == b' ' || bytes[value_start] == b'\t')
                {
                    value_start += 1;
                }
                if value_start < bytes.len() {
                    if bytes[value_start] == b'"' || bytes[value_start] == b'\'' {
                        value_start += 1;
                    }
                    if value_start < bytes.len() {
                        let end = value_start + value_end(&text[value_start..]);
                        let value = &text[value_start..end];
                        if !value.chars().all(|c| c.is_ascii_digit()) {
                            return Some((i, value_start));
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// 键后面必须是 `=` / `:` / 空白分隔，否则视为普通单词（如 passwordless）
fn separator_value_start(text: &str, key_end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = key_end;
    if i >= bytes.len() {
        return None;
    }
    if bytes[i].is_ascii_alphanumeric() {
        return None;
    }
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i < bytes.len() && (bytes[i] == b'=' || bytes[i] == b':') {
        i += 1;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
    }
    if i >= bytes.len() {
        return None;
    }
    // 跳过成对的引号
    if bytes[i] == b'"' || bytes[i] == b'\'' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    Some(i)
}

/// 值到下一个空白/分隔符为止
fn value_end(value: &str) -> usize {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' || b == b';' || b == b',' || b == b'&' {
            break;
        }
        i += 1;
    }
    i.max(1)
}

/// 把 password/token/secret 之类的值替换为 `***`
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        match find_next_secret(rest) {
            Some((_, value_start)) => {
                out.push_str(&rest[..value_start]);
                let end = value_start + value_end(&rest[value_start..]);
                out.push_str("***");
                rest = &rest[end..];
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// 读线程取当前日志 sender（不持锁 send）
fn current_log_tx(sink: &Arc<Mutex<Option<LogSink>>>) -> Option<SyncSender<String>> {
    sink.lock().as_ref().map(|s| s.tx.clone())
}

/// 读线程落盘入口：二进制输出跳过，缓冲满/断开时丢弃，绝不阻塞终端。
fn push_log(
    tx: &SyncSender<String>,
    text: &str,
    backpressure_warned: &mut bool,
    binary_warned: &mut bool,
) {
    if text.contains('\0') {
        if !*binary_warned {
            *binary_warned = true;
            eprintln!("[snail-ssh] 会话日志跳过含二进制数据的输出片段");
        }
        return;
    }
    match tx.try_send(redact(text)) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            if !*backpressure_warned {
                *backpressure_warned = true;
                eprintln!("[snail-ssh] 会话日志写入跟不上输出，已丢弃部分内容");
            }
        }
        Err(TrySendError::Disconnected(_)) => {}
    }
}

#[derive(Clone)]
pub struct SshManager {
    sessions: Arc<Mutex<HashMap<String, Arc<ActiveSession>>>>,
    metrics_cache: Arc<Mutex<HashMap<String, RemoteMetricsSnapshot>>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            metrics_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(
        &self,
        config: SshConnectConfig,
        app_handle: AppHandle,
    ) -> Result<String, String> {
        let sess =
            connect_and_auth(&config).map(|(session, _host_key)| session)?;

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

        // 先同步扣下首屏（Last login / MOTD / 首个提示符）：此时前端还没注册监听，
        // 交给下面的读线程 emit 只会被丢掉。读取量极小且带超时，不影响建连耗时。
        let banner = read_initial_output(&mut channel).unwrap_or_default();

        let session_id = format!("ssh-{}", Uuid::new_v4());
        let channel_arc = Arc::new(Mutex::new(channel));
        let is_alive = Arc::new(AtomicBool::new(true));
        let log_sink: Arc<Mutex<Option<LogSink>>> = Arc::new(Mutex::new(None));

        // 后台读取远端输出并广播给前端（非阻塞读 + 短轮询）
        let channel_reader = Arc::clone(&channel_arc);
        let is_alive_reader = Arc::clone(&is_alive);
        let log_sink_reader = Arc::clone(&log_sink);
        let app = app_handle.clone();
        let stdout_event = format!("ssh-stdout-{}", session_id);
        let closed_event = format!("ssh-closed-{}", session_id);

        std::thread::spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            let mut pending: Vec<u8> = Vec::new();
            let mut exit_reason: Option<&str> = None;
            let mut log_backpressure_warned = false;
            let mut log_binary_warned = false;

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
                            // 先 emit（保持原行为与顺序不变），再异步落盘；
                            // 未开启日志时不 clone，避免无谓分配
                            match current_log_tx(&log_sink_reader) {
                                Some(tx) => {
                                    let _ = app.emit(&stdout_event, text.clone());
                                    push_log(&tx, &text, &mut log_backpressure_warned, &mut log_binary_warned);
                                }
                                None => {
                                    let _ = app.emit(&stdout_event, text);
                                }
                            }
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
                let tail = String::from_utf8_lossy(&pending).to_string();
                let _ = app.emit(&stdout_event, tail.clone());
                if let Some(tx) = current_log_tx(&log_sink_reader) {
                    push_log(&tx, &tail, &mut log_backpressure_warned, &mut log_binary_warned);
                }
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
            banner: Arc::new(Mutex::new(banner)),
            log_sink,
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
        let active = self.get_active(session_id)?;        if !active.is_alive.load(Ordering::Relaxed) {
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
        self.metrics_cache.lock().remove(session_id);
        let active = self.sessions.lock().remove(session_id);
        if let Some(active) = active {
            active.is_alive.store(false, Ordering::Relaxed);
            // 显式 drop 日志槽：关闭 channel 让 writer 线程退出并 flush，避免尾部内容丢失
            active.log_sink.lock().take();
            // 尽力关闭通道；SFTP 会话随 Arc 释放（进行中的传输结束后自动断开）
            if let Some(mut ch) = active.channel.try_lock() {
                let _ = wait_retry(|| ch.close(), Duration::from_secs(2));
            }
        }
        Ok(())
    }

    /// 开/关会话日志；开启返回日志文件路径，关闭返回被关闭文件的路径（原本未开启则为 None）。
    pub fn set_session_logging(
        &self,
        session_id: &str,
        enabled: bool,
        host_label: &str,
    ) -> Result<Option<String>, String> {
        let active = self.get_active(session_id)?;

        if !enabled {
            let removed = active.log_sink.lock().take();
            return Ok(removed.map(|sink| sink.path.to_string_lossy().to_string()));
        }

        // 已在记录则幂等返回原路径
        if let Some(existing) = active.log_sink.lock().as_ref() {
            return Ok(Some(existing.path.to_string_lossy().to_string()));
        }

        let dir = crate::storage::app_data_dir()
            .join("logs")
            .join(sanitize_host(host_label));
        fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {}", e))?;

        let short_id: String = session_id.trim_start_matches("ssh-").chars().take(8).collect();
        let started = now_secs();
        let path = dir.join(format!("{}_{}.log", format_timestamp(started), short_id));
        let file = fs::File::create(&path).map_err(|e| format!("创建日志文件失败: {}", e))?;

        let (tx, rx) = sync_channel::<String>(LOG_CHANNEL_CAPACITY);
        let header = format!(
            "# SnailSSH session log (credentials redacted): {} started {} UTC\n",
            host_label,
            format_timestamp(started)
        );
        std::thread::spawn(move || {
            let mut file = file;
            let _ = file.write_all(header.as_bytes());
            while let Ok(chunk) = rx.recv() {
                if file.write_all(chunk.as_bytes()).is_err() {
                    break;
                }
            }
            let _ = file.flush();
        });

        *active.log_sink.lock() = Some(LogSink { path: path.clone(), tx });
        Ok(Some(path.to_string_lossy().to_string()))
    }

    /// 查询当前日志路径（未开启返回 None）
    pub fn get_session_log_path(&self, session_id: &str) -> Result<Option<String>, String> {
        let active = self.get_active(session_id)?;
        let path = active
            .log_sink
            .lock()
            .as_ref()
            .map(|sink| sink.path.to_string_lossy().to_string());
        Ok(path)
    }

    /// 获取（必要时懒加载）该会话专属的 SFTP 连接，与终端 Shell 互不干扰
    pub fn get_sftp_session(&self, session_id: &str) -> Result<Arc<Mutex<ssh2::Session>>, String> {        let active = self.get_active(session_id)?;

        // 快路径：已建立则直接复用
        if let Some(existing) = active.sftp_session.lock().as_ref() {
            return Ok(Arc::clone(existing));
        }

        // 建连期间不持有会话表锁，避免阻塞其他会话操作
        let config = active.config.clone();
        let sess = connect_and_auth(&config).map(|(session, _host_key)| session)?;
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

    /// 供探测命令读取会话对应的连接配置（用于一次性建连采集主机指标）
    pub fn get_config(&self, session_id: &str) -> Result<SshConnectConfig, String> {
        Ok(self.get_active(session_id)?.config.clone())
    }

    /// 获取终端会话当前的工作目录（CWD）。
    ///
    /// 探测通道必须开在终端自己那条连接上：该连接下的 shell 通道与 exec 通道由
    /// 同一个远端 sshd 会话进程派生（互为兄弟进程），脚本据此把候选收敛到本会话的
    /// 登录 shell，不受其他会话、其他用户以及挂在 pts 上的常驻进程影响。
    pub fn get_session_cwd(&self, session_id: &str) -> Result<String, String> {
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let output = {
            let sess = active.session.lock();
            run_probe_command(&sess, CWD_DETECT_SCRIPT)?
        };

        let path = output
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or("/");

        if path.starts_with('/') {
            Ok(path.to_string())
        } else {
            Ok("/".to_string())
        }
    }

    /// 取走建连时扣下的首屏输出（Last login / MOTD / 首个提示符）。
    /// 只返回一次，重复调用得到空串，避免前端重复写入造成重影。
    pub fn take_initial_output(&self, session_id: &str) -> Result<String, String> {
        let active = self.get_active(session_id)?;
        let mut banner = active.banner.lock();
        Ok(std::mem::take(&mut *banner))
    }

    /// 在会话连接上执行一次性命令并读回输出。
    /// 供远端历史拉取、脚本执行、探测等复用；通道开在终端自己那条连接上。
    pub fn exec_command(&self, session_id: &str, command: &str) -> Result<String, String> {
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }
        let sess = active.session.lock();
        run_probe_command(&sess, command)
    }

    /// 采集远端主机实时系统指标（MobaXterm 风格底部监控条），复用终端连接瞬态 exec
    pub fn get_remote_metrics(&self, session_id: &str) -> Result<RemoteSystemMetrics, String> {
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let default_host = active.config.host.clone();
        let output = {
            let sess = active.session.lock();
            run_probe_command(&sess, REMOTE_METRICS_SCRIPT)?
        };

        let prev_snap = self.metrics_cache.lock().get(session_id).cloned();
        let (metrics, new_snap) = parse_remote_metrics(&output, prev_snap, &default_host);
        self.metrics_cache.lock().insert(session_id.to_string(), new_snap);

        Ok(metrics)
    }
}

/// 建连时同步扣下 shell 首屏输出。
/// 会话已切非阻塞，因此按 EAGAIN 轮询；读到数据后静默 `BANNER_QUIET_GAP` 即收工，
/// 始终没有数据（远端既不打印 Last login 也无 MOTD）则按 `BANNER_FIRST_BYTE_TIMEOUT` 收工。
fn read_initial_output(channel: &mut ssh2::Channel) -> Result<String, String> {
    let mut out = String::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    let start = Instant::now();
    let mut last_data: Option<Instant> = None;

    loop {
        match channel.read(&mut buf) {
            Ok(0) => {
                // libssh2 非阻塞模式下 0 字节可能是 EOF，也可能只是本次没读到
                if channel.eof() {
                    break;
                }
            }
            Ok(n) => {
                out.push_str(&decode_stream(&mut pending, &buf[..n]));
                last_data = Some(Instant::now());
                continue;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(format!("读取会话首屏输出失败: {}", e)),
        }

        let now = Instant::now();
        let waited = now.duration_since(start);
        let finished = match last_data {
            Some(t) => now.duration_since(t) >= BANNER_QUIET_GAP || waited >= BANNER_MAX_WAIT,
            None => waited >= BANNER_FIRST_BYTE_TIMEOUT,
        };
        if finished {
            break;
        }
        std::thread::sleep(IO_POLL_INTERVAL);
    }

    // 收尾时残留在缓冲区里的多字节序列按宽松解码补上，避免丢掉末尾字符
    if !pending.is_empty() {
        out.push_str(&String::from_utf8_lossy(&pending));
    }
    Ok(out)
}

/// 在指定连接上开一个瞬态 exec 通道执行脚本并读回输出。
/// 会话可能处于非阻塞模式，因此按 EAGAIN 轮询并在超时后返回已读到的内容。
fn run_probe_command(sess: &ssh2::Session, command: &str) -> Result<String, String> {
    let mut channel = wait_retry(|| sess.channel_session(), PTY_WAIT_TIMEOUT)
        .map_err(|e| format!("打开命令通道失败: {}", e))?;
    wait_retry(|| channel.exec(command), PTY_WAIT_TIMEOUT)
        .map_err(|e| format!("执行命令失败: {}", e))?;

    let mut output: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];
    let start = Instant::now();
    loop {
        match channel.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if start.elapsed() >= CWD_PROBE_TIMEOUT {
                    break;
                }
                std::thread::sleep(IO_POLL_INTERVAL);
            }
            Err(e) => return Err(format!("读取命令输出失败: {}", e)),
        }
        if start.elapsed() >= CWD_PROBE_TIMEOUT {
            break;
        }
    }
    let _ = wait_retry(|| channel.wait_close(), Duration::from_secs(1));

    Ok(String::from_utf8_lossy(&output).to_string())
}

/// 远端工作目录探测脚本（必须在终端所属连接上执行）：
/// 1. 按 `SSH_CONNECTION` 的客户端端口认本连接：同一条连接下 shell 通道与 exec 通道
///    携带完全相同的该变量，不同 tab 是不同 TCP 连接、端口不同，因此可精确区分是哪个会话，
///    且不依赖 sshd 的进程树形状（登录 shell 被包装脚本再 fork 一层也能认对）；
/// 2. 兜底：与探测进程同父、且 stdin 指向伪终端的兄弟进程（即本会话登录 shell）；
/// 3. 兜底：/dev/pts 活跃度启发式，只认“stdin 指向 pts 的 shell”（老算法会选中 sshd 的
///    家目录或挂在 pts 上的后台服务，导致工作区跳到无关目录）；
/// 4. 兜底：本用户下所有交互 shell（同样要求 stdin 指向 pts）；
/// 5. 仍无法判断则降级为 HOME 或 /
const CWD_DETECT_SCRIPT: &str = r#"CWD=""

SELF_PORT=$(tr '\0' '\n' < "/proc/$$/environ" 2>/dev/null | awk -F= '/^SSH_CONNECTION=/{split($2,f," "); print f[2]; exit}')
SELF_PPID=$(awk '/^PPid:/{print $2}' "/proc/$$/status" 2>/dev/null)

if [ -n "$SELF_PORT" ]; then
  for pid in $(ps -u "$(id -u 2>/dev/null || echo 0)" -o pid= 2>/dev/null); do
    [ "$pid" = "$$" ] && continue
    [ -d "/proc/$pid" ] || continue
    case "$(readlink "/proc/$pid/fd/0" 2>/dev/null)" in
      /dev/pts/*) ;;
      *) continue ;;
    esac
    case "$(cat "/proc/$pid/comm" 2>/dev/null)" in
      bash|zsh|sh|dash|ash|ksh|mksh|fish|csh|tcsh|nu|pwsh|xonsh) ;;
      *) continue ;;
    esac
    pid_port=$(tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | awk -F= '/^SSH_CONNECTION=/{split($2,f," "); print f[2]; exit}')
    [ "$pid_port" = "$SELF_PORT" ] || continue
    target=$(readlink -f "/proc/$pid/cwd" 2>/dev/null)
    if [ -n "$target" ] && [ -d "$target" ]; then
      CWD="$target"
      break
    fi
  done
fi

if [ -z "$CWD" ] && [ -n "$SELF_PPID" ]; then
  for pid in $(ps -eo pid=,ppid= 2>/dev/null | awk -v p="$SELF_PPID" '$2 == p {print $1}'); do
    [ "$pid" = "$$" ] && continue
    [ -d "/proc/$pid" ] || continue
    case "$(readlink "/proc/$pid/fd/0" 2>/dev/null)" in
      /dev/pts/*) ;;
      *) continue ;;
    esac
    target=$(readlink -f "/proc/$pid/cwd" 2>/dev/null)
    if [ -n "$target" ] && [ -d "$target" ]; then
      CWD="$target"
      break
    fi
  done
fi

if [ -z "$CWD" ]; then
  for pts in $(ls -t /dev/pts/[0-9]* 2>/dev/null); do
    for pid in $(fuser "$pts" 2>/dev/null); do
      [ "$pid" = "$$" ] && continue
      [ -d "/proc/$pid" ] || continue
      case "$(cat "/proc/$pid/comm" 2>/dev/null)" in
        bash|zsh|sh|dash|ash|ksh|mksh|fish|csh|tcsh|nu|pwsh|xonsh) ;;
        *) continue ;;
      esac
      case "$(readlink "/proc/$pid/fd/0" 2>/dev/null)" in
        /dev/pts/*) ;;
        *) continue ;;
      esac
      target=$(readlink -f "/proc/$pid/cwd" 2>/dev/null)
      if [ -n "$target" ] && [ -d "$target" ]; then
        CWD="$target"
        break 2
      fi
    done
  done
fi

if [ -z "$CWD" ]; then
  for pid in $(ps -u "$(id -u 2>/dev/null || echo 0)" -o pid= 2>/dev/null); do
    [ "$pid" = "$$" ] && continue
    [ -d "/proc/$pid" ] || continue
    case "$(cat "/proc/$pid/comm" 2>/dev/null)" in
      bash|zsh|sh|dash|ash|ksh|mksh|fish|csh|tcsh|nu|pwsh|xonsh) ;;
      *) continue ;;
    esac
    case "$(readlink "/proc/$pid/fd/0" 2>/dev/null)" in
      /dev/pts/*) ;;
      *) continue ;;
    esac
    target=$(readlink -f "/proc/$pid/cwd" 2>/dev/null)
    if [ -n "$target" ] && [ -d "$target" ]; then
      CWD="$target"
      break
    fi
  done
fi

if [ -z "$CWD" ] || [ ! -d "$CWD" ]; then
  CWD="${HOME:-/}"
fi
echo "$CWD"
"#;

/// 建立 TCP 连接、完成握手、校验主机密钥并认证。
/// 主机密钥在认证之前校验，避免把凭证发送给中间人。
fn connect_and_auth(config: &SshConnectConfig) -> Result<(ssh2::Session, HostKeyInfo), String> {
    let addr = resolve_addr(&config.host, config.port)?;
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .map_err(|e| format!("TCP 连接超时/失败: {}", e))?;
    // 关闭 Nagle，降低交互式输入的往返延迟
    let _ = stream.set_nodelay(true);

    let mut sess = ssh2::Session::new().map_err(|e| e.to_string())?;
    sess.set_tcp_stream(stream);
    // 必须在 handshake 之前设置主机密钥算法优先级
    prefer_openssh_host_key_order(&sess);
    sess.handshake().map_err(|e| format!("SSH 握手失败: {}", e))?;

    let host_key = verify_host_key(&sess, config)?;

    authenticate(&sess, config)?;

    if !sess.authenticated() {
        return Err("身份认证未通过，请检查账号密码或密钥".into());
    }
    Ok((sess, host_key))
}

/// 与 OpenSSH 客户端的 HostKeyAlgorithms 默认顺序保持一致。
///
/// libssh2 默认把 ECDSA 排在 ed25519 之前，而 OpenSSH 优先 ed25519；
/// 若服务器同时提供两种密钥，`ssh` 会把 ed25519 写进 known_hosts，
/// 而 libssh2 却协商到 ECDSA，主机名匹配、密钥不同，就会被误报为
/// “主机密钥不一致”而无法登录。统一优先级后两者协商结果一致。
fn prefer_openssh_host_key_order(sess: &ssh2::Session) {
    // 未编译进 libssh2 的算法会被自动忽略
    let prefs = [
        "ssh-ed25519",
        "ssh-ed25519-cert-v01@openssh.com",
        "ecdsa-sha2-nistp256",
        "ecdsa-sha2-nistp384",
        "ecdsa-sha2-nistp521",
        "ecdsa-sha2-nistp256-cert-v01@openssh.com",
        "ecdsa-sha2-nistp384-cert-v01@openssh.com",
        "ecdsa-sha2-nistp521-cert-v01@openssh.com",
        "rsa-sha2-512",
        "rsa-sha2-256",
        "ssh-rsa",
        "ssh-dss",
    ]
    .join(",");
    let _ = sess.method_pref(ssh2::MethodType::HostKey, &prefs);
}

fn known_hosts_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("known_hosts")
}

/// 计算 OpenSSH 风格的 SHA256 指纹
fn fingerprint_sha256(key: &[u8]) -> String {
    let digest = Sha256::digest(key);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    )
}

/// known_hosts 里存的是 base64 密钥，解码后算指纹用于向用户展示
fn fingerprint_from_base64(key_b64: &str) -> Option<String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(key_b64.trim())
        .ok()?;
    Some(fingerprint_sha256(&raw))
}

/// 判断 known_hosts 条目名是否指向当前主机（支持逗号分隔的多个别名）。
/// 语义对齐 OpenSSH：非默认端口的记录写作 `[host]:port`，
/// 因此裸主机名只在 22 端口下才视为同一主机。
fn name_covers_host(name: &str, config: &SshConnectConfig) -> bool {
    let port_entry = format!("[{}]:{}", config.host, config.port);
    name.split(',').map(str::trim).any(|part| {
        if part == port_entry {
            return true;
        }
        config.port == 22 && part == config.host
    })
}

/// 某主机在 known_hosts 中已记录的密钥指纹（去重，限量避免异常文件产生巨量输出）
fn existing_host_fingerprints(known_hosts: &ssh2::KnownHosts, config: &SshConnectConfig) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    if let Ok(hosts) = known_hosts.hosts() {
        for host in hosts {
            let Some(name) = host.name() else { continue };
            if !name_covers_host(name, config) {
                continue;
            }
            if let Some(fp) = fingerprint_from_base64(host.key()) {
                if !result.contains(&fp) {
                    result.push(fp);
                }
            }
            if result.len() >= 8 {
                break;
            }
        }
    }
    result
}

/// 校验远端主机密钥：对比 ~/.ssh/known_hosts。
/// - 一致：放行
/// - 未知：仅在用户已确认（trust_host_key）时写入 known_hosts 后放行，
///   否则返回 HOST_KEY_UNKNOWN|<指纹>，由前端弹窗确认
/// - 不一致：默认阻断并返回 HOST_KEY_MISMATCH|<新指纹>|<旧指纹...>；
///   用户在界面上确认替换后会传入 trust_host_key=true，此时移除同名旧条目
///   并写入新密钥（等价于 ssh-keygen -R + 重新信任）
fn verify_host_key(sess: &ssh2::Session, config: &SshConnectConfig) -> Result<HostKeyInfo, String> {
    let (key, key_type) = sess
        .host_key()
        .ok_or_else(|| "无法获取远端主机公钥".to_string())?;
    let fingerprint = fingerprint_sha256(key);

    let mut known_hosts = sess
        .known_hosts()
        .map_err(|e| format!("初始化 known_hosts 失败: {}", e))?;
    let kh_path = known_hosts_path();
    if kh_path.exists() {
        // 解析失败不阻断：当作无记录处理，交由指纹确认流程兜底
        let _ = known_hosts.read_file(&kh_path, KnownHostFileKind::OpenSSH);
    }

    // 非默认端口必须写成 [host]:port 形式，否则后续 check_port 匹配不到
    let host_entry = if config.port == 22 {
        config.host.clone()
    } else {
        format!("[{}]:{}", config.host, config.port)
    };

    match known_hosts.check_port(&config.host, config.port, key) {
        CheckResult::Match => Ok(HostKeyInfo {
            fingerprint,
            status: "matched".into(),
        }),
        CheckResult::Mismatch => {
            let old_fingerprints = existing_host_fingerprints(&known_hosts, config);
            if !config.trust_host_key {
                return Err(format!(
                    "HOST_KEY_MISMATCH|{}|{}",
                    fingerprint,
                    old_fingerprints.join(";")
                ));
            }
            // 用户已确认替换：先移除同名旧条目，再写入新密钥
            if let Ok(hosts) = known_hosts.hosts() {
                for host in hosts {
                    if let Some(name) = host.name() {
                        if name_covers_host(name, config) {
                            let _ = known_hosts.remove(&host);
                        }
                    }
                }
            }
            write_host_key(&mut known_hosts, &kh_path, &host_entry, key, key_type)?;

            // 复核：若仍不一致（例如同名条目被哈希化存储，无法按名移除），
            // 明确告知用户手动清理，避免“看似已信任却仍连不上”
            match known_hosts.check_port(&config.host, config.port, key) {
                CheckResult::Match => Ok(HostKeyInfo {
                    fingerprint,
                    status: "replaced".into(),
                }),
                _ => Err(format!(
                    "主机密钥已尝试更新，但 ~/.ssh/known_hosts 中仍存在冲突记录（可能为哈希存储）。\n新指纹：{}\n请手动执行 ssh-keygen -R {} 后重试",
                    fingerprint, config.host
                )),
            }
        }
        CheckResult::NotFound => {
            if !config.trust_host_key {
                return Err(format!("HOST_KEY_UNKNOWN|{}", fingerprint));
            }
            write_host_key(&mut known_hosts, &kh_path, &host_entry, key, key_type)?;
            Ok(HostKeyInfo {
                fingerprint,
                status: "trusted".into(),
            })
        }
        CheckResult::Failure => Err("主机密钥校验失败（known_hosts 不可用）".into()),
    }
}

/// 把主机公钥写入 known_hosts 文件
fn write_host_key(
    known_hosts: &mut ssh2::KnownHosts,
    kh_path: &Path,
    host_entry: &str,
    key: &[u8],
    key_type: ssh2::HostKeyType,
) -> Result<(), String> {
    if let Some(parent) = kh_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 ~/.ssh 目录失败: {}", e))?;
    }
    known_hosts
        .add(host_entry, key, host_entry, key_type.into())
        .map_err(|e| format!("写入 known_hosts 失败: {}", e))?;
    known_hosts
        .write_file(kh_path, KnownHostFileKind::OpenSSH)
        .map_err(|e| format!("保存 known_hosts 失败: {}", e))
}

/// 真实连通性测试：TCP 建连 + SSH 握手 + 主机密钥校验 + 身份认证，返回真实耗时。
pub fn test_connection(config: &SshConnectConfig) -> Result<ConnectionTestResult, String> {
    let start = Instant::now();
    let (_sess, host_key) = connect_and_auth(config)?;
    let latency_ms = start.elapsed().as_millis() as u64;
    Ok(ConnectionTestResult {
        latency_ms,
        host_key_fingerprint: host_key.fingerprint,
        host_key_status: host_key.status,
    })
}

/// 采集远端真实指标（发行版 / CPU / 内存 / 磁盘 / 延迟）。
/// 使用一次性连接执行探测脚本，不占用终端与 SFTP 会话。
const PROBE_SCRIPT: &str = r#"if [ -r /etc/os-release ]; then
  . /etc/os-release
  echo "OS_ID=${ID:-}"
  echo "OS_NAME=${PRETTY_NAME:-${NAME:-}}"
else
  echo "OS_ID="
  echo "OS_NAME=$(uname -s 2>/dev/null)"
fi
free -m 2>/dev/null | awk '/^Mem:/{print "MEM=" $2 " " $3}'
df -P / 2>/dev/null | awk 'NR==2{print "DISK=" $2 " " $3}'
echo "C1 $(grep '^cpu ' /proc/stat 2>/dev/null)"
sleep 0.4
echo "C2 $(grep '^cpu ' /proc/stat 2>/dev/null)"
"#;

pub fn probe_host_info(config: &SshConnectConfig) -> Result<HostProbeResult, String> {
    let start = Instant::now();
    let (sess, _host_key) = connect_and_auth(config)?;
    let ping_ms = start.elapsed().as_millis() as u64;

    // 探测阶段走阻塞模式并设置超时，避免远端不响应时线程永久挂起
    sess.set_timeout(SFTP_IO_TIMEOUT_MS);
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("打开探测通道失败: {}", e))?;
    channel
        .exec(PROBE_SCRIPT)
        .map_err(|e| format!("执行探测脚本失败: {}", e))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| format!("读取探测结果失败: {}", e))?;
    let _ = channel.wait_close();

    Ok(parse_probe_output(&output, ping_ms))
}

fn parse_probe_output(output: &str, ping_ms: u64) -> HostProbeResult {
    let mut result = HostProbeResult {
        ping_ms: Some(ping_ms),
        ..Default::default()
    };
    let mut cpu_before: Option<Vec<u64>> = None;
    let mut cpu_after: Option<Vec<u64>> = None;

    for line in output.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("OS_ID=") {
            if !v.is_empty() {
                result.os = Some(v.to_string());
            }
        } else if let Some(v) = line.strip_prefix("OS_NAME=") {
            if !v.is_empty() {
                result.os_name = Some(v.to_string());
            }
        } else if let Some(v) = line.strip_prefix("MEM=") {
            result.mem = parse_usage_percent(v);
        } else if let Some(v) = line.strip_prefix("DISK=") {
            result.disk = parse_usage_percent(v);
        } else if let Some(v) = line.strip_prefix("C1 ") {
            cpu_before = Some(parse_cpu_fields(v));
        } else if let Some(v) = line.strip_prefix("C2 ") {
            cpu_after = Some(parse_cpu_fields(v));
        }
    }

    if let (Some(before), Some(after)) = (cpu_before, cpu_after) {
        result.cpu = cpu_usage_percent(&before, &after);
    }
    result
}

/// 解析 "总量 已用" 形式的两个数值，返回已用百分比
fn parse_usage_percent(raw: &str) -> Option<u32> {
    let mut parts = raw.split_whitespace();
    let total: f64 = parts.next()?.parse().ok()?;
    let used: f64 = parts.next()?.parse().ok()?;
    if total <= 0.0 {
        return None;
    }
    Some(((used / total) * 100.0).round().clamp(0.0, 100.0) as u32)
}

/// 解析 /proc/stat 中 cpu 行的数值部分（跳过 "cpu" 标识）
fn parse_cpu_fields(raw: &str) -> Vec<u64> {
    raw.split_whitespace()
        .filter_map(|v| v.parse::<u64>().ok())
        .collect()
}

/// 依据两次 /proc/stat 采样计算 CPU 使用率百分比
fn cpu_usage_percent(before: &[u64], after: &[u64]) -> Option<u32> {
    let len = before.len().min(after.len());
    if len < 4 {
        return None;
    }
    let total_before: u64 = before[..len].iter().sum();
    let total_after: u64 = after[..len].iter().sum();
    // 下标 3=idle、4=iowait（若有），二者之和视为空闲
    let idle_before = before[3] + before.get(4).copied().unwrap_or(0);
    let idle_after = after[3] + after.get(4).copied().unwrap_or(0);

    let total_delta = total_after.saturating_sub(total_before);
    if total_delta == 0 {
        return None;
    }
    let idle_delta = idle_after.saturating_sub(idle_before);
    let usage = 100.0 - (idle_delta as f64 / total_delta as f64) * 100.0;
    Some(usage.round().clamp(0.0, 100.0) as u32)
}

/// 毫秒级极速远端监控采集脚本：
/// 一次性聚合 /proc/uptime、/proc/meminfo、/proc/net/dev、/proc/stat 与 df/users，
/// 全部直接读取虚拟文件系统，执行仅需 5~15ms，完全无 sleep 阻塞。
const REMOTE_METRICS_SCRIPT: &str = r#"
printf "HOSTNAME:%s\n" "$(hostname 2>/dev/null || cat /proc/sys/kernel/hostname 2>/dev/null)"
printf "UPTIME:%s\n" "$(cat /proc/uptime 2>/dev/null | cut -d' ' -f1)"
printf "USERS:%s\n" "$(users 2>/dev/null || who 2>/dev/null | awk '{print $1}' | tr '\n' ' ')"
printf "MEM:%s\n" "$(awk '/MemTotal:/{t=$2} /MemAvailable:/{a=$2} /MemFree:/{f=$2} END{print t, (a?a:f)}' /proc/meminfo 2>/dev/null)"
printf "NET:%s\n" "$(awk 'NR>2 && !/:/{next} /:/{gsub(/:/," "); if($1!="lo"){rx+=$2; tx+=$10}} END{print rx, tx}' /proc/net/dev 2>/dev/null)"
printf "DISK:%s\n" "$(df -P -k 2>/dev/null | awk 'NR>1 && $1 ~ /^\// {printf "%s=%s=%s=%s;", $6, $5, $2, $3}')"
printf "CPU:%s\n" "$(grep '^cpu ' /proc/stat 2>/dev/null)"
"#;

fn format_uptime(secs: f64) -> String {
    let total_secs = secs.max(0.0) as u64;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    if days > 0 {
        format!("{} days", days)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{} mins", mins)
    }
}

fn format_kb_size(kb: u64) -> String {
    if kb >= 1024 * 1024 {
        format!("{:.1} GB", kb as f64 / (1024.0 * 1024.0))
    } else if kb >= 1024 {
        format!("{:.0} MB", kb as f64 / 1024.0)
    } else {
        format!("{} KB", kb)
    }
}

fn parse_remote_metrics(
    output: &str,
    last_snap: Option<RemoteMetricsSnapshot>,
    default_host: &str,
) -> (RemoteSystemMetrics, RemoteMetricsSnapshot) {
    let now = Instant::now();
    let mut metrics = RemoteSystemMetrics::default();
    let mut cpu_fields: Vec<u64> = Vec::new();
    let mut net_rx: u64 = 0;
    let mut net_tx: u64 = 0;

    for line in output.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("HOSTNAME:") {
            let h = v.trim();
            if !h.is_empty() {
                metrics.hostname = h.to_string();
            }
        } else if let Some(v) = line.strip_prefix("UPTIME:") {
            if let Ok(secs) = v.trim().parse::<f64>() {
                metrics.uptime = format_uptime(secs);
            }
        } else if let Some(v) = line.strip_prefix("USERS:") {
            metrics.users = v.split_whitespace().map(|s| s.to_string()).collect();
        } else if let Some(v) = line.strip_prefix("MEM:") {
            let mut parts = v.split_whitespace();
            if let (Some(total_str), Some(avail_str)) = (parts.next(), parts.next()) {
                if let (Ok(total_kb), Ok(avail_kb)) =
                    (total_str.parse::<u64>(), avail_str.parse::<u64>())
                {
                    if total_kb > 0 {
                        let used_kb = total_kb.saturating_sub(avail_kb);
                        metrics.mem_total_bytes = total_kb * 1024;
                        metrics.mem_used_bytes = used_kb * 1024;
                        metrics.mem_usage_percent = ((used_kb as f64 / total_kb as f64) * 100.0)
                            .round()
                            .clamp(0.0, 100.0) as u32;
                    }
                }
            }
        } else if let Some(v) = line.strip_prefix("NET:") {
            let mut parts = v.split_whitespace();
            if let (Some(rx_str), Some(tx_str)) = (parts.next(), parts.next()) {
                net_rx = rx_str.parse().unwrap_or(0);
                net_tx = tx_str.parse().unwrap_or(0);
            }
        } else if let Some(v) = line.strip_prefix("DISK:") {
            for item in v.split(';') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = item.split('=').collect();
                if parts.len() >= 2 {
                    let mount = parts[0].to_string();
                    let pct_str = parts[1].trim_end_matches('%');
                    let use_pct = pct_str.parse::<u32>().unwrap_or(0);
                    let total_kb = parts.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                    let used_kb = parts.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                    metrics.disks.push(RemoteDiskUsage {
                        mount_point: mount,
                        use_percent: use_pct,
                        total: format_kb_size(total_kb),
                        used: format_kb_size(used_kb),
                    });
                }
            }
        } else if let Some(v) = line.strip_prefix("CPU:") {
            cpu_fields = parse_cpu_fields(v);
        }
    }

    if metrics.hostname.is_empty() {
        metrics.hostname = default_host.to_string();
    }

    if let Some(prev) = &last_snap {
        if !cpu_fields.is_empty() && !prev.cpu_fields.is_empty() {
            metrics.cpu = cpu_usage_percent(&prev.cpu_fields, &cpu_fields).unwrap_or(0);
        }
        let delta_secs = now.duration_since(prev.sampled_at).as_secs_f64();
        if delta_secs > 0.05 {
            let rx_diff = net_rx.saturating_sub(prev.net_rx_bytes);
            let tx_diff = net_tx.saturating_sub(prev.net_tx_bytes);
            metrics.net_download_speed = rx_diff as f64 / delta_secs;
            metrics.net_upload_speed = tx_diff as f64 / delta_secs;
        }
    }

    let new_snap = RemoteMetricsSnapshot {
        sampled_at: now,
        cpu_fields,
        net_rx_bytes: net_rx,
        net_tx_bytes: net_tx,
    };

    (metrics, new_snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_masks_common_credential_forms() {
        assert_eq!(redact("export DB_PASSWORD=supersecret"), "export DB_PASSWORD=***");
        assert_eq!(redact("mysql -p s3cret -u root"), "mysql -p *** -u root");
        assert_eq!(redact("Authorization: token abc123"), "Authorization: token ***");
        assert_eq!(redact("api_key=deadbeef&"), "api_key=***&");
    }

    #[test]
    fn redact_keeps_non_secret_text() {
        // 普通单词不应被误伤
        assert_eq!(redact("passwordless login is fine"), "passwordless login is fine");
        assert_eq!(redact("ls -al /var/log"), "ls -al /var/log");
    }

    #[test]
    fn sanitize_host_replaces_invalid_filename_chars() {
        assert_eq!(sanitize_host("C:\\temp/foo"), "C__temp_foo");
        assert_eq!(sanitize_host("   "), "unknown-host");
        assert_eq!(sanitize_host("root@10.0.0.1"), "root@10.0.0.1");
    }

    #[test]
    fn fingerprint_matches_openssh_format_for_empty_input() {
        // SHA256("") 的 base64（无填充）是固定值，可用于校验指纹格式
        assert_eq!(
            fingerprint_sha256(b""),
            "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"
        );
    }

    #[test]
    fn parse_usage_percent_computes_ratio() {
        assert_eq!(parse_usage_percent("1024 512"), Some(50));
        assert_eq!(parse_usage_percent("8192 2048"), Some(25));
        assert_eq!(parse_usage_percent("0 0"), None);
        assert_eq!(parse_usage_percent("garbage"), None);
    }

    #[test]
    fn cpu_usage_percent_uses_idle_delta() {
        // idle 从 100 增至 200，总量从 200 增至 400 => 50% 使用率
        let before = vec![10, 0, 10, 100, 80];
        let after = vec![60, 0, 60, 200, 80];
        assert_eq!(cpu_usage_percent(&before, &after), Some(50));
        // 无时间推进时无法计算
        assert_eq!(cpu_usage_percent(&before, &before), None);
        assert_eq!(cpu_usage_percent(&[1, 2], &[3, 4]), None);
    }

    #[test]
    fn parse_probe_output_extracts_all_metrics() {
        let sample = concat!(
            "OS_ID=ubuntu\n",
            "OS_NAME=Ubuntu 22.04.4 LTS\n",
            "MEM=16384 8192\n",
            "DISK=102400 25600\n",
            "C1 cpu  100 0 100 1000 50 0 0 0 0 0\n",
            "C2 cpu  200 0 200 1200 50 0 0 0 0 0\n"
        );
        let result = parse_probe_output(sample, 37);
        assert_eq!(result.os.as_deref(), Some("ubuntu"));
        assert_eq!(result.os_name.as_deref(), Some("Ubuntu 22.04.4 LTS"));
        assert_eq!(result.mem, Some(50));
        assert_eq!(result.disk, Some(25));
        assert_eq!(result.ping_ms, Some(37));
        // 总量 +400，idle+iowait +200 => 50%
        assert_eq!(result.cpu, Some(50));
    }

    #[test]
    fn name_covers_host_matches_plain_and_ported_entries() {
        let base = SshConnectConfig {
            host: "example.com".into(),
            port: 22,
            username: "root".into(),
            auth_type: "password".into(),
            password: None,
            private_key: None,
            passphrase: None,
            trust_host_key: false,
            cols: 80,
            rows: 24,
        };
        assert!(name_covers_host("example.com", &base));
        assert!(name_covers_host("example.com,10.0.0.1", &base));
        assert!(!name_covers_host("example.org", &base));

        let custom = SshConnectConfig {
            port: 2222,
            ..base
        };
        assert!(name_covers_host("[example.com]:2222", &custom));
        assert!(!name_covers_host("example.com", &custom));
    }

    #[test]
    fn parse_probe_output_tolerates_missing_fields() {
        let result = parse_probe_output("OS_ID=\nOS_NAME=FreeBSD\n", 5);
        assert_eq!(result.os, None);
        assert_eq!(result.os_name.as_deref(), Some("FreeBSD"));
        assert_eq!(result.cpu, None);
        assert_eq!(result.mem, None);
    }

    #[test]
    fn parse_remote_metrics_extracts_complete_info() {
        let sample = concat!(
            "HOSTNAME:quickea144\n",
            "UPTIME:9622543.21\n",
            "USERS:root root root\n",
            "MEM:16432128 2097152\n",
            "NET:10485760 20971520\n",
            "DISK:/=9%=104857600=9437184;/boot=58%=1048576=608174;\n",
            "CPU:cpu  200 0 200 1200 50 0 0 0 0 0\n"
        );
        let (metrics, snap) = parse_remote_metrics(sample, None, "fallback.host");
        assert_eq!(metrics.hostname, "quickea144");
        assert_eq!(metrics.uptime, "111 days");
        assert_eq!(metrics.users, vec!["root", "root", "root"]);
        assert_eq!(metrics.disks.len(), 2);
        assert_eq!(metrics.disks[0].mount_point, "/");
        assert_eq!(metrics.disks[0].use_percent, 9);
        assert_eq!(metrics.disks[1].mount_point, "/boot");
        assert_eq!(metrics.disks[1].use_percent, 58);
        assert_eq!(snap.net_rx_bytes, 10485760);
        assert_eq!(snap.net_tx_bytes, 20971520);
    }
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
