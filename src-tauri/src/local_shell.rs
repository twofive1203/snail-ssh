//! 本机交互式终端。和 SSH 会话分开：不进 `SshManager`，也不写主机资产。
//!
//! Windows 用 ConPTY，Unix 用伪终端。`portable-pty` 0.8 不改编码：ConPTY 输出管道
//! 本身是 UTF-8（控制台 UTF-16 API 在进管道前已被转成 VT），所以读线程只按 UTF-8
//! 边界切块。Windows PowerShell 5.1 和 cmd 仍可能走 OEM 代码页，这是降级路径，不在这里转码。

use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// attach 之前的输出上限。超出后丢掉最旧的字节，并在取走时加上截断标记。
const OUTPUT_BUFFER_LIMIT: usize = 256 * 1024;
const TRUNC_MARKER: &str = "\r\n[snail-ssh] 本地终端启动输出过长，已截断较早的内容\r\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    Unix,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocalSessionRequest {
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub cols: u32,
    #[serde(default)]
    pub rows: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocalSessionResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub label: String,
    pub kind: String,
    pub program: String,
}

struct ResolvedShell {
    program: PathBuf,
    args: Vec<String>,
    cwd: PathBuf,
    label: String,
    kind: String,
}

struct LaunchProbe {
    lookup: Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>,
    is_exec: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    is_dir: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    is_file: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    conventional_pwsh: Vec<PathBuf>,
    shell_env: Option<String>,
    home: Option<PathBuf>,
    platform: Platform,
}

trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: String);
}

impl EventSink for AppHandle {
    fn emit(&self, event: &str, payload: String) {
        let _ = Emitter::emit(self, event, payload);
    }
}

struct ByteBuffer {
    data: Vec<u8>,
    truncated: bool,
    limit: usize,
}

impl ByteBuffer {
    fn new(limit: usize) -> Self {
        Self {
            data: Vec::new(),
            truncated: false,
            limit,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if chunk.is_empty() || self.limit == 0 {
            return;
        }
        self.data.extend_from_slice(chunk);
        if self.data.len() <= self.limit {
            return;
        }
        let overflow = self.data.len() - self.limit;
        self.data.drain(..overflow);
        while matches!(self.data.first(), Some(b) if b & 0b1100_0000 == 0b1000_0000) {
            self.data.remove(0);
        }
        self.truncated = true;
    }

    fn take_string(&mut self) -> String {
        let bytes = std::mem::take(&mut self.data);
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if self.truncated {
            text.insert_str(0, TRUNC_MARKER);
            self.truncated = false;
        }
        text
    }
}

/// 实时输出按完整 UTF-8 切出去，半个汉字留到下一块。
struct Utf8Splitter {
    pending: Vec<u8>,
}

impl Utf8Splitter {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

    fn push(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        let mut out = String::new();
        loop {
            if self.pending.is_empty() {
                break;
            }
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    break;
                }
                Err(err) => {
                    let valid = err.valid_up_to();
                    if valid > 0 {
                        let bytes: Vec<u8> = self.pending.drain(..valid).collect();
                        out.push_str(&String::from_utf8(bytes).unwrap_or_default());
                        continue;
                    }
                    if let Some(len) = err.error_len() {
                        self.pending.drain(..len);
                        out.push('\u{FFFD}');
                        continue;
                    }
                    // 开头是尚未收齐的多字节序列
                    break;
                }
            }
        }
        out
    }
}

struct GateState {
    attached: bool,
    user_closed: bool,
    close_noted: bool,
    buffer: ByteBuffer,
    codec: Utf8Splitter,
    pending_close: Option<String>,
}

impl GateState {
    fn new(limit: usize) -> Self {
        Self {
            attached: false,
            user_closed: false,
            close_noted: false,
            buffer: ByteBuffer::new(limit),
            codec: Utf8Splitter::new(),
            pending_close: None,
        }
    }

    /// 已 attach 时返回要广播的文本；否则只进缓冲。
    fn push_bytes(&mut self, chunk: &[u8]) -> Option<String> {
        if self.attached {
            let text = self.codec.push(chunk);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        } else {
            self.buffer.push(chunk);
            None
        }
    }

    fn take(&mut self) -> (String, Option<String>) {
        if self.attached {
            return (String::new(), None);
        }
        self.attached = true;
        let text = self.buffer.take_string();
        let closed = self.pending_close.take();
        (text, closed)
    }

    fn note_exit(&mut self, reason: String) -> Option<String> {
        if self.user_closed || self.close_noted {
            return None;
        }
        self.close_noted = true;
        if self.attached {
            Some(reason)
        } else {
            self.pending_close = Some(reason);
            None
        }
    }

    fn mark_user_closed(&mut self) {
        self.user_closed = true;
        self.close_noted = true;
        self.pending_close = None;
    }
}

struct PtyIo {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

#[cfg(windows)]
struct JobGuard {
    handle: winapi::shared::ntdef::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for JobGuard {}

#[cfg(windows)]
impl JobGuard {
    fn assign(process: std::os::windows::io::RawHandle) -> std::io::Result<Self> {
        use winapi::shared::ntdef::HANDLE;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::jobapi2::{AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject};
        use winapi::um::winnt::{
            JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of_val(&info) as u32,
            );
            if ok == 0 {
                CloseHandle(job);
                return Err(std::io::Error::last_os_error());
            }
            if AssignProcessToJobObject(job, process as HANDLE) == 0 {
                let err = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(err);
            }
            Ok(Self { handle: job })
        }
    }

    fn terminate(&self) {
        unsafe {
            winapi::um::jobapi2::TerminateJobObject(self.handle, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for JobGuard {
    fn drop(&mut self) {
        unsafe {
            winapi::um::jobapi2::TerminateJobObject(self.handle, 1);
            winapi::um::handleapi::CloseHandle(self.handle);
        }
    }
}

struct Session {
    alive: AtomicBool,
    label: String,
    gate: Mutex<GateState>,
    io: Mutex<PtyIo>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    killer: Mutex<Option<Box<dyn ChildKiller + Send + Sync>>>,
    #[cfg(windows)]
    job: Mutex<Option<JobGuard>>,
    /// Unix 关进程组要用。Windows 上只给测试核对父子进程，正常路径不读。
    #[allow(dead_code)]
    pid: Option<u32>,
    log: Mutex<Option<crate::ssh::SessionLogHandle>>,
    sink: Arc<dyn EventSink>,
    stdout_event: String,
    closed_event: String,
}

#[derive(Clone)]
pub struct LocalShellManager {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
}

impl LocalShellManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_session(
        &self,
        request: CreateLocalSessionRequest,
        app: AppHandle,
    ) -> Result<CreateLocalSessionResponse, String> {
        self.create_inner(request, system_probe(), Arc::new(app))
    }

    fn create_inner(
        &self,
        request: CreateLocalSessionRequest,
        probe: LaunchProbe,
        sink: Arc<dyn EventSink>,
    ) -> Result<CreateLocalSessionResponse, String> {
        let resolved = prepare_launch(&request, &probe)?;
        let (spawned, reader) = spawn_pty_with_reader(&resolved, request.cols, request.rows)?;
        let session_id = format!("local-{}", Uuid::new_v4());
        let stdout_event = format!("local-stdout-{session_id}");
        let closed_event = format!("local-closed-{session_id}");
        let label = resolved.label.clone();
        let kind = resolved.kind.clone();
        let program = resolved.program.display().to_string();
        let session = Arc::new(Session {
            alive: AtomicBool::new(true),
            label: label.clone(),
            gate: Mutex::new(GateState::new(OUTPUT_BUFFER_LIMIT)),
            io: Mutex::new(PtyIo {
                writer: spawned.writer,
                master: spawned.master,
            }),
            child: Mutex::new(spawned.child),
            killer: Mutex::new(Some(spawned.killer)),
            #[cfg(windows)]
            job: Mutex::new(Some(spawned.job)),
            pid: spawned.pid,
            log: Mutex::new(None),
            sink: Arc::clone(&sink),
            stdout_event,
            closed_event,
        });
        self.sessions.lock().insert(session_id.clone(), Arc::clone(&session));
        start_reader_with(Arc::clone(&session), reader);
        // ConPTY 在子进程退出后仍可能握着输出管道，直到 stdin 被关掉。
        // 另起线程等进程结束，再丢掉 writer，读线程才能收到 EOF 并上报退出。
        watch_exit(Arc::clone(&session));
        Ok(CreateLocalSessionResponse {
            session_id,
            label,
            kind,
            program,
        })
    }

    pub fn write_session(&self, session_id: &str, data: &str) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        let session = self.get(session_id)?;
        if !session.alive.load(Ordering::SeqCst) {
            return Err("会话已退出".into());
        }
        let mut io = session.io.lock();
        io.writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("写入失败: {e}"))?;
        let _ = io.writer.flush();
        Ok(())
    }

    pub fn resize_session(&self, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
        let session = self.get(session_id)?;
        if !session.alive.load(Ordering::SeqCst) {
            return Err("会话已退出".into());
        }
        let io = session.io.lock();
        io.master
            .resize(pty_size(cols, rows))
            .map_err(|e| format!("调整窗口失败: {e}"))
    }

    pub fn take_banner(&self, session_id: &str) -> Result<String, String> {
        let session = self.get(session_id)?;
        let (text, closed) = session.gate.lock().take();
        if let Some(reason) = closed {
            session.sink.emit(&session.closed_event, reason);
        }
        Ok(text)
    }

    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        let Some(session) = self.sessions.lock().remove(session_id) else {
            return Ok(());
        };
        session.shutdown();
        Ok(())
    }

    pub fn set_logging(
        &self,
        session_id: &str,
        enabled: bool,
        label: &str,
    ) -> Result<Option<String>, String> {
        let session = self.get(session_id)?;
        if !enabled {
            let removed = session.log.lock().take();
            return match removed {
                Some(handle) => {
                    let path = handle.path.display().to_string();
                    handle.shutdown();
                    Ok(Some(path))
                }
                None => Ok(None),
            };
        }
        let existing = session
            .log
            .lock()
            .as_ref()
            .map(|handle| handle.path.display().to_string());
        if let Some(path) = existing {
            return Ok(Some(path));
        }
        let mut short: String = session_id.trim_start_matches("local-").chars().take(8).collect();
        if short.is_empty() {
            short = "local".into();
        }
        let header = if label.trim().is_empty() {
            session.label.as_str()
        } else {
            label
        };
        let handle = crate::ssh::start_session_log("local", header, &short)?;
        let path = handle.path.display().to_string();
        *session.log.lock() = Some(handle);
        Ok(Some(path))
    }

    pub fn log_path(&self, session_id: &str) -> Result<Option<String>, String> {
        let session = self.get(session_id)?;
        let path = session
            .log
            .lock()
            .as_ref()
            .map(|handle| handle.path.display().to_string());
        Ok(path)
    }

    fn get(&self, session_id: &str) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .get(session_id)
            .cloned()
            .ok_or_else(|| "会话不存在或已关闭".to_string())
    }

    #[cfg(test)]
    fn session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    #[cfg(test)]
    fn is_alive(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .get(session_id)
            .map(|session| session.alive.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    #[cfg(test)]
    fn process_id(&self, session_id: &str) -> Option<u32> {
        self.sessions.lock().get(session_id).and_then(|session| session.pid)
    }
}

impl Session {
    fn shutdown(&self) {
        self.gate.lock().mark_user_closed();
        self.alive.store(false, Ordering::SeqCst);
        self.kill_tree();
        if let Some(handle) = self.log.lock().take() {
            handle.shutdown();
        }
    }

    fn kill_tree(&self) {
        #[cfg(windows)]
        if let Some(job) = self.job.lock().take() {
            job.terminate();
        }
        #[cfg(unix)]
        if let Some(pid) = self.pid {
            if pid > 1 {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
        if let Some(mut killer) = self.killer.lock().take() {
            let _ = killer.kill();
        }
    }

    fn finish(&self, reason: String) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(reason) = self.gate.lock().note_exit(reason) {
            self.sink.emit(&self.closed_event, reason);
        }
    }
}

struct SpawnedPty {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    #[cfg(windows)]
    job: JobGuard,
    pid: Option<u32>,
}

fn format_exit(status: &portable_pty::ExitStatus) -> String {
    format!("进程已退出（退出码 {}）", status.exit_code())
}

fn pty_size(cols: u32, rows: u32) -> PtySize {
    PtySize {
        cols: dim(cols, 80),
        rows: dim(rows, 24),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn dim(value: u32, default: u16) -> u16 {
    if value == 0 {
        default
    } else {
        value.min(1000) as u16
    }
}

/// 读端在 spawn 时取走。`take_writer` 之后 master 仍负责 resize。
fn spawn_pty_with_reader(
    resolved: &ResolvedShell,
    cols: u32,
    rows: u32,
) -> Result<(SpawnedPty, Box<dyn Read + Send>), String> {
    let pair = native_pty_system()
        .openpty(pty_size(cols, rows))
        .map_err(|e| format!("打开本地终端失败: {e}"))?;
    let mut cmd = CommandBuilder::new(resolved.program.as_os_str());
    for arg in &resolved.args {
        cmd.arg(arg);
    }
    cmd.cwd(resolved.cwd.as_os_str());
    cmd.env("TERM_PROGRAM", "SnailSSH");
    // 从 Windows Terminal 启动时会继承 WT_SESSION。删掉它，避免子进程误认宿主。
    cmd.env_remove("WT_SESSION");
    #[cfg(unix)]
    if cmd.get_env("TERM").is_none() {
        cmd.env("TERM", "xterm-256color");
    }
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("启动 {} 失败: {e}", resolved.program.display()))?;
    let pid = child.process_id();
    let killer = child.clone_killer();
    #[cfg(windows)]
    let job = {
        let raw = child.as_raw_handle().ok_or_else(|| {
            let _ = child.kill();
            "无法取得本地进程句柄".to_string()
        })?;
        match JobGuard::assign(raw) {
            Ok(job) => job,
            Err(err) => {
                let _ = child.kill();
                return Err(format!("无法把本地进程纳入作业对象，已取消启动: {err}"));
            }
        }
    };
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("打开本地终端输出失败: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("打开本地终端输入失败: {e}"))?;
    Ok((
        SpawnedPty {
            writer,
            master: pair.master,
            child,
            killer,
            #[cfg(windows)]
            job,
            pid,
        },
        reader,
    ))
}

fn start_reader_with(session: Arc<Session>, mut reader: Box<dyn Read + Send>) {
    std::thread::spawn(move || {
        let mut backpressure = false;
        let mut binary = false;
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let text = session.gate.lock().push_bytes(&buf[..n]);
                    if let Some(text) = text {
                        if let Some(tx) = session.log.lock().as_ref().map(|handle| handle.tx.clone()) {
                            crate::ssh::push_log(&tx, &text, &mut backpressure, &mut binary);
                        }
                        session.sink.emit(&session.stdout_event, text);
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let reason = {
            let mut child = session.child.lock();
            match child.try_wait() {
                Ok(Some(status)) => format_exit(&status),
                Ok(None) => match child.wait() {
                    Ok(status) => format_exit(&status),
                    Err(err) => format!("进程已退出（{err}）"),
                },
                Err(err) => format!("进程已退出（{err}）"),
            }
        };
        session.finish(reason);
    });
}

fn watch_exit(session: Arc<Session>) {
    std::thread::spawn(move || {
        {
            let mut child = session.child.lock();
            let _ = child.wait();
        }
        let mut io = session.io.lock();
        let _ = std::mem::replace(&mut io.writer, Box::new(std::io::sink()));
    });
}

fn prepare_launch(request: &CreateLocalSessionRequest, probe: &LaunchProbe) -> Result<ResolvedShell, String> {
    reject_nul(request.program.as_deref().unwrap_or(""), "程序")?;
    for arg in &request.args {
        reject_nul(arg, "参数")?;
    }
    reject_nul(request.cwd.as_deref().unwrap_or(""), "工作目录")?;

    let program_raw = request.program.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (program, label, kind) = match program_raw {
        Some(raw) => {
            let program = resolve_user_program(raw, probe)?;
            let (label, kind) = classify(&program);
            (program, label, kind)
        }
        None => resolve_auto(probe)?,
    };
    let cwd = resolve_cwd(request.cwd.as_deref(), probe)?;
    Ok(ResolvedShell {
        program,
        args: request.args.clone(),
        cwd,
        label,
        kind,
    })
}

fn reject_nul(value: &str, what: &str) -> Result<(), String> {
    if value.contains('\0') {
        Err(format!("{what}含有空字节"))
    } else {
        Ok(())
    }
}

fn resolve_user_program(raw: &str, probe: &LaunchProbe) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if is_path_like(raw) {
        if (probe.is_exec)(&path) {
            return Ok(path);
        }
        return Err(format!("找不到可执行文件: {raw}"));
    }
    (probe.lookup)(raw).ok_or_else(|| format!("找不到可执行文件: {raw}"))
}

fn resolve_auto(probe: &LaunchProbe) -> Result<(PathBuf, String, String), String> {
    match probe.platform {
        Platform::Windows => resolve_windows(probe),
        Platform::Unix => resolve_unix(probe),
    }
}

fn resolve_windows(probe: &LaunchProbe) -> Result<(PathBuf, String, String), String> {
    if let Some(path) = (probe.lookup)("pwsh") {
        return Ok(classify_owned(path));
    }
    for candidate in &probe.conventional_pwsh {
        if (probe.is_exec)(candidate) {
            return Ok(classify_owned(candidate.clone()));
        }
    }
    if let Some(path) = (probe.lookup)("powershell") {
        return Ok(classify_owned(path));
    }
    if let Some(path) = (probe.lookup)("cmd") {
        return Ok(classify_owned(path));
    }
    Err("找不到可用的本地 shell（已尝试 pwsh、PowerShell 7 安装目录、powershell.exe、cmd.exe）".into())
}

fn resolve_unix(probe: &LaunchProbe) -> Result<(PathBuf, String, String), String> {
    if let Some(shell) = probe.shell_env.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let path = PathBuf::from(shell);
        if (probe.is_exec)(&path) {
            return Ok(classify_owned(path));
        }
        if is_path_like(shell) {
            // 绝对路径不可执行就继续往下，不把坏的 $SHELL 当成最终结果
        } else if let Some(found) = (probe.lookup)(shell) {
            return Ok(classify_owned(found));
        }
    }
    if let Some(path) = (probe.lookup)("bash") {
        return Ok(classify_owned(path));
    }
    if let Some(path) = (probe.lookup)("sh") {
        return Ok(classify_owned(path));
    }
    Err("找不到可用的本地 shell（已尝试 $SHELL、bash、sh）".into())
}

fn resolve_cwd(raw: Option<&str>, probe: &LaunchProbe) -> Result<PathBuf, String> {
    let raw = raw.map(str::trim).filter(|s| !s.is_empty());
    let Some(raw) = raw else {
        return probe.home.clone().ok_or_else(|| "找不到用户主目录".to_string());
    };
    let path = PathBuf::from(raw);
    if (probe.is_dir)(&path) {
        return Ok(path);
    }
    if (probe.is_file)(&path) {
        return Err("工作目录不是目录".into());
    }
    Err("工作目录不存在".into())
}

fn classify_owned(path: PathBuf) -> (PathBuf, String, String) {
    let (label, kind) = classify(&path);
    (path, label, kind)
}

fn classify(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("shell")
        .to_ascii_lowercase();
    match stem.as_str() {
        "pwsh" => ("PowerShell 7".into(), "pwsh".into()),
        "powershell" => ("Windows PowerShell".into(), "powershell".into()),
        "cmd" => ("命令提示符".into(), "cmd".into()),
        other => (other.to_string(), other.to_string()),
    }
}

fn is_path_like(raw: &str) -> bool {
    Path::new(raw).is_absolute() || raw.contains('/') || raw.contains('\\')
}

fn system_probe() -> LaunchProbe {
    LaunchProbe {
        lookup: Arc::new(lookup_on_path),
        is_exec: Arc::new(is_executable_file),
        is_dir: Arc::new(|path| path.is_dir()),
        is_file: Arc::new(|path| path.is_file()),
        conventional_pwsh: conventional_pwsh_paths(),
        shell_env: std::env::var("SHELL").ok(),
        home: dirs::home_dir(),
        platform: current_platform(),
    }
}

fn current_platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Unix
    }
}

fn lookup_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let direct = dir.join(name);
        if is_executable_file(&direct) {
            return Some(direct);
        }
        #[cfg(windows)]
        {
            let lower = name.to_ascii_lowercase();
            if !lower.ends_with(".exe") && !lower.ends_with(".cmd") && !lower.ends_with(".bat") {
                let with_exe = dir.join(format!("{name}.exe"));
                if is_executable_file(&with_exe) {
                    return Some(with_exe);
                }
            }
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    true
}

fn conventional_pwsh_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(var) {
            out.push(PathBuf::from(root).join("PowerShell").join("7").join("pwsh.exe"));
        }
    }
    out.push(PathBuf::from(r"C:\Program Files\PowerShell\7\pwsh.exe"));
    out
}

#[cfg(test)]
fn host_platform_request(program: Option<&str>, cwd: Option<&str>, args: &[&str]) -> CreateLocalSessionRequest {
    CreateLocalSessionRequest {
        program: program.map(str::to_string),
        args: args.iter().map(|s| (*s).to_string()).collect(),
        cwd: cwd.map(str::to_string),
        cols: 80,
        rows: 24,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::{Duration, Instant};

    fn fake_probe(
        platform: Platform,
        lookup: HashMap<String, PathBuf>,
        exec: HashSet<PathBuf>,
        dirs: HashSet<PathBuf>,
        files: HashSet<PathBuf>,
        conventional: Vec<PathBuf>,
        shell_env: Option<String>,
        home: Option<PathBuf>,
    ) -> LaunchProbe {
        let lookup = Arc::new(lookup);
        let exec = Arc::new(exec);
        let dirs = Arc::new(dirs);
        let files = Arc::new(files);
        LaunchProbe {
            lookup: {
                let lookup = Arc::clone(&lookup);
                Arc::new(move |name| lookup.get(name).cloned())
            },
            is_exec: {
                let exec = Arc::clone(&exec);
                Arc::new(move |path| exec.contains(path))
            },
            is_dir: {
                let dirs = Arc::clone(&dirs);
                Arc::new(move |path| dirs.contains(path))
            },
            is_file: {
                let files = Arc::clone(&files);
                Arc::new(move |path| files.contains(path))
            },
            conventional_pwsh: conventional,
            shell_env,
            home,
            platform,
        }
    }

    fn home() -> PathBuf {
        PathBuf::from("/home/snail")
    }

    fn base_sets() -> (HashSet<PathBuf>, HashSet<PathBuf>) {
        let mut dirs = HashSet::new();
        dirs.insert(home());
        (dirs, HashSet::new())
    }

    #[test]
    fn windows_prefers_pwsh_on_path() {
        let pwsh = PathBuf::from(r"C:\Tools\pwsh.exe");
        let mut lookup = HashMap::new();
        lookup.insert("pwsh".into(), pwsh.clone());
        lookup.insert("powershell".into(), PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"));
        let (dirs, files) = base_sets();
        let probe = fake_probe(Platform::Windows, lookup, HashSet::new(), dirs, files, Vec::new(), None, Some(home()));
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, pwsh);
        assert_eq!(resolved.label, "PowerShell 7");
        assert_eq!(resolved.kind, "pwsh");
    }

    #[test]
    fn windows_uses_conventional_pwsh_when_path_misses() {
        let installed = PathBuf::from(r"D:\PowerShell\7\pwsh.exe");
        let mut exec = HashSet::new();
        exec.insert(installed.clone());
        let (dirs, files) = base_sets();
        let probe = fake_probe(
            Platform::Windows,
            HashMap::new(),
            exec,
            dirs,
            files,
            vec![installed.clone()],
            None,
            Some(home()),
        );
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, installed);
        assert_eq!(resolved.label, "PowerShell 7");
    }

    #[test]
    fn windows_falls_back_to_powershell_then_cmd() {
        let powershell = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        let cmd = PathBuf::from(r"C:\Windows\System32\cmd.exe");
        let (dirs, files) = base_sets();
        let mut lookup = HashMap::new();
        lookup.insert("powershell".into(), powershell.clone());
        lookup.insert("cmd".into(), cmd.clone());
        let probe = fake_probe(Platform::Windows, lookup.clone(), HashSet::new(), dirs.clone(), files.clone(), Vec::new(), None, Some(home()));
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, powershell);
        assert_eq!(resolved.label, "Windows PowerShell");
        assert_eq!(resolved.kind, "powershell");

        let mut only_cmd = HashMap::new();
        only_cmd.insert("cmd".into(), cmd.clone());
        let probe = fake_probe(Platform::Windows, only_cmd, HashSet::new(), dirs, files, Vec::new(), None, Some(home()));
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, cmd);
        assert_eq!(resolved.kind, "cmd");
        assert_eq!(resolved.label, "命令提示符");
    }

    #[test]
    fn unix_falls_back_when_shell_is_not_executable() {
        let bash = PathBuf::from("/bin/bash");
        let sh = PathBuf::from("/bin/sh");
        let mut lookup = HashMap::new();
        lookup.insert("bash".into(), bash.clone());
        lookup.insert("sh".into(), sh.clone());
        let mut exec = HashSet::new();
        exec.insert(bash.clone());
        exec.insert(sh.clone());
        let (dirs, files) = base_sets();
        let probe = fake_probe(
            Platform::Unix,
            lookup.clone(),
            exec.clone(),
            dirs.clone(),
            files.clone(),
            Vec::new(),
            Some("/missing/zsh".into()),
            Some(home()),
        );
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, bash);
        assert_eq!(resolved.kind, "bash");

        let mut only_sh = HashMap::new();
        only_sh.insert("sh".into(), sh.clone());
        let probe = fake_probe(Platform::Unix, only_sh, exec, dirs, files, Vec::new(), Some("/missing/zsh".into()), Some(home()));
        let resolved = prepare_launch(&host_platform_request(None, None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, sh);
        assert_eq!(resolved.kind, "sh");
    }

    #[test]
    fn user_program_wins_over_path_pwsh() {
        let custom = PathBuf::from(r"D:\tools\custom.exe");
        let mut lookup = HashMap::new();
        lookup.insert("pwsh".into(), PathBuf::from(r"C:\pwsh.exe"));
        let mut exec = HashSet::new();
        exec.insert(custom.clone());
        let (dirs, files) = base_sets();
        let probe = fake_probe(Platform::Windows, lookup, exec, dirs, files, Vec::new(), None, Some(home()));
        let resolved = prepare_launch(&host_platform_request(Some(r"D:\tools\custom.exe"), None, &[]), &probe).unwrap();
        assert_eq!(resolved.program, custom);
        assert_eq!(resolved.kind, "custom");
    }

    #[test]
    fn validation_failures_do_not_keep_a_session() {
        let mgr = LocalShellManager::new();
        let file_cwd = PathBuf::from("/home/snail/notes.txt");
        let sink: Arc<dyn EventSink> = Arc::new(MemSink::default());

        let err = mgr
            .create_inner(
                host_platform_request(Some("pwsh\0"), None, &[]),
                empty_windows_probe(),
                Arc::clone(&sink),
            )
            .unwrap_err();
        assert!(err.contains("空字节"), "{err}");
        assert_eq!(mgr.session_count(), 0);

        let err = mgr
            .create_inner(
                host_platform_request(None, Some("/home/snail/notes.txt"), &[]),
                empty_windows_probe(),
                Arc::clone(&sink),
            )
            .unwrap_err();
        // 三个自动候选都不存在，创建失败，不能留下会话
        assert!(err.contains("找不到"), "{err}");
        assert_eq!(mgr.session_count(), 0);

        let mut lookup = HashMap::new();
        lookup.insert("pwsh".into(), PathBuf::from(r"C:\pwsh.exe"));
        let mut exec = HashSet::new();
        exec.insert(PathBuf::from(r"C:\pwsh.exe"));
        let (dirs, files) = {
            let (dirs, mut files) = base_sets();
            files.insert(file_cwd.clone());
            (dirs, files)
        };
        let probe = fake_probe(Platform::Windows, lookup, exec, dirs, files, Vec::new(), None, Some(home()));
        let err = mgr
            .create_inner(
                host_platform_request(Some(r"C:\pwsh.exe"), Some("/home/snail/notes.txt"), &["-NoLogo\0"]),
                probe,
                sink,
            )
            .unwrap_err();
        assert!(err.contains("空字节"), "{err}");
        assert_eq!(mgr.session_count(), 0);

        let (dirs, mut files) = base_sets();
        files.insert(file_cwd.clone());
        let mut exec = HashSet::new();
        exec.insert(PathBuf::from(r"C:\pwsh.exe"));
        let probe = fake_probe(Platform::Windows, HashMap::new(), exec, dirs, files, Vec::new(), None, Some(home()));
        let err = mgr
            .create_inner(
                host_platform_request(Some(r"C:\pwsh.exe"), Some("/home/snail/notes.txt"), &[]),
                probe,
                Arc::new(MemSink::default()),
            )
            .unwrap_err();
        assert!(err.contains("不是目录"), "{err}");
        assert_eq!(mgr.session_count(), 0);
    }

    fn empty_windows_probe() -> LaunchProbe {
        let (dirs, files) = base_sets();
        fake_probe(
            Platform::Windows,
            HashMap::new(),
            HashSet::new(),
            dirs,
            files,
            Vec::new(),
            None,
            Some(home()),
        )
    }

    #[test]
    fn unknown_id_write_resize_take_fail_but_close_is_ok() {
        let mgr = LocalShellManager::new();
        assert!(mgr.write_session("local-missing", "x").unwrap_err().contains("不存在"));
        assert!(mgr.resize_session("local-missing", 80, 24).unwrap_err().contains("不存在"));
        assert!(mgr.take_banner("local-missing").is_err());
        assert!(mgr.close_session("local-missing").is_ok());
        assert!(mgr.close_session("local-missing").is_ok());
    }

    #[test]
    fn banner_is_taken_once_and_later_bytes_are_not() {
        let mut gate = GateState::new(64);
        assert!(gate.push_bytes(b"hello").is_none());
        let (text, closed) = gate.take();
        assert_eq!(text, "hello");
        assert!(closed.is_none());
        assert_eq!(gate.push_bytes(b"world").as_deref(), Some("world"));
        let (again, closed) = gate.take();
        assert_eq!(again, "");
        assert!(closed.is_none());
    }

    #[test]
    fn exit_before_attach_returns_output_and_one_reason() {
        let mut gate = GateState::new(64);
        gate.push_bytes("退出前的输出".as_bytes());
        assert!(gate.note_exit("进程已退出（退出码 3）".into()).is_none());
        let (text, closed) = gate.take();
        assert!(text.contains("退出前的输出"));
        assert_eq!(closed.as_deref(), Some("进程已退出（退出码 3）"));
        let (again, closed_again) = gate.take();
        assert_eq!(again, "");
        assert!(closed_again.is_none());
        assert!(gate.note_exit("再次退出".into()).is_none());
    }

    #[test]
    fn user_close_suppresses_exit_event() {
        let mut gate = GateState::new(32);
        gate.mark_user_closed();
        assert!(gate.note_exit("进程已退出（退出码 1）".into()).is_none());
    }

    #[test]
    fn buffer_drops_oldest_bytes_and_marks_truncation() {
        let mut buffer = ByteBuffer::new(16);
        buffer.push(b"abcdefghijklmnopqrstuvwxyz");
        let text = buffer.take_string();
        assert!(text.contains("截断"));
        // 标记本身含有字母，只检查被丢掉的是最旧的那段输出
        assert!(!text.contains("abcdefgh"));
        assert!(text.contains('z'));
    }

    #[derive(Default)]
    struct MemSink {
        events: Mutex<Vec<(String, String)>>,
    }

    impl EventSink for MemSink {
        fn emit(&self, event: &str, payload: String) {
            self.events.lock().push((event.to_string(), payload));
        }
    }

    impl MemSink {
        fn joined(&self, event: &str) -> String {
            self.events
                .lock()
                .iter()
                .filter(|(name, _)| name == event)
                .map(|(_, payload)| payload.clone())
                .collect()
        }

        fn count(&self, event: &str) -> usize {
            self.events.lock().iter().filter(|(name, _)| name == event).count()
        }
    }

    fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if pred() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        pred()
    }

    #[test]
    fn shell_exit_before_take_keeps_output_and_one_close() {
        let Some(program) = shell_for_exit_test() else {
            eprintln!("skip: no cmd/sh for exit-before-take");
            return;
        };
        let mgr = LocalShellManager::new();
        let sink = Arc::new(MemSink::default());
        let created = mgr
            .create_inner(program.request, system_probe(), sink.clone() as Arc<dyn EventSink>)
            .expect("spawn");
        let _guard = CloseGuard { mgr: &mgr, id: created.session_id.clone() };
        assert!(
            wait_until(Duration::from_secs(5), || !mgr.is_alive(&created.session_id)),
            "process did not exit"
        );
        let banner = mgr.take_banner(&created.session_id).unwrap();
        assert!(banner.contains("HELLO_LOCAL"), "{banner}");
        let closed = format!("local-closed-{}", created.session_id);
        assert_eq!(sink.count(&closed), 1, "{:?}", sink.events.lock().clone());
        assert!(mgr.take_banner(&created.session_id).unwrap().is_empty());
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(sink.count(&closed), 1);
    }

    #[test]
    fn user_close_does_not_emit_and_kills_the_tree() {
        let Some(program) = shell_for_sleep_test() else {
            eprintln!("skip: no cmd/sh for process-tree test");
            return;
        };
        let mgr = LocalShellManager::new();
        let sink = Arc::new(MemSink::default());
        let created = mgr
            .create_inner(program.request, system_probe(), sink.clone() as Arc<dyn EventSink>)
            .expect("spawn");
        let id = created.session_id.clone();
        let parent = mgr.process_id(&id).expect("pid");
        assert!(
            wait_until(Duration::from_secs(5), || !child_pids(parent).is_empty()),
            "child process did not appear"
        );
        let children = child_pids(parent);
        mgr.close_session(&id).unwrap();
        assert!(mgr.close_session(&id).unwrap() == ());
        assert!(
            wait_until(Duration::from_secs(5), || {
                !process_alive(parent) && children.iter().all(|pid| !process_alive(*pid))
            }),
            "parent={parent} alive={} children={children:?} still={:?}",
            process_alive(parent),
            children.iter().filter(|pid| process_alive(**pid)).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(sink.count(&format!("local-closed-{id}")), 0);
    }

    #[test]
    fn pwsh_roundtrip_is_utf8_and_kills_children() {
        let Some(program) = find_pwsh() else {
            eprintln!("skip: pwsh not found");
            return;
        };
        let mgr = LocalShellManager::new();
        let sink = Arc::new(MemSink::default());
        let created = mgr
            .create_inner(
                CreateLocalSessionRequest {
                    program: Some(program.display().to_string()),
                    args: vec!["-NoLogo".into(), "-NoProfile".into()],
                    cwd: None,
                    cols: 100,
                    rows: 30,
                },
                system_probe(),
                sink.clone() as Arc<dyn EventSink>,
            )
            .expect("spawn pwsh");
        let _guard = CloseGuard { mgr: &mgr, id: created.session_id.clone() };
        let quoted = program.display().to_string().replace('\'', "''");
        // 标记拆开再拼，避免命令回显里的字面量提前满足断言。
        let script = format!(
            "$p = Start-Process -FilePath '{quoted}' -ArgumentList '-NoLogo','-NoProfile','-Command','Start-Sleep 180' -PassThru -WindowStyle Hidden; Write-Output ('CHILD' + '=' + $p.Id); Write-Output ('SNAIL_' + 'MARKER_' + '中文')\r"
        );
        mgr.write_session(&created.session_id, &script).unwrap();
        let stdout_event = format!("local-stdout-{}", created.session_id);
        let mut seen = mgr.take_banner(&created.session_id).unwrap_or_default();
        let mut child = None;
        assert!(
            wait_until(Duration::from_secs(12), || {
                seen.push_str(&sink.joined(&stdout_event));
                child = parse_child_pid(&seen);
                seen.contains("SNAIL_MARKER_中文") && child.is_some()
            }),
            "output={seen}"
        );
        assert!(std::str::from_utf8(seen.as_bytes()).is_ok());
        let child = child.expect("child pid");
        let parent = mgr.process_id(&created.session_id).expect("parent pid");
        assert!(process_alive(parent));
        assert!(process_alive(child), "child {child} was not alive");
        mgr.close_session(&created.session_id).unwrap();
        assert!(
            wait_until(Duration::from_secs(5), || !process_alive(parent) && !process_alive(child)),
            "parent alive={} child alive={}",
            process_alive(parent),
            process_alive(child)
        );
    }

    struct ExitProgram {
        request: CreateLocalSessionRequest,
    }

    fn shell_for_exit_test() -> Option<ExitProgram> {
        #[cfg(windows)]
        {
            let program = lookup_on_path("cmd")?;
            return Some(ExitProgram {
                request: CreateLocalSessionRequest {
                    program: Some(program.display().to_string()),
                    args: vec!["/c".into(), "echo HELLO_LOCAL".into()],
                    cwd: None,
                    cols: 80,
                    rows: 24,
                },
            });
        }
        #[cfg(not(windows))]
        {
            let program = lookup_on_path("sh")?;
            Some(ExitProgram {
                request: CreateLocalSessionRequest {
                    program: Some(program.display().to_string()),
                    args: vec!["-c".into(), "echo HELLO_LOCAL".into()],
                    cwd: None,
                    cols: 80,
                    rows: 24,
                },
            })
        }
    }

    fn shell_for_sleep_test() -> Option<ExitProgram> {
        #[cfg(windows)]
        {
            let program = lookup_on_path("cmd")?;
            return Some(ExitProgram {
                request: CreateLocalSessionRequest {
                    program: Some(program.display().to_string()),
                    args: vec!["/c".into(), "ping".into(), "127.0.0.1".into(), "-n".into(), "40".into()],
                    cwd: None,
                    cols: 80,
                    rows: 24,
                },
            });
        }
        #[cfg(not(windows))]
        {
            let program = lookup_on_path("sh")?;
            Some(ExitProgram {
                request: CreateLocalSessionRequest {
                    program: Some(program.display().to_string()),
                    args: vec!["-c".into(), "sleep 30 & wait".into()],
                    cwd: None,
                    cols: 80,
                    rows: 24,
                },
            })
        }
    }

    fn parse_child_pid(text: &str) -> Option<u32> {
        text.split("CHILD=").skip(1).find_map(|rest| {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                digits.parse().ok()
            }
        })
    }

    fn find_pwsh() -> Option<PathBuf> {
        if let Some(path) = lookup_on_path("pwsh") {
            return Some(path);
        }
        conventional_pwsh_paths().into_iter().find(|path| is_executable_file(path))
    }

    struct CloseGuard<'a> {
        mgr: &'a LocalShellManager,
        id: String,
    }

    impl Drop for CloseGuard<'_> {
        fn drop(&mut self) {
            let _ = self.mgr.close_session(&self.id);
        }
    }

    fn process_alive(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        #[cfg(windows)]
        {
            use winapi::um::handleapi::CloseHandle;
            use winapi::um::minwinbase::STILL_ACTIVE;
            use winapi::um::processthreadsapi::{GetExitCodeProcess, OpenProcess};
            use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
            unsafe {
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    return false;
                }
                let mut code = 0u32;
                let ok = GetExitCodeProcess(handle, &mut code);
                CloseHandle(handle);
                ok != 0 && code == STILL_ACTIVE
            }
        }
        #[cfg(not(windows))]
        {
            unsafe { libc::kill(pid as i32, 0) == 0 }
        }
    }

    fn child_pids(parent: u32) -> Vec<u32> {
        #[cfg(windows)]
        {
            use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
            use winapi::um::tlhelp32::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
            };
            unsafe {
                let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
                if snap == INVALID_HANDLE_VALUE {
                    return Vec::new();
                }
                let mut entry: PROCESSENTRY32W = std::mem::zeroed();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                let mut out = Vec::new();
                if Process32FirstW(snap, &mut entry) != 0 {
                    loop {
                        if entry.th32ParentProcessID == parent {
                            out.push(entry.th32ProcessID);
                        }
                        if Process32NextW(snap, &mut entry) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snap);
                out
            }
        }
        #[cfg(not(windows))]
        {
            // /proc 不保证存在；用 ps 会把测试绑死在一种用户态工具上。
            // 这里读 /proc/<pid>/stat 的 ppid。读不到就当没有子进程。
            let mut out = Vec::new();
            let Ok(entries) = std::fs::read_dir("/proc") else {
                return out;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(pid_text) = name.to_str() else { continue };
                let Ok(pid) = pid_text.parse::<u32>() else { continue };
                let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else { continue };
                let Some(end) = stat.rfind(')') else { continue };
                let rest = stat[end + 1..].split_whitespace().collect::<Vec<_>>();
                // state, ppid
                if rest.get(1).and_then(|ppid| ppid.parse::<u32>().ok()) == Some(parent) {
                    out.push(pid);
                }
            }
            out
        }
    }
}
