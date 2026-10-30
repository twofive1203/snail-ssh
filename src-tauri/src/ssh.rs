use crate::authprompt::AuthPrompter;
use crate::conn::Conn;
use crate::model::{JumpHop, PortForward, ProxyConfig};
use crate::rt;
use parking_lot::Mutex;
use russh::ChannelMsg;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc as tokio_mpsc;
use uuid::Uuid;

/// 写入指令通道的容量：远端不消费输入时在这里形成背压
const SHELL_COMMAND_CAPACITY: usize = 256;
/// 写入超时：远端窗口长时间不打开时判为卡死，避免命令线程永久阻塞
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Shell 启动后的首屏输出（Last login / MOTD / 首个提示符）抓取窗口。
/// 这几行由远端在 shell 刚起来时立刻发出，而前端要等 Vue 渲染完才注册 stdout 监听，
/// 若仍走事件广播会在注册前被丢弃（表现为看不到欢迎信息），因此建会话时先同步扣下。
/// 已经读到数据后，静默这么久即认为首屏结束；始终没有数据时只等这么久。
const BANNER_QUIET_GAP: Duration = Duration::from_millis(60);
const BANNER_FIRST_BYTE_TIMEOUT: Duration = Duration::from_millis(400);
/// 首屏抓取的总上限，避免远端持续刷输出导致建连迟迟不返回
const BANNER_MAX_WAIT: Duration = Duration::from_millis(800);

/// 远程监控采样周期。与前端原先每会话 2.5s 的 setInterval 对齐；
/// 现在只有一条常驻任务按这个周期采「当前活动会话」，不再随会话数线性增长。
const METRICS_SAMPLE_INTERVAL: Duration = Duration::from_millis(2500);

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
    /// 从近到远的跳板。第一跳走 proxy（若有），之后每跳是上一条连接上的 direct-tcpip。
    #[serde(default)]
    pub jumps: Vec<JumpHop>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub forwards: Vec<PortForward>,
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
/// 会话日志的写入句柄：路径用于前端展示，tx 交给 writer 线程落盘。
/// 直接 drop 即关闭 channel，writer 线程随之 flush 退出。
struct LogSink {
    path: PathBuf,
    tx: SyncSender<String>,
}

/// 会话日志缓冲上限：写线程跟不上时丢弃而不是阻塞终端读线程
const LOG_CHANNEL_CAPACITY: usize = 256;

/// 单个日志文件上限（每主机）。滚动后旧文件不再参与大小检查，因此每主机实际占用上限是它的两倍。
const LOG_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// 过期日志的保留天数。写成常量而不是命令参数，避免前端把它放大成"清空全部日志"。
const LOG_PRUNE_MAX_AGE_DAYS: u64 = 7;

/// 日志落盘状态：writer 线程独占文件句柄与已写字节数。
/// 计数刻意不放进 LogSink——LogSink 是读线程看得见的句柄，把滚动判断留在落盘侧
/// 才能保证终端输出不因为一次 rename 卡顿。
struct SessionLogWriter {
    path: PathBuf,
    file: fs::File,
    written: u64,
}

impl SessionLogWriter {
    fn new(path: PathBuf, file: fs::File) -> Self {
        // File::create 已经截断文件，这里仍按磁盘实际长度起步：
        // 以后若改成追加打开，计数也不会从 0 重新累积。
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Self {
            path,
            file,
            written,
        }
    }

    /// 返回 false 表示磁盘写入已失败，调用方应停止继续写。
    fn write_chunk(&mut self, chunk: &str) -> bool {
        if chunk.is_empty() {
            return true;
        }
        if should_roll_log(self.written) && !self.roll() {
            // 滚动失败就不再继续写：继续写会把单文件推过上限，失去 50MB 约束的意义
            eprintln!(
                "[snail-ssh] 会话日志滚动失败，本次记录已停止: {}",
                self.path.display()
            );
            return false;
        }
        if self.file.write_all(chunk.as_bytes()).is_err() {
            return false;
        }
        self.written += chunk.len() as u64;
        true
    }

    fn roll(&mut self) -> bool {
        let _ = self.file.flush();
        let rolled = rolled_log_path(&self.path);
        // Windows 上目标文件被其它句柄占用时 rename 会失败，先显式删掉旧备份
        let _ = fs::remove_file(&rolled);
        if fs::rename(&self.path, &rolled).is_err() {
            return false;
        }
        match fs::File::create(&self.path) {
            Ok(file) => {
                crate::fsperm::restrict_file(&self.path);
                self.file = file;
                self.written = 0;
                true
            }
            Err(_) => false,
        }
    }

    fn flush(&mut self) {
        let _ = self.file.flush();
    }
}

/// 是否该把当前文件滚动到 .1：达到阈值才动，避免每个 chunk 都去算路径。
/// 写入前先判定，所以单文件最多比阈值多出一个 chunk，不会持续长。
fn should_roll_log(written: u64) -> bool {
    written >= LOG_MAX_BYTES
}

/// 滚动目标的文件名：`<原名>.1`，与 prune_session_logs 的后缀判定保持一致。
fn rolled_log_path(path: &Path) -> PathBuf {
    let mut rolled = path.as_os_str().to_os_string();
    rolled.push(".1");
    PathBuf::from(rolled)
}

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

/// 同一片段里能看到 BEGIN/END 的私钥块整段丢掉。块被输出切开时，没有 BEGIN 的后半段仍可能漏过去。
fn mask_private_key_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hiding = false;
    for line in text.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        if !hiding && lower.contains("-----begin") && lower.contains("private") {
            hiding = !lower.contains("-----end");
            out.push_str("[redacted private key]\n");
            continue;
        }
        if hiding {
            if lower.contains("-----end") {
                hiding = false;
            }
            continue;
        }
        out.push_str(line);
    }
    out
}

/// 把 password/token/secret 之类的值替换为 `***`
pub(crate) fn redact(text: &str) -> String {
    let masked = mask_private_key_blocks(text);
    let mut out = String::with_capacity(masked.len());
    let mut rest = masked.as_str();
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
pub(crate) fn push_log(
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

/// 会话日志文件名判定：`.log` 是正在写的文件，`.log.1` 是滚动出来的上一份。
fn is_session_log_file(path: &Path) -> bool {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.ends_with(".log") || name.ends_with(".log.1"),
        None => false,
    }
}

/// 删除 `logs/` 下修改时间超过 `max_age_days` 的会话日志，返回删除的文件数。
/// 日志是明文且不轮转，长期不清理会静静吃满磁盘，所以启动时顺手扫一遍；
/// 单点失败只告警不中断（清理失败不应该阻止应用启动），也不跟随符号链接以免误删目录树外文件。
pub fn prune_session_logs(max_age_days: u64) -> Result<u64, String> {
    let root = crate::storage::app_data_dir().join("logs");
    if !root.is_dir() {
        return Ok(0);
    }
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(max_age_days.saturating_mul(86_400)))
        .unwrap_or(UNIX_EPOCH);

    let mut removed = 0u64;
    let host_dirs = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!(
                "[snail-ssh] 读取会话日志目录失败 {}: {}",
                root.display(),
                err
            );
            return Ok(0);
        }
    };

    for host_entry in host_dirs.flatten() {
        // DirEntry::file_type 不跟随符号链接，链接目录在这里就被排除
        if !host_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let host_dir = host_entry.path();
        let files = match fs::read_dir(&host_dir) {
            Ok(entries) => entries,
            Err(err) => {
                eprintln!(
                    "[snail-ssh] 读取日志目录失败 {}: {}",
                    host_dir.display(),
                    err
                );
                continue;
            }
        };

        for file_entry in files.flatten() {
            // 只删普通文件：符号链接、子目录一律跳过
            if !file_entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let path = file_entry.path();
            if !is_session_log_file(&path) {
                continue;
            }
            let expired = file_entry
                .metadata()
                .and_then(|meta| meta.modified())
                .map(|mtime| mtime < cutoff)
                .unwrap_or(false);
            if !expired {
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(err) => eprintln!(
                    "[snail-ssh] 删除过期会话日志失败 {}: {}",
                    path.display(),
                    err
                ),
            }
        }
    }

    Ok(removed)
}

/// 启动时自动清理的入口：保留天数固定 7 天，前端无从传参；
/// lib.rs 的命令只负责把它包进 run_blocking 并登记到 generate_handler。
pub fn auto_prune_session_logs() -> Result<u64, String> {
    prune_session_logs(LOG_PRUNE_MAX_AGE_DAYS)
}

/// 会话日志句柄。drop 掉 `tx` 的最后一个克隆后 writer 线程退出；
/// `shutdown` 会再等它刷盘，本地终端关闭要这条保证，SSH 关闭仍只丢 sender。
pub(crate) struct SessionLogHandle {
    pub path: PathBuf,
    pub tx: SyncSender<String>,
    done: Option<std::sync::mpsc::Receiver<()>>,
}

impl SessionLogHandle {
    pub(crate) fn shutdown(self) {
        let SessionLogHandle { tx, done, .. } = self;
        drop(tx);
        if let Some(done) = done {
            let _ = done.recv_timeout(Duration::from_millis(800));
        }
    }
}

/// 在 `logs/<目录>/` 下开一份脱敏日志。目录名会做文件名清洗，标题行保留原文。
pub(crate) fn start_session_log(
    dir_segment: &str,
    header_label: &str,
    file_tag: &str,
) -> Result<SessionLogHandle, String> {
    let dir = crate::storage::app_data_dir()
        .join("logs")
        .join(sanitize_host(dir_segment));
    fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {}", e))?;
    crate::fsperm::restrict_dir(&crate::storage::app_data_dir().join("logs"));
    crate::fsperm::restrict_dir(&dir);

    let started = now_secs();
    let path = dir.join(format!("{}_{}.log", format_timestamp(started), file_tag));
    let file = fs::File::create(&path).map_err(|e| format!("创建日志文件失败: {}", e))?;
    crate::fsperm::restrict_file(&path);

    let (tx, rx) = sync_channel::<String>(LOG_CHANNEL_CAPACITY);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let header = format!(
        "# SnailSSH session log (credentials redacted): {} started {} UTC\n",
        header_label,
        format_timestamp(started)
    );
    let writer_path = path.clone();
    std::thread::spawn(move || {
        let mut writer = SessionLogWriter::new(writer_path, file);
        let _ = writer.write_chunk(&header);
        while let Ok(chunk) = rx.recv() {
            if !writer.write_chunk(&chunk) {
                break;
            }
        }
        writer.flush();
        let _ = done_tx.send(());
    });

    Ok(SessionLogHandle {
        path,
        tx,
        done: Some(done_rx),
    })
}

/// 终端会话：shell 通道本体由输出泵任务独占，其余线程通过指令通道操作它。
pub struct ActiveSession {
    /// 终端连接：shell 输出泵、工作目录探测、一次性 exec 都跑在这条连接上
    conn: Arc<Conn>,
    shell: ShellHandle,
    /// SFTP 专用连接（首次使用时懒加载的独立连接）
    sftp_conn: Arc<Mutex<Option<Arc<Conn>>>>,
    config: SshConnectConfig,
    is_alive: Arc<AtomicBool>,
    /// 建连时扣下的首屏输出，等前端注册好监听后由 take_initial_output 取走（只取一次）
    banner: Arc<Mutex<String>>,
    /// 会话日志槽：开着时输出泵把脱敏后的内容 try_send 给专用 writer 线程
    log_sink: Arc<Mutex<Option<LogSink>>>,
    /// 后续 SFTP / 探测要重新认证时，用它向界面提问（OTP）
    app: AppHandle,
}

/// 终端 shell 的写输入 / 改窗口 / 关闭指令
enum ShellCmd {
    Write(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// shell 通道的写入口：通道被输出泵任务独占，避免"读线程持锁阻塞、写线程拿不到锁"
struct ShellHandle {
    tx: tokio_mpsc::Sender<ShellCmd>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// 后台采样的目标槽：只保存当前活动会话 id，None 表示不采样。
///
/// 单独包一层而不是裸 `Option<String>`，是为了让「切会话 → 采样目标随之切换」
/// 这类纯逻辑能脱离真实 SSH 连接做单测。
#[derive(Default)]
struct MetricsTargetSlot {
    session_id: Option<String>,
}

impl MetricsTargetSlot {
    fn set(&mut self, session_id: Option<String>) {
        self.session_id = session_id;
    }

    fn current(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// 关闭会话时清目标：只有正在被采样的那个会话需要清，返回是否真的清了。
    /// 不清的话后台会一直对着已断开的连接 exec，每轮都失败。
    fn clear_if(&mut self, session_id: &str) -> bool {
        if self.session_id.as_deref() == Some(session_id) {
            self.session_id = None;
            true
        } else {
            false
        }
    }
}

/// 采样错误去重：会话断开后同一个错误每 2.5s 都会复现，
/// 只在「与上次报告的错误不同」时返回 true，避免日志刷屏；采样成功后 clear 复位。
#[derive(Default)]
struct MetricsErrorThrottle {
    last_reported: Option<String>,
}

impl MetricsErrorThrottle {
    fn should_report(&mut self, error: &str) -> bool {
        if self.last_reported.as_deref() == Some(error) {
            return false;
        }
        self.last_reported = Some(error.to_string());
        true
    }

    fn clear(&mut self) {
        self.last_reported = None;
    }
}

#[derive(Clone)]
pub struct SshManager {
    sessions: Arc<Mutex<HashMap<String, Arc<ActiveSession>>>>,
    metrics_cache: Arc<Mutex<HashMap<String, RemoteMetricsSnapshot>>>,
    /// 后台采样目标槽。用「换目标」而不是「启停任务」驱动采样，
    /// 免去任务启停与前端时序之间的竞态。
    metrics_target: Arc<Mutex<MetricsTargetSlot>>,
    /// 采样任务只启动一次的守卫
    metrics_watcher_started: Arc<AtomicBool>,
    /// 目标变更时的立即唤醒：切到新会话不等整个周期就能出第一帧数据
    metrics_wake: Arc<tokio::sync::Notify>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            metrics_cache: Arc::new(Mutex::new(HashMap::new())),
            metrics_target: Arc::new(Mutex::new(MetricsTargetSlot::default())),
            metrics_watcher_started: Arc::new(AtomicBool::new(false)),
            metrics_wake: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn create_session(
        &self,
        config: SshConnectConfig,
        app_handle: AppHandle,
    ) -> Result<String, String> {
        let session_id = format!("ssh-{}", Uuid::new_v4());
        let is_alive = Arc::new(AtomicBool::new(true));
        let log_sink: Arc<Mutex<Option<LogSink>>> = Arc::new(Mutex::new(None));
        let banner: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

        let stdout_event = format!("ssh-stdout-{}", session_id);
        let closed_event = format!("ssh-closed-{}", session_id);

        let forward_session_id = session_id.clone();
        let (conn, shell) = rt::block_on({
            let config = config.clone();
            let is_alive = Arc::clone(&is_alive);
            let log_sink = Arc::clone(&log_sink);
            let banner = Arc::clone(&banner);
            let app = app_handle.clone();
            async move {
                let conn = Conn::connect_async(config.clone(), Some(AuthPrompter::new(app.clone()))).await?;
                let forward_notes = conn.start_forwards(&config.forwards).await;
                if !forward_notes.is_empty() {
                    let _ = app.emit(
                        "ssh-forward-status",
                        serde_json::json!({
                            "sessionId": forward_session_id,
                            "notes": forward_notes,
                        }),
                    );
                }
                let mut channel =
                    Conn::open_shell(conn.handle(), config.cols, config.rows).await?;

                // 先同步扣下首屏：此时前端还没注册监听，交给输出泵 emit 只会被丢掉。
                *banner.lock() = read_initial_output(&mut channel).await;

                let (tx, rx) = tokio_mpsc::channel(SHELL_COMMAND_CAPACITY);
                let task = spawn_shell_pump(
                    channel,
                    rx,
                    Arc::new(app),
                    stdout_event,
                    closed_event,
                    Arc::clone(&is_alive),
                    Arc::clone(&log_sink),
                );
                Ok::<_, String>((
                    conn,
                    ShellHandle {
                        tx,
                        task: Mutex::new(Some(task)),
                    },
                ))
            }
        })?;

        let active = Arc::new(ActiveSession {
            conn,
            shell,
            sftp_conn: Arc::new(Mutex::new(None)),
            config,
            is_alive,
            banner,
            log_sink,
            app: app_handle.clone(),
        });
        self.sessions.lock().insert(session_id.clone(), active);
        Ok(session_id)
    }

    pub fn session_app(&self, session_id: &str) -> Result<AppHandle, String> {
        Ok(self.get_active(session_id)?.app.clone())
    }

    fn get_active(&self, session_id: &str) -> Result<Arc<ActiveSession>, String> {
        self.sessions
            .lock()
            .get(session_id)
            .map(Arc::clone)
            .ok_or_else(|| "会话不存在或已关闭".to_string())
    }

    /// 写入远端：指令通道满（远端未消费）时短暂退让重试，超时即判失败
    pub fn write_session(&self, session_id: &str, data: &str) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let bytes = data.as_bytes().to_vec();
        let start = Instant::now();
        loop {
            match active.shell.tx.try_send(ShellCmd::Write(bytes.clone())) {
                Ok(()) => return Ok(()),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    if start.elapsed() > WRITE_TIMEOUT {
                        return Err("写入超时（远端未消费输入）".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    return Err("会话已断开，无法写入".into());
                }
            }
        }
    }

    pub fn resize_session(&self, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
        let active = self.get_active(session_id)?;
        // 窗口尺寸属于尽力而为的调整，失败不影响终端可用性
        let _ = active.shell.tx.try_send(ShellCmd::Resize { cols, rows });
        Ok(())
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.metrics_cache.lock().remove(session_id);
        // 被关的会话若正是采样目标，立刻停采样，别让后台继续撞已断开的连接
        self.metrics_target.lock().clear_if(session_id);
        let active = self.sessions.lock().remove(session_id);
        if let Some(active) = active {
            active.is_alive.store(false, Ordering::Relaxed);
            // 显式 drop 日志槽：关闭 channel 让 writer 线程退出并 flush，避免尾部内容丢失
            active.log_sink.lock().take();
            // 尽力关闭通道；SFTP 连接随 Arc 释放
            let _ = active.shell.tx.try_send(ShellCmd::Close);
            let conn = Arc::clone(&active.conn);
            rt::spawn(async move {
                // 给输出泵一点时间把 Close 发出去，然后断开整条连接
                tokio::time::sleep(Duration::from_millis(80)).await;
                conn.disconnect_async().await;
            });
            if let Some(task) = active.shell.task.lock().take() {
                task.abort();
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

        let short_id: String = session_id.trim_start_matches("ssh-").chars().take(8).collect();
        let handle = start_session_log(host_label, host_label, &short_id)?;
        let path = handle.path.to_string_lossy().to_string();
        // SSH 关闭仍只 drop sender，不等写线程；done 通道随 handle 丢掉
        *active.log_sink.lock() = Some(LogSink {
            path: handle.path.clone(),
            tx: handle.tx.clone(),
        });
        Ok(Some(path))
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
    pub fn get_sftp_session(&self, session_id: &str) -> Result<Arc<Conn>, String> {
        let active = self.get_active(session_id)?;

        // 快路径：已建立则直接复用
        if let Some(existing) = active.sftp_conn.lock().as_ref() {
            return Ok(Arc::clone(existing));
        }

        // 建连期间不持有会话表锁，避免阻塞其他会话操作
        let mut config = active.config.clone();
        // SFTP 是第二条连接，不重复绑定同一批本地端口
        config.forwards.clear();
        let created = Conn::connect_prompted(&config, AuthPrompter::new(active.app.clone()))?;

        let mut slot = active.sftp_conn.lock();
        if let Some(existing) = slot.as_ref() {
            // 并发进入时复用先建立好的连接
            return Ok(Arc::clone(existing));
        }
        *slot = Some(Arc::clone(&created));
        Ok(created)
    }

    /// 丢弃该会话缓存的 SFTP 连接（SFTP 子系统通道已被关闭时调用）。
    /// 下次 `get_sftp_session` 会重新建连，让后续读写自愈。
    pub fn reset_sftp_session(&self, session_id: &str) {
        let Ok(active) = self.get_active(session_id) else {
            return;
        };
        let dropped = active.sftp_conn.lock().take();
        if let Some(conn) = dropped {
            // 这条连接可能已经半死，不等挥手，直接异步断开
            rt::spawn(async move { conn.disconnect_async().await });
        }
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

        let output = active.conn.exec(CWD_DETECT_SCRIPT)?;

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
        active.conn.exec(command)
    }

    /// 采集远端主机实时系统指标（MobaXterm 风格底部监控条），复用终端连接瞬态 exec。
    ///
    /// 前端正常路径已改成订阅后台采样任务广播的 `ssh-metrics-<session_id>`，
    /// 这里保留同步入口供浏览器预览 / 测试 / 一次性拉取使用，解析与基线更新走同一个内部函数。
    pub fn get_remote_metrics(&self, session_id: &str) -> Result<RemoteSystemMetrics, String> {
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let default_host = active.config.host.clone();
        let output = active.conn.exec(REMOTE_METRICS_SCRIPT)?;
        Ok(self.apply_metrics_output(session_id, &output, &default_host))
    }

    /// 切换后台采样目标：前端切活动会话 / 开关监控条时调用，None 表示停止采样。
    pub fn set_metrics_target(&self, session_id: Option<String>) {
        self.metrics_target.lock().set(session_id);
        // 换目标立即采一次：否则切回会话要先干等一个采样周期才出数据
        self.metrics_wake.notify_one();
    }

    /// 启动后台监控采样任务：每 `METRICS_SAMPLE_INTERVAL` 对目标会话采一次样并广播
    /// `ssh-metrics-<session_id>`。幂等，重复调用只生效一次。
    ///
    /// 采样走 `exec_async` 而不是阻塞的 `Conn::exec`：本任务就住在 rt 运行时里，
    /// 在异步上下文内 `block_on` 会 panic；同时也避免每条会话占一个阻塞线程，
    /// 把线程让给 SFTP / 传输。
    pub fn start_metrics_watcher(&self, app: AppHandle) {
        if self.metrics_watcher_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let manager = self.clone();
        let wake = Arc::clone(&self.metrics_wake);
        rt::spawn(async move {
            let mut errors = MetricsErrorThrottle::default();
            loop {
                // 被目标变更唤醒时立刻采一帧，否则按固定周期采样。
                // 放在循环开头（而非末尾）是为了「刚切过去」也能马上有数据。
                let _ = tokio::time::timeout(METRICS_SAMPLE_INTERVAL, wake.notified()).await;
                // 只在取目标这一瞬间持锁，采样与 emit 期间都不握锁
                let target = manager.metrics_target.lock().current().map(str::to_string);
                let Some(session_id) = target else {
                    // 没目标说明监控关了或切走了，复位去重状态，下次重新告警
                    errors.clear();
                    continue;
                };

                match manager.sample_metrics_async(&session_id).await {
                    Ok(metrics) => {
                        errors.clear();
                        let _ = app.emit(&format!("ssh-metrics-{}", session_id), metrics);
                    }
                    Err(err) => {
                        if errors.should_report(&err) {
                            eprintln!("[snail-ssh] 远程监控采样失败（{}）: {}", session_id, err);
                        }
                    }
                }
            }
        });
    }

    /// 后台单次采样：异步 exec + 更新速率基线
    async fn sample_metrics_async(&self, session_id: &str) -> Result<RemoteSystemMetrics, String> {
        let active = self.get_active(session_id)?;
        if !active.is_alive.load(Ordering::Relaxed) {
            return Err("会话已断开".into());
        }

        let default_host = active.config.host.clone();
        let output = active
            .conn
            .exec_async(REMOTE_METRICS_SCRIPT, crate::conn::EXEC_TIMEOUT)
            .await?;
        Ok(self.apply_metrics_output(session_id, &output, &default_host))
    }

    /// 解析脚本输出并刷新该会话的速率基线。
    /// 必须用上一次快照做差值基准（CPU 与网络速率），否则第一次之后永远是 0 或跳变。
    fn apply_metrics_output(
        &self,
        session_id: &str,
        output: &str,
        default_host: &str,
    ) -> RemoteSystemMetrics {
        let mut cache = self.metrics_cache.lock();
        let prev_snap = cache.get(session_id).cloned();
        let (metrics, new_snap) = parse_remote_metrics(output, prev_snap, default_host);
        cache.insert(session_id.to_string(), new_snap);
        metrics
    }
}

/// 输出端：把远端输出与关闭原因送出去。
/// 生产环境是 Tauri 事件广播，集成测试里换成内存收集器，两边跑同一套输出泵逻辑。
pub(crate) trait ShellEventSink: Send + Sync + 'static {
    fn send(&self, event: &str, payload: String);
}

impl ShellEventSink for AppHandle {
    fn send(&self, event: &str, payload: String) {
        let _ = self.emit(event, payload);
    }
}

/// 建连时同步扣下 shell 首屏输出（Last login / MOTD / 首个提示符）。
/// 读到数据后静默 `BANNER_QUIET_GAP` 即收工，始终无数据则按 `BANNER_FIRST_BYTE_TIMEOUT` 收工，
/// 总耗时不超过 `BANNER_MAX_WAIT`。
async fn read_initial_output(channel: &mut crate::conn::ShellChannel) -> String {
    let mut out = String::new();
    let mut pending: Vec<u8> = Vec::new();
    let start = Instant::now();
    let mut last_data: Option<Instant> = None;

    loop {
        let now = Instant::now();
        if now.duration_since(start) >= BANNER_MAX_WAIT {
            break;
        }
        let deadline = match last_data {
            Some(at) => at + BANNER_QUIET_GAP,
            None => start + BANNER_FIRST_BYTE_TIMEOUT,
        };
        let wait = deadline.saturating_duration_since(now);
        if wait.is_zero() {
            break;
        }
        match tokio::time::timeout(wait, channel.wait()).await {
            Err(_) => break,
            Ok(None) => break,
            Ok(Some(ChannelMsg::Data { data })) | Ok(Some(ChannelMsg::ExtendedData { data, .. })) => {
                out.push_str(&decode_stream(&mut pending, &data));
                last_data = Some(Instant::now());
            }
            Ok(Some(ChannelMsg::Eof)) | Ok(Some(ChannelMsg::Close)) => break,
            Ok(Some(_)) => {}
        }
    }

    // 收尾时残留在缓冲区里的多字节序列按宽松解码补上，避免丢掉末尾字符
    if !pending.is_empty() {
        out.push_str(&String::from_utf8_lossy(&pending));
    }
    out
}

/// 输出广播：先 emit（保持原行为与顺序不变），再异步落盘
fn publish_output(
    sink: &dyn ShellEventSink,
    event: &str,
    text: String,
    log_sink: &Arc<Mutex<Option<LogSink>>>,
    log_backpressure_warned: &mut bool,
    log_binary_warned: &mut bool,
) {
    if text.is_empty() {
        return;
    }
    match current_log_tx(log_sink) {
        Some(tx) => {
            sink.send(event, text.clone());
            push_log(&tx, &text, log_backpressure_warned, log_binary_warned);
        }
        None => sink.send(event, text),
    }
}

/// 启动 shell 输出泵：独占通道，一边转发远端输出，一边执行上游的写/改窗口/关闭指令。
fn spawn_shell_pump(
    mut channel: crate::conn::ShellChannel,
    mut rx: tokio_mpsc::Receiver<ShellCmd>,
    sink: Arc<dyn ShellEventSink>,
    stdout_event: String,
    closed_event: String,
    is_alive: Arc<AtomicBool>,
    log_sink: Arc<Mutex<Option<LogSink>>>,
) -> tokio::task::JoinHandle<()> {
    rt::spawn(async move {
        let mut pending: Vec<u8> = Vec::new();
        let mut exit_reason: Option<&'static str> = None;
        let mut log_backpressure_warned = false;
        let mut log_binary_warned = false;

        loop {
            tokio::select! {
                message = channel.wait() => match message {
                    Some(ChannelMsg::Data { data })
                    | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        // 按 UTF-8 边界解码，避免多字节字符被分片截断出现乱码
                        let text = decode_stream(&mut pending, &data);
                        publish_output(
                            sink.as_ref(),
                            &stdout_event,
                            text,
                            &log_sink,
                            &mut log_backpressure_warned,
                            &mut log_binary_warned,
                        );
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => {
                        exit_reason = Some("远端已关闭 Shell 会话");
                        break;
                    }
                    None => {
                        exit_reason = Some("SSH 连接已中断");
                        break;
                    }
                    _ => {}
                },
                command = rx.recv() => match command {
                    Some(ShellCmd::Write(bytes)) => {
                        if channel.data_bytes(bytes).await.is_err() {
                            exit_reason = Some("SSH 连接已中断");
                            break;
                        }
                    }
                    Some(ShellCmd::Resize { cols, rows }) => {
                        let _ = channel
                            .window_change(cols.max(1), rows.max(1), 0, 0)
                            .await;
                    }
                    // 会话被主动关闭：不再上报 closed 事件（前端已自行清理）
                    Some(ShellCmd::Close) | None => break,
                },
            }
        }

        if !pending.is_empty() {
            let tail = String::from_utf8_lossy(&pending).to_string();
            publish_output(
                sink.as_ref(),
                &stdout_event,
                tail,
                &log_sink,
                &mut log_backpressure_warned,
                &mut log_binary_warned,
            );
        }
        if let Some(reason) = exit_reason {
            is_alive.store(false, Ordering::Relaxed);
            sink.send(&closed_event, reason.to_string());
        }
    })
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
/// 真实连通性测试：TCP 建连 + SSH 握手 + 主机密钥校验 + 身份认证，返回真实耗时。
pub fn test_connection(config: &SshConnectConfig, prompter: Option<AuthPrompter>) -> Result<ConnectionTestResult, String> {
    let start = Instant::now();
    let mut config = config.clone();
    config.forwards.clear();
    let conn = match prompter {
        Some(prompter) => Conn::connect_prompted(&config, prompter)?,
        None => Conn::connect(&config)?,
    };
    let latency_ms = start.elapsed().as_millis() as u64;
    let host_key = conn.host_key().clone();
    // 测试连接不保留会话：立刻断开，避免在远端堆积空闲连接
    conn.disconnect();
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

pub fn probe_host_info(config: &SshConnectConfig, prompter: Option<AuthPrompter>) -> Result<HostProbeResult, String> {
    let start = Instant::now();
    let mut config = config.clone();
    config.forwards.clear();
    let conn = match prompter {
        Some(prompter) => Conn::connect_prompted(&config, prompter)?,
        None => Conn::connect(&config)?,
    };
    let ping_ms = start.elapsed().as_millis() as u64;

    // 探测脚本自带 0.4s 采样间隔，超时兜底避免远端不响应时线程永久挂起
    let output = conn.exec_with_timeout(PROBE_SCRIPT, Duration::from_secs(10))?;
    conn.disconnect();

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
    fn redact_drops_private_key_blocks() {
        let begin = format!("-----{} {} {}-----", "BEGIN", "OPENSSH", "PRIVATE KEY");
        let end = format!("-----{} {} {}-----", "END", "OPENSSH", "PRIVATE KEY");
        let text = format!("before\n{begin}\nabc\n{end}\nafter\n");
        let cleaned = redact(&text);
        assert!(cleaned.contains("before"));
        assert!(cleaned.contains("after"));
        assert!(!cleaned.contains("abc"));
        assert!(cleaned.contains("[redacted private key]"));
    }

    #[test]
    fn sanitize_host_replaces_invalid_filename_chars() {
        assert_eq!(sanitize_host("C:\\temp/foo"), "C__temp_foo");
        assert_eq!(sanitize_host("   "), "unknown-host");
        assert_eq!(sanitize_host("root@10.0.0.1"), "root@10.0.0.1");
    }

    #[test]
    fn should_roll_log_only_at_limit() {
        assert!(!should_roll_log(0));
        assert!(!should_roll_log(LOG_MAX_BYTES - 1));
        // 达到阈值即滚动（写入前判定，所以单文件最多超出一个 chunk）
        assert!(should_roll_log(LOG_MAX_BYTES));
        assert!(should_roll_log(LOG_MAX_BYTES + 1));
    }

    #[test]
    fn rolled_log_path_appends_numbered_suffix() {
        let path = Path::new("/tmp/logs/host/20240101_000000_abcd.log");
        let rolled = rolled_log_path(path);
        assert_eq!(
            rolled.file_name().unwrap(),
            std::ffi::OsStr::new("20240101_000000_abcd.log.1")
        );
        // 滚动文件仍要被 prune_session_logs 识别，否则清理会漏掉备份
        assert!(is_session_log_file(&rolled));
        assert!(is_session_log_file(path));
        assert!(!is_session_log_file(Path::new(
            "/tmp/logs/host/20240101_000000_abcd.txt"
        )));
    }

    #[test]
    fn reaching_limit_rolls_current_file() {
        let dir = std::env::temp_dir().join(format!("snail-ssh-log-roll-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("20240101_000000_abcd.log");
        let file = fs::File::create(&path).unwrap();

        let mut writer = SessionLogWriter::new(path.clone(), file);
        // 直接推到阈值，避免测试真的写 50MB
        writer.written = LOG_MAX_BYTES;
        assert!(writer.write_chunk("after roll\n"));
        assert_eq!(writer.written, "after roll\n".len() as u64);
        assert_eq!(fs::read_to_string(&path).unwrap(), "after roll\n");
        assert!(rolled_log_path(&path).is_file());

        drop(writer);
        let _ = fs::remove_dir_all(&dir);
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
    fn metrics_target_slot_holds_only_active_session() {
        let mut slot = MetricsTargetSlot::default();
        // 前端没设目标时后台不采样
        assert_eq!(slot.current(), None);

        slot.set(Some("ssh-a".into()));
        assert_eq!(slot.current(), Some("ssh-a"));

        // 切会话是覆盖而非排队：只采当前活动会话
        slot.set(Some("ssh-b".into()));
        assert_eq!(slot.current(), Some("ssh-b"));

        // 关监控清空目标 => 停止采样
        slot.set(None);
        assert_eq!(slot.current(), None);
    }

    #[test]
    fn metrics_target_slot_clears_only_for_matching_session() {
        let mut slot = MetricsTargetSlot::default();
        slot.set(Some("ssh-a".into()));

        // 关其他会话不影响当前采样目标
        assert!(!slot.clear_if("ssh-b"));
        assert_eq!(slot.current(), Some("ssh-a"));

        assert!(slot.clear_if("ssh-a"));
        assert_eq!(slot.current(), None);
        // 已清空后重复关闭同一会话不再算「清过」
        assert!(!slot.clear_if("ssh-a"));
    }

    #[test]
    fn metrics_error_is_reported_once_until_recovery() {
        let mut throttle = MetricsErrorThrottle::default();
        // 断开后同一错误每轮都会复现：只第一次打印
        assert!(throttle.should_report("会话已断开"));
        assert!(!throttle.should_report("会话已断开"));
        assert!(!throttle.should_report("会话已断开"));

        // 换了另一类错误仍应可见
        assert!(throttle.should_report("执行命令失败"));

        // 采样成功后复位，之后再次断开要重新告警
        throttle.clear();
        assert!(throttle.should_report("执行命令失败"));
    }

    #[test]
    fn metrics_sample_reuses_cached_snapshot_as_baseline() {
        let manager = SshManager::new();
        // 首次采样没有基线，CPU 无从计算
        let first = manager.apply_metrics_output(
            "ssh-a",
            "HOSTNAME:box\nCPU:cpu 100 0 100 800\n",
            "fallback",
        );
        assert_eq!(first.cpu, 0);
        assert_eq!(first.hostname, "box");

        // 第二次基于上一次快照：total 1000→1400、idle 800→1000 => 50%
        let second = manager.apply_metrics_output(
            "ssh-a",
            "HOSTNAME:box\nCPU:cpu 200 0 200 1000\n",
            "fallback",
        );
        assert_eq!(second.cpu, 50);

        // 基线按会话隔离：另一个会话拿不到 ssh-a 的快照
        let other = manager.apply_metrics_output(
            "ssh-b",
            "HOSTNAME:other\nCPU:cpu 200 0 200 1000\n",
            "fallback",
        );
        assert_eq!(other.cpu, 0);
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

/// 真实服务器上的 shell 输出泵验证（需要环境变量，默认 ignore）。
/// 覆盖：首屏抓取 / 按键写入 / 窗口调整 / 主动关闭，以及同连接上的 CWD 探测 exec。
#[cfg(test)]
mod live_tests {
    use super::*;

    #[derive(Default)]
    struct CollectSink {
        events: Mutex<Vec<(String, String)>>,
    }

    impl ShellEventSink for CollectSink {
        fn send(&self, event: &str, payload: String) {
            self.events.lock().push((event.to_string(), payload));
        }
    }

    impl CollectSink {
        fn joined(&self, event: &str) -> String {
            self.events
                .lock()
                .iter()
                .filter(|(name, _)| name == event)
                .map(|(_, payload)| payload.clone())
                .collect()
        }
    }

    fn live_config() -> Option<SshConnectConfig> {
        let host = std::env::var("SNAIL_TEST_HOST").ok()?;
        Some(SshConnectConfig {
            host,
            port: std::env::var("SNAIL_TEST_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(22),
            username: std::env::var("SNAIL_TEST_USER").unwrap_or_default(),
            auth_type: std::env::var("SNAIL_TEST_AUTH").unwrap_or_else(|_| "key".into()),
            password: std::env::var("SNAIL_TEST_PASSWORD").ok(),
            private_key: std::env::var("SNAIL_TEST_KEY").ok(),
            passphrase: None,
            trust_host_key: false,
            cols: 120,
            rows: 30,
            jumps: Vec::new(),
            proxy: None,
            forwards: Vec::new(),
        })
    }

    #[test]
    #[ignore]
    fn live_shell_pump_roundtrip() {
        let Some(config) = live_config() else {
            return;
        };
        let conn = Conn::connect(&config).expect("connect");
        let sink = Arc::new(CollectSink::default());

        let (tx, banner_cwd) = rt::block_on({
            let sink = Arc::clone(&sink);
            async {
                let mut channel = Conn::open_shell(conn.handle(), config.cols, config.rows)
                    .await
                    .expect("shell");
                let banner = read_initial_output(&mut channel).await;
                // 同一条连接上的 CWD 探测（自动跟随目录依赖它）
                let cwd = conn
                    .exec_async(CWD_DETECT_SCRIPT, crate::conn::EXEC_TIMEOUT)
                    .await
                    .expect("cwd probe");
                let (tx, rx) = tokio_mpsc::channel(SHELL_COMMAND_CAPACITY);
                spawn_shell_pump(
                    channel,
                    rx,
                    sink,
                    "stdout".into(),
                    "closed".into(),
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(Mutex::new(None)),
                );
                (tx, (banner, cwd))
            }
        });

        let (banner, cwd_output) = banner_cwd;
        println!("banner_len = {} cwd_output = {:?}", banner.len(), cwd_output);
        assert!(!banner.is_empty(), "首屏输出应抓到 MOTD 或提示符");
        let cwd = cwd_output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .last()
            .expect("cwd 探测无输出");
        assert!(cwd.starts_with('/'), "cwd 探测应返回绝对路径，实际 {}", cwd);

        // 写入：回显 + 标记
        tx.try_send(ShellCmd::Write(
            b"printf 'SNAIL_PUMP_%s\n' OK\n".to_vec(),
        ))
        .expect("send write");
        // 窗口调整：不应打断会话
        tx.try_send(ShellCmd::Resize {
            cols: 100,
            rows: 24,
        })
        .expect("send resize");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if sink.joined("stdout").contains("SNAIL_PUMP_OK") {
                break;
            }
            assert!(Instant::now() < deadline, "等待远端回显超时");
            std::thread::sleep(Duration::from_millis(50));
        }

        tx.try_send(ShellCmd::Close).expect("send close");
        std::thread::sleep(Duration::from_millis(200));
        println!("stdout = {:?}", sink.joined("stdout"));
        println!("closed = {:?}", sink.joined("closed"));
        conn.disconnect();
    }
}
