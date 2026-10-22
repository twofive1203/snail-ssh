use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::conn::Conn;
use crate::sftp_compat::{FileStat, OpenFlags, OpenType, RenameFlags, Sftp};

use crate::transfer::{resume_offset, TransferSignal, TRANSFER_CANCELLED};

/// 在线编辑器单次读取上限，避免把超大文件整个读进内存
const MAX_EDITABLE_FILE_SIZE: u64 = 8 * 1024 * 1024;
/// 上传在途窗口。russh-sftp 每次 write 内部按 32KB 分包、并发 16 个请求流水线发送，
/// 窗口要接近 SSH channel 窗口（常见 2MB）才喂得满；取 4MB 以覆盖略大的服务端窗口。
const UPLOAD_WINDOW_SIZE: usize = 4 * 1024 * 1024;
/// 每轮从磁盘补进窗口的上限。单次 write 会阻塞到新数据发出并收到一个 ACK，
/// 补数过大时暂停/取消要等这次调用返回。
const UPLOAD_FILL_CHUNK: usize = 256 * 1024;
/// 已消费前缀超过该值且不少于缓冲一半时压实，避免每次 ACK 都搬移整个窗口。
const UPLOAD_COMPACT_THRESHOLD: usize = 1024 * 1024;
/// 下载读缓冲。russh-sftp 对一次 read 会并发预取多个请求，1MB 对应数 MB 在途。
/// 暂停/取消要等这一次 read 返回，粒度就是这块缓冲。
const DOWNLOAD_READ_SIZE: usize = 1024 * 1024;
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
    session_arc: Arc<Conn>,
    remote_path: &str,
) -> Result<Vec<SftpFileItem>, String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
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

        result.push(build_file_item(&name, &stat));
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
    session_arc: Arc<Conn>,
    remote_path: &str,
) -> Result<String, String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| e.to_string())?;
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
    session_arc: Arc<Conn>,
    remote_path: &str,
    content: &str,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| e.to_string())?;
    let mut file = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("创建/覆盖远程文件失败: {}", e))?;

    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入远程文件失败: {}", e))?;

    Ok(())
}

pub fn sftp_mkdir(
    session_arc: Arc<Conn>,
    remote_path: &str,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| e.to_string())?;
    sftp.mkdir(Path::new(remote_path), 0o755)
        .map_err(|e| format!("创建目录失败: {}", e))
}

pub fn sftp_remove_file(
    session_arc: Arc<Conn>,
    remote_path: &str,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| e.to_string())?;
    remove_path_recursive(&sftp, Path::new(remote_path))
}

/// 递归删除：目录会先清空子项再删除自身。
/// 使用 lstat 而非 stat，避免把指向目录的软链接当成目录递归下去。
fn remove_path_recursive(sftp: &Sftp, path: &Path) -> Result<(), String> {
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

/// 家目录探测超时。SFTP 会话默认 30s，这里收短，避免打开文件区时被登录脚本卡住。
const HOME_PROBE_TIMEOUT_MS: u32 = 5_000;

/// 远端登录用户的家目录，用作文件工作区 / SFTP 视图的默认位置。
///
/// 优先读登录环境的 `$HOME`（并确认 SFTP 能访问该目录）。
/// 仅 SFTP、受限 shell 无法 exec 时，回退为子系统当前目录（OpenSSH 通常也是家目录，但部分服务端会停在 `/`）。
pub fn sftp_home_dir(session_arc: Arc<Conn>) -> Result<String, String> {
    let conn = session_arc.as_ref();
    if let Some(home) = probe_login_home(conn) {
        if let Ok(sftp) = conn.sftp() {
            if let Some(dir) = canonical_dir(&sftp, &home) {
                return Ok(dir);
            }
        }
    }
    let sftp = conn.sftp().map_err(|e| e.to_string())?;
    let cwd = sftp
        .realpath(Path::new("."))
        .map_err(|e| format!("获取远端家目录失败: {}", e))?;
    Ok(cwd.to_string_lossy().to_string())
}

/// 在同一条 SSH 连接上开瞬态 exec，读登录环境的 `$HOME`。
/// 失败返回 None，由调用方回退，不把文件区打开卡死。
fn probe_login_home(conn: &Conn) -> Option<String> {
    // 标记行：避免登录横幅或包装脚本把其它输出混进 stdout
    let output = conn
        .exec_with_timeout(
            "printf 'SNAIL_HOME:%s\n' \"$HOME\"",
            Duration::from_millis(HOME_PROBE_TIMEOUT_MS as u64),
        )
        .ok()?;
    parse_home_marker(&output)
}

fn parse_home_marker(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("SNAIL_HOME:") else {
            continue;
        };
        let home = rest.trim();
        if home.starts_with('/') && !home.contains('\0') {
            return Some(home.to_string());
        }
    }
    None
}

/// 把候选路径规范化，并确认它是 SFTP 可见的目录。
fn canonical_dir(sftp: &Sftp, path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }
    let canon = sftp
        .realpath(Path::new(path))
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| p.starts_with('/'))
        .unwrap_or_else(|| path.to_string());
    let stat = sftp.stat(Path::new(&canon)).ok()?;
    if stat.is_dir() {
        Some(canon)
    } else {
        None
    }
}

/// 由 SFTP 条目统计信息构造列表项（read_dir / stat 共用同一套格式化规则）
fn build_file_item(name: &str, stat: &FileStat) -> SftpFileItem {
    let is_dir = stat.is_dir();
    let size_bytes = stat.size.unwrap_or(0);
    let perm = stat.perm.unwrap_or(0);

    SftpFileItem {
        name: name.to_string(),
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
        editable: !is_dir && is_editable_file(name),
        is_hidden: name.starts_with('.'),
    }
}

/// 只填 uid/gid/perm 的 stat，size/时间保持 None，避免误改文件大小
fn metadata_patch(uid: Option<u32>, gid: Option<u32>, perm: Option<u32>) -> FileStat {
    FileStat {
        size: None,
        uid,
        gid,
        perm,
        atime: None,
        mtime: None,
    }
}

/// 重命名 / 移动。overwrite=false 时目标已存在会由远端报错，不静默覆盖。
pub fn sftp_rename(
    session_arc: Arc<Conn>,
    from: &str,
    to: &str,
    overwrite: bool,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let flags = if overwrite {
        RenameFlags::OVERWRITE
    } else {
        // 不传 ATOMIC/NATIVE：这两个只是 hint，部分服务端会直接拒绝；
        // flags=0 时目标已存在会由服务端报错，正是我们要的非覆盖语义。
        RenameFlags::empty()
    };
    sftp.rename(Path::new(from), Path::new(to), Some(flags))
        .map_err(|e| format!("重命名失败: {}", e))
}

/// 修改权限位（八进制，含 setuid/setgid/sticky 的低 12 位）
pub fn sftp_chmod(
    session_arc: Arc<Conn>,
    path: &str,
    mode: u32,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    sftp.setstat(Path::new(path), metadata_patch(None, None, Some(mode & 0o7777)))
        .map_err(|e| format!("修改权限失败: {}", e))
}

/// 修改属主 / 属组（数字 id，None 表示不改该项）。非 root 会返回远端原始权限错误。
pub fn sftp_chown(
    session_arc: Arc<Conn>,
    path: &str,
    uid: Option<u32>,
    gid: Option<u32>,
) -> Result<(), String> {
    if uid.is_none() && gid.is_none() {
        return Err("未指定新的属主或属组".into());
    }
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    sftp.setstat(Path::new(path), metadata_patch(uid, gid, None))
        .map_err(|e| format!("修改属主/属组失败: {}", e))
}

/// 新建空白文件。目标已存在时直接报错，不静默覆盖。
pub fn sftp_touch(
    session_arc: Arc<Conn>,
    path: &str,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    if sftp.lstat(Path::new(path)).is_ok() {
        return Err(format!("目标已存在: {}", path));
    }
    // 必须立即释放句柄，否则部分老 sshd 上文件句柄会泄漏到会话结束
    {
        let _file = sftp
            .create(Path::new(path))
            .map_err(|e| format!("创建文件失败: {}", e))?;
    }
    Ok(())
}

/// 单条目属性（供属性弹窗复用）
pub fn sftp_stat(
    session_arc: Arc<Conn>,
    path: &str,
) -> Result<SftpFileItem, String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let stat = sftp
        .stat(Path::new(path))
        .map_err(|e| format!("读取属性失败: {}", e))?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    Ok(build_file_item(&name, &stat))
}

/// 上传在途缓冲：前缀是尚未确认的字节，尾部是新读入的数据。
/// 每次 write 必须从「第一个未确认字节」开始，不能只传新增部分；否则库内部会跳过错误的前缀。
struct UploadWindow {
    buf: Vec<u8>,
    start: usize,
}

impl UploadWindow {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            start: 0,
        }
    }

    fn len(&self) -> usize {
        self.buf.len() - self.start
    }

    fn is_empty(&self) -> bool {
        self.start == self.buf.len()
    }

    fn as_slice(&self) -> &[u8] {
        &self.buf[self.start..]
    }

    fn extend_from_slice(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.compact_if_needed();
        self.buf.extend_from_slice(data);
    }

    /// 丢弃已确认前缀。调用方必须保证 n 不超过当前未确认长度。
    fn consume(&mut self, n: usize) {
        self.start += n;
        if self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
            return;
        }
        self.compact_if_needed();
    }

    fn compact_if_needed(&mut self) {
        if self.start >= UPLOAD_COMPACT_THRESHOLD && self.start * 2 >= self.buf.len() {
            self.buf.copy_within(self.start.., 0);
            self.buf.truncate(self.len());
            self.start = 0;
        }
    }
}

/// 块循环中的暂停/取消检查点：暂停时阻塞轮询，取消时上报并返回哨兵错误。
fn pause_or_cancel(
    signal: &TransferSignal,
    reporter: &mut ProgressReporter,
    app: &AppHandle,
) -> Result<(), String> {
    if signal.is_cancelled() {
        reporter.report(app, "cancelled", true);
        return Err(TRANSFER_CANCELLED.into());
    }
    if signal.is_paused() {
        reporter.report(app, "paused", true);
        if !signal.wait_if_paused() {
            reporter.report(app, "cancelled", true);
            return Err(TRANSFER_CANCELLED.into());
        }
    }
    Ok(())
}

/// 上传单个文件（供单文件与目录上传复用）：支持取消/暂停与断点续传。
/// 续传起点按"远端已存在且比本地源短"计算；`reporter` 的 total 为完整文件大小，
/// 续传时 `advance(offset)` 让进度从已有位置起算。
fn transfer_upload_one(
    sftp: &Sftp,
    local: &Path,
    remote: &Path,
    reporter: &mut ProgressReporter,
    signal: &TransferSignal,
    app: &AppHandle,
) -> Result<(), String> {
    let mut local_file = File::open(local).map_err(|e| format!("打开本地文件失败: {}", e))?;
    let source_len = local_file.metadata().map(|m| m.len()).unwrap_or(0);
    let remote_len = sftp
        .stat(remote)
        .ok()
        .and_then(|stat| stat.size);
    let offset = resume_offset(remote_len, source_len);

    // 红线：续传路径绝不能带 OpenFlags::TRUNC，否则会把已传部分清空；
    // offset == 0 时用 create()（语义等于截断重建），正确处理远端更长/需重传的情况。
    let mut remote_file = if offset == 0 {
        sftp.create(remote)
            .map_err(|e| format!("创建远程文件失败: {}", e))?
    } else {
        sftp.open_mode(
            remote,
            OpenFlags::WRITE | OpenFlags::CREATE,
            0o644,
            OpenType::File,
        )
        .map_err(|e| format!("打开远程文件失败: {}", e))?
    };

    if offset > 0 {
        local_file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("定位本地文件失败: {}", e))?;
        remote_file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("定位远程文件失败: {}", e))?;
        reporter.advance(offset);
    }

    // 不能用 write_all 按块等待：它会把当前块全部 ACK 完才读下一块，窗口在块边界被排空。
    // 这里保持约 4MB 未确认数据，每次只消费已 ACK 的前缀并立刻补新数据。
    // 进度只计已确认字节；续传仍以远端文件长度为准，不把尚未 ACK 的数据算进已传。
    let mut window = UploadWindow::with_capacity(UPLOAD_WINDOW_SIZE * 2);
    let mut read_buf = vec![0u8; UPLOAD_FILL_CHUNK];
    let mut local_eof = false;
    loop {
        pause_or_cancel(signal, reporter, app)?;

        if !local_eof {
            let room = UPLOAD_WINDOW_SIZE.saturating_sub(window.len());
            // 窗口快满时不补几十字节，等腾出至少一块的 1/4 再读盘。
            if room >= UPLOAD_FILL_CHUNK / 4 {
                let to_read = room.min(read_buf.len());
                let n = local_file
                    .read(&mut read_buf[..to_read])
                    .map_err(|e| format!("读取本地文件失败: {}", e))?;
                if n == 0 {
                    local_eof = true;
                } else {
                    window.extend_from_slice(&read_buf[..n]);
                }
            }
        }

        if window.is_empty() {
            break;
        }

        let n = remote_file
            .write(window.as_slice())
            .map_err(|e| format!("传输中断: {}", e))?;
        if n == 0 || n > window.len() {
            return Err("传输中断: 远端确认的字节数异常".into());
        }
        window.consume(n);
        reporter.advance(n as u64);
        signal.set_transferred(reporter.transferred);
        reporter.report(app, "running", false);
    }
    Ok(())
}

/// 下载单个文件（供单文件与目录下载复用）：支持取消/暂停与断点续传。
fn transfer_download_one(
    sftp: &Sftp,
    remote: &Path,
    local: &Path,
    reporter: &mut ProgressReporter,
    signal: &TransferSignal,
    app: &AppHandle,
) -> Result<(), String> {
    let mut remote_file = sftp
        .open(remote)
        .map_err(|e| format!("打开远程文件失败: {}", e))?;
    let remote_len = remote_file
        .stat()
        .map(|s| s.size.unwrap_or(0))
        .unwrap_or(0);
    let local_len = fs::metadata(local).ok().map(|m| m.len());
    let offset = resume_offset(local_len, remote_len);

    // offset == 0 时截断重建，避免本地残留旧文件的尾巴；offset > 0 时保留已下部分续写。
    let mut local_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .open(local)
        .map_err(|e| format!("创建本地文件失败: {}", e))?;

    if offset > 0 {
        remote_file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("定位远程文件失败: {}", e))?;
        local_file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("定位本地文件失败: {}", e))?;
        reporter.advance(offset);
    }

    // 读缓冲越大，SFTP 层预读越深。预读状态跨 read 调用保留，不会在块边界排空。
    let mut buf = vec![0u8; DOWNLOAD_READ_SIZE];
    loop {
        pause_or_cancel(signal, reporter, app)?;
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
        signal.set_transferred(reporter.transferred);
        reporter.report(app, "running", false);
    }

    local_file
        .flush()
        .map_err(|e| format!("本地文件落盘失败: {}", e))?;
    Ok(())
}

pub fn sftp_upload_file(
    session_arc: Arc<Conn>,
    local_path: &str,
    remote_path: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let total_size = fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;

    let mut reporter = ProgressReporter::new(task_id, total_size);
    transfer_upload_one(
        &sftp,
        Path::new(local_path),
        Path::new(remote_path),
        &mut reporter,
        &signal,
        &app_handle,
    )?;
    reporter.report(&app_handle, "completed", true);
    Ok(())
}

pub fn sftp_download_file(
    session_arc: Arc<Conn>,
    remote_path: &str,
    local_path: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
    let total_size = sftp
        .stat(Path::new(remote_path))
        .ok()
        .and_then(|s| s.size)
        .unwrap_or(0);

    let mut reporter = ProgressReporter::new(task_id, total_size);
    transfer_download_one(
        &sftp,
        Path::new(remote_path),
        Path::new(local_path),
        &mut reporter,
        &signal,
        &app_handle,
    )?;
    reporter.report(&app_handle, "completed", true);
    Ok(())
}

/// 统一下载入口：按远端条目类型分派单文件 / 目录（前端无需判断类型）。
pub fn sftp_download_path(
    session_arc: Arc<Conn>,
    remote_path: &str,
    local_path: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let is_dir = {
        let conn = session_arc.as_ref();
        let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
        sftp.stat(Path::new(remote_path))
            .map(|s| s.is_dir())
            .map_err(|e| format!("读取远程信息失败: {}", e))?
    };
    if is_dir {
        sftp_download_dir(session_arc, remote_path, local_path, task_id, signal, app_handle)
    } else {
        sftp_download_file(session_arc, remote_path, local_path, task_id, signal, app_handle)
    }
}

/// 待下载的单个文件（远端路径 -> 本地路径）
struct DownloadEntry {
    remote: PathBuf,
    local: PathBuf,
}

/// 目录递归下载：保留远端目录结构，进度按实际会传输的字节总数统计。
pub fn sftp_download_dir(
    session_arc: Arc<Conn>,
    remote_path: &str,
    local_path: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;

    // 先枚举待传输文件并统计总字节数，否则进度条无法给出百分比。
    // 复用同一份列表，避免统计与传输两套递归逻辑出现分歧。
    let mut entries = Vec::new();
    let mut total = 0u64;
    collect_download_entries(
        &sftp,
        Path::new(remote_path),
        Path::new(local_path),
        &mut entries,
        &mut total,
    )?;

    // 空目录或目录软链接被全部跳过时，也要保证本地目录被创建出来
    fs::create_dir_all(Path::new(local_path)).map_err(|e| format!("创建本地目录失败: {}", e))?;

    let mut reporter = ProgressReporter::new(task_id, total);

    for entry in &entries {
        // 本地已与远端等大：视为已续传完成，直接计入进度避免进度回退
        let remote_len = sftp.stat(&entry.remote).ok().and_then(|s| s.size);
        if let (Some(remote_len), Ok(meta)) = (remote_len, fs::metadata(&entry.local)) {
            if remote_len > 0 && meta.len() == remote_len {
                reporter.advance(remote_len);
                continue;
            }
        }
        if let Some(parent) = entry.local.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建本地目录失败: {}", e))?;
        }
        transfer_download_one(
            &sftp,
            &entry.remote,
            &entry.local,
            &mut reporter,
            &signal,
            &app_handle,
        )?;
    }

    reporter.report(&app_handle, "completed", true);
    Ok(())
}

/// 递归枚举目录下需要下载的普通文件。
/// 只跟随指向普通文件的软链接；指向目录的软链接与 fifo/socket/设备文件均跳过，
/// 前者避免递归成环，后者避免 open 时无限阻塞。
fn collect_download_entries(
    sftp: &Sftp,
    remote: &Path,
    local: &Path,
    out: &mut Vec<DownloadEntry>,
    total: &mut u64,
) -> Result<(), String> {
    let lstat = sftp
        .lstat(remote)
        .map_err(|e| format!("读取远端信息失败: {}", e))?;
    let file_type = lstat.file_type();

    if file_type.is_symlink() {
        match sftp.stat(remote) {
            Ok(target) if target.is_file() => {
                *total += target.size.unwrap_or(0);
                out.push(DownloadEntry {
                    remote: remote.to_path_buf(),
                    local: local.to_path_buf(),
                });
            }
            Ok(_) => eprintln!("[snail-ssh] 跳过目录软链接: {}", remote.display()),
            Err(e) => eprintln!(
                "[snail-ssh] 软链接无法解析，已跳过 {}: {}",
                remote.display(),
                e
            ),
        }
        return Ok(());
    }

    if file_type.is_dir() {
        let entries = sftp
            .readdir(remote)
            .map_err(|e| format!("读取远端目录失败: {}", e))?;
        for (child, _) in entries {
            let name = match child.file_name() {
                Some(name) if name == "." || name == ".." => continue,
                Some(name) => name.to_os_string(),
                None => continue,
            };
            collect_download_entries(sftp, &child, &local.join(name), out, total)?;
        }
        return Ok(());
    }

    if file_type.is_file() {
        *total += lstat.size.unwrap_or(0);
        out.push(DownloadEntry {
            remote: remote.to_path_buf(),
            local: local.to_path_buf(),
        });
        return Ok(());
    }

    eprintln!("[snail-ssh] 跳过特殊文件: {}", remote.display());
    Ok(())
}

/// 待上传的单个文件（本地路径 -> 远端路径）
struct UploadEntry {
    local: PathBuf,
    remote: PathBuf,
}

/// 统一上传入口：按本地元数据自动分派单文件 / 目录，前端无需判断类型。
/// `remote_dir` 为目标父目录，文件或目录都以自身的 basename 落进去。
pub fn sftp_upload_path(
    session_arc: Arc<Conn>,
    local_path: &str,
    remote_dir: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let meta = fs::metadata(local_path).map_err(|e| format!("读取本地路径失败: {}", e))?;
    if meta.is_dir() {
        return sftp_upload_dir(
            session_arc,
            local_path,
            remote_dir,
            task_id,
            signal,
            app_handle,
        );
    }
    let name = Path::new(local_path)
        .file_name()
        .ok_or_else(|| "无法确定本地文件名".to_string())?;
    let remote_path = Path::new(remote_dir).join(name);
    sftp_upload_file(
        session_arc,
        local_path,
        &remote_path.to_string_lossy(),
        task_id,
        signal,
        app_handle,
    )
}

/// 目录递归上传：保留本地目录结构，进度按实际会传输的字节总数统计。
pub fn sftp_upload_dir(
    session_arc: Arc<Conn>,
    local_path: &str,
    remote_dir: &str,
    task_id: &str,
    signal: Arc<TransferSignal>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let local_root = Path::new(local_path);
    let base_name = local_root
        .file_name()
        .ok_or_else(|| "无法确定本地目录名称".to_string())?;
    let remote_root = Path::new(remote_dir).join(base_name);

    let conn = session_arc.as_ref();
    let sftp = conn.sftp().map_err(|e| format!("初始化 SFTP 失败: {}", e))?;

    // 大目录枚举可能耗时：先发一条 scanning 事件，避免用户误以为卡死
    let _ = app_handle.emit(
        &format!("sftp-progress-{}", task_id),
        TransferProgressPayload {
            task_id: task_id.to_string(),
            progress: 0,
            speed: "扫描本地目录...".into(),
            status: "scanning".into(),
            transferred: 0,
            total: 0,
        },
    );

    // 先枚举待上传文件并统计总字节数，否则进度条无法给出百分比
    let mut entries = Vec::new();
    let mut total = 0u64;
    collect_upload_entries(local_root, &remote_root, &mut entries, &mut total)?;

    ensure_remote_dir(&sftp, &remote_root)?;

    let mut reporter = ProgressReporter::new(task_id, total);

    for entry in &entries {
        if let Some(parent) = entry.remote.parent() {
            ensure_remote_dir(&sftp, parent)?;
        }
        transfer_upload_one(
            &sftp,
            &entry.local,
            &entry.remote,
            &mut reporter,
            &signal,
            &app_handle,
        )?;
    }

    reporter.report(&app_handle, "completed", true);
    Ok(())
}

/// 递归枚举本地目录下需要上传的普通文件。
/// 软链接策略与下载保持对称：只跟随指向普通文件的软链接，目录软链接跳过（防递归成环）。
fn collect_upload_entries(
    local: &Path,
    remote: &Path,
    out: &mut Vec<UploadEntry>,
    total: &mut u64,
) -> Result<(), String> {
    let meta = fs::symlink_metadata(local).map_err(|e| format!("读取本地信息失败: {}", e))?;
    let file_type = meta.file_type();

    if file_type.is_symlink() {
        match fs::metadata(local) {
            Ok(target) if target.is_file() => {
                *total += target.len();
                out.push(UploadEntry {
                    local: local.to_path_buf(),
                    remote: remote.to_path_buf(),
                });
            }
            Ok(_) => eprintln!("[snail-ssh] 跳过目录软链接: {}", local.display()),
            Err(e) => eprintln!(
                "[snail-ssh] 软链接无法解析，已跳过 {}: {}",
                local.display(),
                e
            ),
        }
        return Ok(());
    }

    if file_type.is_dir() {
        let entries = fs::read_dir(local).map_err(|e| format!("读取本地目录失败: {}", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取本地目录失败: {}", e))?;
            collect_upload_entries(&entry.path(), &remote.join(entry.file_name()), out, total)?;
        }
        return Ok(());
    }

    if file_type.is_file() {
        *total += meta.len();
        out.push(UploadEntry {
            local: local.to_path_buf(),
            remote: remote.to_path_buf(),
        });
        return Ok(());
    }

    eprintln!("[snail-ssh] 跳过特殊文件: {}", local.display());
    Ok(())
}

/// 逐级创建远端目录（ssh2 没有 mkdir -p），已存在则忽略。
fn ensure_remote_dir(sftp: &Sftp, dir: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in dir.components() {
        match component {
            std::path::Component::RootDir => current.push("/"),
            std::path::Component::Normal(seg) => {
                current.push(seg);
                if sftp.stat(&current).map(|s| s.is_dir()).unwrap_or(false) {
                    continue;
                }
                if let Err(e) = sftp.mkdir(&current, 0o755) {
                    // 并发或目录已存在时忽略该错误
                    if !sftp.stat(&current).map(|s| s.is_dir()).unwrap_or(false) {
                        return Err(format!("创建远端目录失败 {}: {}", current.display(), e));
                    }
                }
            }
            _ => {}
        }
    }
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
    // Dockerfile / .env / .env.local 没有统一后缀，按文件名认
    if lower == "dockerfile"
        || lower.starts_with("dockerfile.")
        || lower == ".env"
        || lower.starts_with(".env.")
    {
        return true;
    }
    const EXTS: &[&str] = &[
        ".conf", ".yaml", ".yml", ".json", ".sh", ".bash", ".py", ".js", ".ts",
        ".txt", ".env", ".ini", ".toml", ".log", ".xml", ".html", ".htm", ".css",
        ".properties", ".cfg", ".cnf", ".service",
    ];
    EXTS.iter().any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::{is_editable_file, parse_home_marker};

    #[test]
    fn editable_common_configs() {
        assert!(is_editable_file("nginx.conf"));
        assert!(is_editable_file("app.yaml"));
        assert!(is_editable_file("site.xml"));
        assert!(is_editable_file("index.html"));
        assert!(is_editable_file("Dockerfile"));
        assert!(is_editable_file(".env"));
        assert!(is_editable_file(".env.local"));
        assert!(is_editable_file("app.service"));
        assert!(!is_editable_file("backup.tar.gz"));
        assert!(!is_editable_file("ssl-bundle.crt"));
    }

    #[test]
    fn home_marker_ignores_banner_and_relative_paths() {
        assert_eq!(
            parse_home_marker("SNAIL_HOME:/home/demo\n"),
            Some("/home/demo".into())
        );
        assert_eq!(
            parse_home_marker("Last login\nSNAIL_HOME:/root\n"),
            Some("/root".into())
        );
        assert_eq!(parse_home_marker("SNAIL_HOME:\n"), None);
        assert_eq!(parse_home_marker("SNAIL_HOME:relative\n"), None);
        assert_eq!(parse_home_marker("not a marker\n"), None);
    }

    #[test]
    fn upload_window_keeps_unacked_prefix_and_compacts() {
        let mut window = super::UploadWindow::with_capacity(64);
        window.extend_from_slice(b"abcdefghij");
        window.consume(3);
        assert_eq!(window.as_slice(), b"defghij");
        window.extend_from_slice(b"XYZ");
        assert_eq!(window.as_slice(), b"defghijXYZ");
        window.consume(window.len());
        assert!(window.is_empty());
        window.extend_from_slice(b"next");
        assert_eq!(window.as_slice(), b"next");

        let chunk = vec![0xABu8; super::UPLOAD_FILL_CHUNK];
        while window.len() < super::UPLOAD_WINDOW_SIZE {
            window.extend_from_slice(&chunk);
        }
        let keep = 16;
        let tail = window.as_slice()[window.len() - keep..].to_vec();
        window.consume(window.len() - keep);
        assert_eq!(window.as_slice(), tail.as_slice());
        assert_eq!(window.start, 0);
    }
}
