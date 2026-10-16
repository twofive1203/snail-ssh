use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 在线编辑器单次读取上限，避免把超大文件整个读进内存
const MAX_EDITABLE_FILE_SIZE: u64 = 8 * 1024 * 1024;
/// 传输块大小
const TRANSFER_CHUNK_SIZE: usize = 64 * 1024;
/// 进度事件最小上报间隔
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpFileItem {
    pub name: String,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    pub size: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    pub permissions: String,
    pub owner: String,
    pub group: String,
    #[serde(rename = "modifiedTime")]
    pub modified_time: String,
    pub editable: bool,
    #[serde(rename = "isHidden")]
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgressPayload {
    #[serde(rename = "taskId")]
    pub task_id: String,
    pub progress: u32,
    pub speed: String,
    pub status: String,
    pub transferred: u64,
    pub total: u64,
}

/// 传输进度统计：按采样窗口计算瞬时速度，并限制事件频率
struct ProgressReporter {
    task_id: String,
    total: u64,
    transferred: u64,
    window_started_at: Instant,
    window_bytes: u64,
}

impl ProgressReporter {
    fn new(task_id: &str, total: u64) -> Self {
        Self {
            task_id: task_id.to_string(),
            total,
            transferred: 0,
            window_started_at: Instant::now(),
            window_bytes: 0,
        }
    }

    fn advance(&mut self, n: u64) {
        self.transferred += n;
    }

    fn report(&mut self, app: &AppHandle, status: &str, force: bool) {
        let elapsed = self.window_started_at.elapsed();
        if !force && elapsed < PROGRESS_INTERVAL {
            return;
        }

        let seconds = elapsed.as_secs_f64().max(0.001);
        let bytes_in_window = self.transferred.saturating_sub(self.window_bytes);
        let speed = bytes_in_window as f64 / seconds;

        self.window_started_at = Instant::now();
        self.window_bytes = self.transferred;

        let progress = if self.total == 0 {
            100
        } else {
            ((self.transferred as f64 / self.total as f64) * 100.0).min(100.0) as u32
        };

        let _ = app.emit(
            &format!("sftp-progress-{}", self.task_id),
            TransferProgressPayload {
                task_id: self.task_id.clone(),
                progress,
                speed: format_speed(speed),
                status: status.to_string(),
                transferred: self.transferred,
                total: self.total,
            },
        );
    }
}

pub fn sftp_read_dir(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
) -> Result<Vec<SftpFileItem>, String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let mut entries = sftp
        .readdir(Path::new(remote_path))
        .map_err(|e| format!("读取远程目录失败: {}", e))?;

    let mut result = Vec::new();

    // 补充上级目录 ".."
    if remote_path != "/" && !remote_path.is_empty() {
        result.push(SftpFileItem {
            name: "..".into(),
            is_dir: true,
            size: "-".into(),
            size_bytes: 0,
            permissions: "drwxr-xr-x (755)".into(),
            owner: "-".into(),
            group: "-".into(),
            modified_time: "-".into(),
            editable: false,
            is_hidden: false,
        });
    }

    for (p, stat) in entries.drain(..) {
        let name = match p.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        if name == "." || name == ".." {
            continue;
        }

        let is_dir = stat.is_dir();
        let size_bytes = stat.size.unwrap_or(0);
        let perm = stat.perm.unwrap_or(0);

        result.push(SftpFileItem {
            name: name.clone(),
            is_dir,
            size: if is_dir {
                "-".into()
            } else {
                format_file_size(size_bytes)
            },
            size_bytes,
            permissions: format_permissions(is_dir, perm),
            owner: stat.uid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            group: stat.gid.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
            modified_time: stat
                .mtime
                .map(format_timestamp)
                .unwrap_or_else(|| "-".into()),
            editable: !is_dir && is_editable_file(&name),
            is_hidden: name.starts_with('.'),
        });
    }

    // 目录优先排序
    result.sort_by(|a, b| {
        if a.name == ".." {
            std::cmp::Ordering::Less
        } else if b.name == ".." {
            std::cmp::Ordering::Greater
        } else if a.is_dir && !b.is_dir {
            std::cmp::Ordering::Less
        } else if !a.is_dir && b.is_dir {
            std::cmp::Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok(result)
}

pub fn sftp_read_file(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
) -> Result<String, String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| e.to_string())?;
    let mut file = sftp
        .open(Path::new(remote_path))
        .map_err(|e| format!("打开远程文件失败: {}", e))?;

    let size = file.stat().map(|s| s.size.unwrap_or(0)).unwrap_or(0);
    if size > MAX_EDITABLE_FILE_SIZE {
        return Err(format!(
            "文件过大（{}），超过在线编辑器 {} 上限，请先下载到本地",
            format_file_size(size),
            format_file_size(MAX_EDITABLE_FILE_SIZE)
        ));
    }

    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|e| format!("读取文件失败: {}", e))?;

    Ok(String::from_utf8_lossy(&content).to_string())
}

pub fn sftp_write_file(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
    content: &str,
) -> Result<(), String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| e.to_string())?;
    let mut file = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("创建/覆盖远程文件失败: {}", e))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入远程文件失败: {}", e))?;

    Ok(())
}

pub fn sftp_mkdir(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
) -> Result<(), String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| e.to_string())?;
    sftp.mkdir(Path::new(remote_path), 0o755)
        .map_err(|e| format!("创建目录失败: {}", e))
}

pub fn sftp_remove_file(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
) -> Result<(), String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| e.to_string())?;
    remove_path_recursive(&sftp, Path::new(remote_path))
}

/// 递归删除：目录会先清空子项再删除自身。
/// 使用 lstat 而非 stat，避免把指向目录的软链接当成目录递归下去。
fn remove_path_recursive(sftp: &ssh2::Sftp, path: &Path) -> Result<(), String> {
    let stat = match sftp.lstat(path) {
        Ok(stat) => stat,
        // 已不存在（并发删除）视作成功
        Err(_) => return Ok(()),
    };

    if !stat.is_dir() {
        return sftp.unlink(path).map_err(|e| format!("删除失败: {}", e));
    }

    let entries = sftp
        .readdir(path)
        .map_err(|e| format!("读取待删目录失败: {}", e))?;
    for (child, _) in entries {
        match child.file_name() {
            Some(name) if name == "." || name == ".." => continue,
            Some(_) => remove_path_recursive(sftp, &child)?,
            None => continue,
        }
    }
    sftp.rmdir(path).map_err(|e| format!("删除目录失败: {}", e))
}

/// 远端登录用户的家目录，用作用户进入 SFTP 视图时的默认位置
pub fn sftp_home_dir(session_arc: Arc<Mutex<ssh2::Session>>) -> Result<String, String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| e.to_string())?;
    let home = sftp
        .realpath(Path::new("."))
        .map_err(|e| format!("获取远端家目录失败: {}", e))?;
    Ok(home.to_string_lossy().to_string())
}

pub fn sftp_upload_file(
    session_arc: Arc<Mutex<ssh2::Session>>,
    local_path: &str,
    remote_path: &str,
    task_id: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut local_file =
        File::open(local_path).map_err(|e| format!("打开本地文件失败: {}", e))?;
    let total_size = local_file.metadata().map(|m| m.len()).unwrap_or(0);

    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("创建远程文件失败: {}", e))?;

    let mut reporter = ProgressReporter::new(task_id, total_size);
    let mut buf = vec![0u8; TRANSFER_CHUNK_SIZE];

    loop {
        let n = local_file
            .read(&mut buf)
            .map_err(|e| format!("读取本地文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        remote_file
            .write_all(&buf[..n])
            .map_err(|e| format!("传输中断: {}", e))?;
        reporter.advance(n as u64);
        reporter.report(&app_handle, "running", false);
    }

    reporter.report(&app_handle, "completed", true);
    Ok(())
}

pub fn sftp_download_file(
    session_arc: Arc<Mutex<ssh2::Session>>,
    remote_path: &str,
    local_path: &str,
    task_id: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    let sess = session_arc.lock();
    let sftp = sess.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let mut remote_file = sftp
        .open(Path::new(remote_path))
        .map_err(|e| format!("打开远程文件失败: {}", e))?;
    let total_size = remote_file
        .stat()
        .map(|s| s.size.unwrap_or(0))
        .unwrap_or(0);

    let mut local_file =
        File::create(local_path).map_err(|e| format!("创建本地文件失败: {}", e))?;

    let mut reporter = ProgressReporter::new(task_id, total_size);
    let mut buf = vec![0u8; TRANSFER_CHUNK_SIZE];

    loop {
        let n = remote_file
            .read(&mut buf)
            .map_err(|e| format!("读取远程文件失败: {}", e))?;
        if n == 0 {
            break;
        }
        local_file
            .write_all(&buf[..n])
            .map_err(|e| format!("本地写入失败: {}", e))?;
        reporter.advance(n as u64);
        reporter.report(&app_handle, "running", false);
    }

    local_file
        .flush()
        .map_err(|e| format!("本地文件落盘失败: {}", e))?;
    reporter.report(&app_handle, "completed", true);
    Ok(())
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        format!("{:.0} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    }
}

fn format_permissions(is_dir: bool, perm: u32) -> String {
    let rwx = |p: u32| {
        format!(
            "{}{}{}",
            if p & 4 != 0 { 'r' } else { '-' },
            if p & 2 != 0 { 'w' } else { '-' },
            if p & 1 != 0 { 'x' } else { '-' }
        )
    };

    let user = rwx((perm >> 6) & 7);
    let group = rwx((perm >> 3) & 7);
    let other = rwx(perm & 7);
    let d = if is_dir { 'd' } else { '-' };
    let oct = perm & 0o777;

    format!("{}{}{}{} ({:o})", d, user, group, other, oct)
}

/// Unix 时间戳格式化为 UTC 的 "YYYY-MM-DD HH:MM"
fn format_timestamp(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let seconds_of_day = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year,
        month,
        day,
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60
    )
}

/// 天数（自 1970-01-01）转公历日期，见 Howard Hinnant 的 civil_from_days
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn is_editable_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".conf")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
        || lower.ends_with(".sh")
        || lower.ends_with(".bash")
        || lower.ends_with(".py")
        || lower.ends_with(".js")
        || lower.ends_with(".ts")
        || lower.ends_with(".txt")
        || lower.ends_with(".env")
        || lower.ends_with(".ini")
        || lower.ends_with(".toml")
        || lower.ends_with(".log")
}
