mod authprompt;
mod conn;
mod forward;
mod fsperm;
mod history;
mod model;
mod net;
mod rt;
mod settings;
mod sftp;
mod sftp_compat;
mod ssh;
mod sshconfig;
mod storage;
mod transfer;
mod vault;

use authprompt::AuthPrompter;
use conn::Conn;
use history::HistoryEntry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sftp::SftpFileItem;
use ssh::{ConnectionTestResult, HostProbeResult, RemoteSystemMetrics, SshConnectConfig, SshManager};
use std::collections::HashMap;
use std::sync::Arc;
use storage::HostItem;
use tauri::{AppHandle, Manager, State};
use transfer::{TransferRegistry, TransferSignal};

#[derive(Serialize, Deserialize)]
pub struct CreateSessionResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

/// 把阻塞型后端操作放到线程池执行：同步命令在 IPC 线程内直接运行，
/// 一旦发生阻塞会连带冻结 WebView 界面，因此耗时命令一律走这里。
async fn run_blocking<F, T>(job: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| format!("后台任务执行失败: {}", e))?
}

/// 统一的传输命令包装：取 SFTP 会话 -> 注册控制信号 -> 执行 -> 无论成败都清理注册表。
async fn run_transfer<F>(
    session_id: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    registry: State<'_, TransferRegistry>,
    job: F,
) -> Result<(), String>
where
    F: FnOnce(Arc<Conn>, Arc<TransferSignal>) -> Result<(), String>
        + Send
        + 'static,
{
    let manager = ssh_manager.inner().clone();
    let registry = registry.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        let signal = registry.register(&task_id, &session_id);
        let result = job(session, signal);
        // 成功/失败/取消都要清理，否则注册表会随任务数增长
        registry.finish(&task_id);
        // 通道被对端关掉时丢弃这条 SFTP 连接，否则用户点重试仍会打在同一条死连接上
        if let Err(err) = &result {
            if sftp::is_session_closed_error(err) {
                manager.reset_sftp_session(&session_id);
            }
        }
        result
    })
    .await
}

/// 原样执行一次 SFTP 操作；若失败原因是 SFTP 通道已被关闭（服务端关子系统、
/// 连接空闲被回收等），丢弃这条连接重建后重试一次。
///
/// 只有幂等的读写/元数据操作走这里：上传下载涉及进度与续传状态，失败由用户重试。
fn run_sftp<T, F>(manager: SshManager, session_id: String, job: F) -> Result<T, String>
where
    F: Fn(Arc<Conn>) -> Result<T, String>,
{
    let session = manager.get_sftp_session(&session_id)?;
    match job(Arc::clone(&session)) {
        Err(err) if sftp::is_session_closed_error(&err) => {
            eprintln!("[snail-ssh] SFTP 通道已失效（{}），重建连接后重试", err);
            manager.reset_sftp_session(&session_id);
            let retried = manager.get_sftp_session(&session_id)?;
            job(retried)
        }
        other => other,
    }
}

// ==========================================
// 主机存储命令
// ==========================================
#[tauri::command]
fn get_hosts() -> Vec<HostItem> {
    storage::load_hosts()
}

#[tauri::command]
fn save_host(host: HostItem) -> Result<(), String> {
    storage::save_host(host)
}

#[tauri::command]
fn delete_host(id: String) -> Result<(), String> {
    storage::delete_host(&id)
}

// ==========================================
// 应用设置（终端字体/主题、片段、书签、分组）
// ==========================================
#[tauri::command]
fn get_settings() -> Option<Value> {
    settings::load_settings()
}

#[tauri::command]
fn save_settings(settings: Value) -> Result<(), String> {
    settings::save_settings(&settings)
}

// ==========================================
// 命令历史
// ==========================================
#[tauri::command]
fn get_command_history(host_key: String) -> Vec<HistoryEntry> {
    history::load_history(&host_key)
}

#[tauri::command]
fn push_command_history(host_key: String, cmd: String) -> Result<Vec<HistoryEntry>, String> {
    history::push_history(&host_key, &cmd)
}

#[tauri::command]
fn import_command_history(host_key: String, cmds: Vec<String>) -> Result<(), String> {
    history::import_history(&host_key, cmds)
}

#[tauri::command]
fn clear_command_history(host_key: Option<String>) -> Result<(), String> {
    history::clear_history(host_key.as_deref())
}

// ==========================================
// SSH 交互终端命令
// ==========================================
#[tauri::command]
async fn create_ssh_session(
    config: SshConnectConfig,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
) -> Result<CreateSessionResponse, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        manager
            .create_session(config, app_handle)
            .map(|session_id| CreateSessionResponse { session_id })
    })
    .await
}

#[tauri::command]
async fn write_ssh_session(
    session_id: String,
    data: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.write_session(&session_id, &data)).await
}

#[tauri::command]
async fn resize_ssh_session(
    session_id: String,
    cols: u32,
    rows: u32,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.resize_session(&session_id, cols, rows)).await
}

#[tauri::command]
async fn close_ssh_session(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    let registry = transfer_registry.inner().clone();
    run_blocking(move || {
        // 先取消该会话名下仍在跑的传输，唤醒暂停中的线程避免永久阻塞
        registry.cancel_by_session(&session_id);
        manager.close_session(&session_id)
    })
    .await
}

// ==========================================
// SSH 连通性测试与主机探测命令
// ==========================================
#[tauri::command]
async fn test_ssh_connection(
    config: SshConnectConfig,
    app_handle: AppHandle,
) -> Result<ConnectionTestResult, String> {
    run_blocking(move || ssh::test_connection(&config, Some(AuthPrompter::new(app_handle)))).await
}

#[tauri::command]
async fn probe_host_info(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<HostProbeResult, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let config = manager.get_config(&session_id)?;
        let app = manager.session_app(&session_id)?;
        ssh::probe_host_info(&config, Some(AuthPrompter::new(app)))
    })
    .await
}

#[tauri::command]
async fn get_ssh_session_cwd(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.get_session_cwd(&session_id)).await
}

/// 取走建连时扣下的 shell 首屏输出（Last login / MOTD / 首个提示符）。
/// 前端必须先注册 stdout 监听再调用，否则取回的首屏会与实时输出乱序。
#[tauri::command]
async fn take_ssh_session_banner(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.take_initial_output(&session_id)).await
}

#[tauri::command]
async fn exec_ssh_command(
    session_id: String,
    command: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.exec_command(&session_id, &command)).await
}

// ==========================================
// 会话日志
// ==========================================
#[tauri::command]
async fn set_session_logging(
    session_id: String,
    enabled: bool,
    host_label: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<Option<String>, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.set_session_logging(&session_id, enabled, &host_label)).await
}

#[tauri::command]
async fn get_session_log_path(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<Option<String>, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.get_session_log_path(&session_id)).await
}

#[tauri::command]
async fn get_log_dir() -> Result<String, String> {
    run_blocking(|| {
        let dir = storage::app_data_dir().join("logs");
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {}", e))?;
        Ok(dir.to_string_lossy().to_string())
    })
    .await
}

/// 清理超过保留期的会话日志（固定 7 天，无参数）。
/// 参数故意不暴露给前端：日志清理是后台卫生工作，没必要让界面能指定任意保留期。
#[tauri::command]
async fn prune_session_logs() -> Result<u64, String> {
    run_blocking(ssh::auto_prune_session_logs).await
}

/// 汇报存储健康状态：钥匙串是否可用、hosts.json 是否完好、是否刚从备份恢复。
/// 纯读内存静态量 + 统计备份文件数，不涉及网络或钥匙串访问，无需 run_blocking。
#[tauri::command]
fn get_storage_health() -> Result<storage::StorageHealth, String> {
    Ok(storage::get_health())
}

/// 在系统文件管理器中打开路径。仅允许应用数据目录（`~/.snail-ssh/`）下的路径，
/// 防止前端被任意传参变成“打开任意路径”。
fn reveal_path(path: &str) -> Result<(), String> {
    let target = std::path::PathBuf::from(path);
    if !target.exists() {
        return Err("路径不存在".into());
    }
    let base = storage::app_data_dir()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let canonical = target.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&base) {
        return Err("仅允许打开应用数据目录内的路径".into());
    }

    let mut cmd = {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
        }
    };
    cmd.arg(&canonical);
    cmd.spawn().map_err(|e| format!("无法打开文件管理器: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn reveal_in_file_manager(path: String) -> Result<(), String> {
    run_blocking(move || reveal_path(&path)).await
}

#[tauri::command]
async fn get_remote_metrics(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<RemoteSystemMetrics, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.get_remote_metrics(&session_id)).await
}

/// 告诉后台采样器「现在该采哪个会话」。
/// 只改一个目标锁，不碰网络，同步命令即可；传 None 表示停止采样。
#[tauri::command]
fn set_metrics_target(session_id: Option<String>, ssh_manager: State<'_, SshManager>) {
    ssh_manager.inner().set_metrics_target(session_id);
}

// ==========================================
// SFTP 远程文件管理命令
// ==========================================
#[tauri::command]
async fn sftp_home_dir(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_home_dir(session)))
    .await
}

#[tauri::command]
async fn sftp_read_dir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<Vec<SftpFileItem>, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_read_dir(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_read_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_read_file(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_write_file(
    session_id: String,
    path: String,
    content: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_write_file(session, &path, &content)))
    .await
}

#[tauri::command]
async fn sftp_mkdir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_mkdir(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_remove_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_remove_file(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_rename(
    session_id: String,
    from: String,
    to: String,
    overwrite: bool,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_rename(session, &from, &to, overwrite)))
    .await
}

#[tauri::command]
async fn sftp_chmod(
    session_id: String,
    path: String,
    mode: u32,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_chmod(session, &path, mode)))
    .await
}

#[tauri::command]
async fn sftp_chown(
    session_id: String,
    path: String,
    uid: Option<u32>,
    gid: Option<u32>,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_chown(session, &path, uid, gid)))
    .await
}

#[tauri::command]
async fn sftp_touch(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_touch(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_stat(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<SftpFileItem, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || run_sftp(manager, session_id, move |session| sftp::sftp_stat(session, &path)))
    .await
}

#[tauri::command]
async fn sftp_upload_file(
    session_id: String,
    local_path: String,
    remote_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let tid = task_id.clone();
    run_transfer(session_id, task_id, ssh_manager, transfer_registry, move |session, signal| {
        sftp::sftp_upload_file(session, &local_path, &remote_path, &tid, signal, app_handle)
    })
    .await
}

#[tauri::command]
async fn sftp_upload_path(
    session_id: String,
    local_path: String,
    remote_dir: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let tid = task_id.clone();
    run_transfer(session_id, task_id, ssh_manager, transfer_registry, move |session, signal| {
        sftp::sftp_upload_path(session, &local_path, &remote_dir, &tid, signal, app_handle)
    })
    .await
}

#[tauri::command]
async fn sftp_download_file(
    session_id: String,
    remote_path: String,
    local_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let tid = task_id.clone();
    run_transfer(session_id, task_id, ssh_manager, transfer_registry, move |session, signal| {
        sftp::sftp_download_file(session, &remote_path, &local_path, &tid, signal, app_handle)
    })
    .await
}

#[tauri::command]
async fn sftp_download_dir(
    session_id: String,
    remote_path: String,
    local_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let tid = task_id.clone();
    run_transfer(session_id, task_id, ssh_manager, transfer_registry, move |session, signal| {
        sftp::sftp_download_dir(session, &remote_path, &local_path, &tid, signal, app_handle)
    })
    .await
}

/// 统一下载入口：文件/目录由后端按远端类型分派
#[tauri::command]
async fn sftp_download_path(
    session_id: String,
    remote_path: String,
    local_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    let tid = task_id.clone();
    run_transfer(session_id, task_id, ssh_manager, transfer_registry, move |session, signal| {
        sftp::sftp_download_path(session, &remote_path, &local_path, &tid, signal, app_handle)
    })
    .await
}

/// 传输控制（pause / resume / cancel）：同步命令，不阻塞
#[tauri::command]
fn sftp_transfer_control(
    task_id: String,
    action: String,
    transfer_registry: State<'_, TransferRegistry>,
) -> Result<(), String> {
    transfer_registry.control(&task_id, &action)
}

#[derive(Clone)]
struct TunnelManager {
    items: Arc<parking_lot::Mutex<HashMap<String, TunnelEntry>>>,
}

struct TunnelEntry {
    label: String,
    conn: Arc<Conn>,
    notes: Vec<String>,
}

impl TunnelManager {
    fn new() -> Self {
        Self {
            items: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Serialize)]
struct TunnelStatus {
    id: String,
    label: String,
    notes: Vec<String>,
}

#[tauri::command]
async fn start_tunnel(
    config: SshConnectConfig,
    app_handle: AppHandle,
    tunnels: State<'_, TunnelManager>,
) -> Result<TunnelStatus, String> {
    let tunnels = tunnels.inner().clone();
    run_blocking(move || {
        let label = format!("{}@{}:{}", config.username, config.host, config.port);
        let forwards = config.forwards.clone();
        let conn = rt::block_on(Conn::connect_async(
            config,
            Some(AuthPrompter::new(app_handle)),
        ))?;
        let notes = rt::block_on(conn.start_forwards(&forwards));
        let id = format!("tunnel-{}", uuid::Uuid::new_v4());
        tunnels.items.lock().insert(
            id.clone(),
            TunnelEntry {
                label: label.clone(),
                conn,
                notes: notes.clone(),
            },
        );
        Ok(TunnelStatus { id, label, notes })
    })
    .await
}

#[tauri::command]
async fn stop_tunnel(id: String, tunnels: State<'_, TunnelManager>) -> Result<(), String> {
    let tunnels = tunnels.inner().clone();
    run_blocking(move || {
        let removed = tunnels.items.lock().remove(&id);
        if let Some(entry) = removed {
            rt::block_on(entry.conn.disconnect_async());
        }
        Ok(())
    })
    .await
}

#[tauri::command]
fn list_tunnels(tunnels: State<'_, TunnelManager>) -> Vec<TunnelStatus> {
    tunnels
        .items
        .lock()
        .iter()
        .map(|(id, entry)| TunnelStatus {
            id: id.clone(),
            label: entry.label.clone(),
            notes: entry.notes.clone(),
        })
        .collect()
}

#[tauri::command]
async fn preview_ssh_config(path: Option<String>) -> Result<sshconfig::ImportPreview, String> {
    run_blocking(move || sshconfig::preview_ssh_config(path)).await
}

#[tauri::command]
async fn export_hosts_ssh_config(path: String) -> Result<(), String> {
    run_blocking(move || {
        let text = sshconfig::export_ssh_config(&storage::load_hosts());
        std::fs::write(&path, text).map_err(|e| format!("写入 {} 失败: {}", path, e))
    })
    .await
}

#[tauri::command]
fn list_ssh_keys() -> Result<Vec<vault::SshKeyInfo>, String> {
    vault::list_keys()
}

#[tauri::command]
async fn generate_ssh_key(
    name: String,
    algorithm: String,
    passphrase: Option<String>,
) -> Result<vault::GeneratedKey, String> {
    run_blocking(move || vault::generate_key(&name, &algorithm, passphrase.as_deref())).await
}

#[tauri::command]
async fn import_ssh_key(
    name: String,
    material: String,
    passphrase: Option<String>,
) -> Result<vault::SshKeyInfo, String> {
    run_blocking(move || vault::import_key(&name, &material, passphrase.as_deref())).await
}

#[tauri::command]
fn delete_ssh_key(id: String) -> Result<(), String> {
    vault::delete_key(&id)
}

#[tauri::command]
fn list_known_hosts() -> Result<Vec<conn::KnownHostEntry>, String> {
    conn::list_known_hosts()
}

#[tauri::command]
fn delete_known_host(hosts: String, fingerprint: String) -> Result<(), String> {
    conn::delete_known_host(&hosts, &fingerprint)
}

#[tauri::command]
fn submit_ssh_auth_prompt(request_id: String, responses: Option<Vec<String>>) -> Result<(), String> {
    authprompt::submit(&request_id, responses)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // single-instance 必须是第一个注册的插件：它要在其它插件和 setup 之前拦下第二个实例，
        // 否则双开会同时写 hosts.json / settings.json。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 二次启动只负责唤回；找不到主窗口就静默跳过，不能 unwrap 把已有会话带走
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(SshManager::new())
        .manage(TransferRegistry::new())
        .manage(TunnelManager::new())
        .setup(|app| {
            if let Some(icon) = app.default_window_icon() {
                for window in app.webview_windows().values() {
                    let _ = window.set_icon(icon.clone());
                }
            }
            // 启动时清理过期会话日志（固定 7 天）：日志是明文且不自动轮转，
            // 没人清理就会一直堆到占满磁盘。放到后台线程，不拖慢窗口显示。
            std::thread::spawn(|| {
                if let Err(err) = ssh::auto_prune_session_logs() {
                    eprintln!("[snail-ssh] 清理过期会话日志失败: {}", err);
                }
            });
            // 远程监控改为后端单条定时任务：只采「当前活动会话」，结果经事件推给前端，
            // 前端不再为每个会话各开一条 setInterval。幂等，重复调用无副作用。
            app.state::<SshManager>()
                .start_metrics_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_hosts,
            save_host,
            delete_host,
            get_settings,
            save_settings,
            create_ssh_session,
            write_ssh_session,
            resize_ssh_session,
            close_ssh_session,
            test_ssh_connection,
            probe_host_info,
            get_ssh_session_cwd,
            take_ssh_session_banner,
            exec_ssh_command,
            set_session_logging,
            get_session_log_path,
            get_log_dir,
            prune_session_logs,
            get_storage_health,
            reveal_in_file_manager,
            get_remote_metrics,
            set_metrics_target,
            get_command_history,
            push_command_history,
            import_command_history,
            clear_command_history,
            sftp_home_dir,
            sftp_read_dir,
            sftp_read_file,
            sftp_write_file,
            sftp_mkdir,
            sftp_remove_file,
            sftp_rename,
            sftp_chmod,
            sftp_chown,
            sftp_touch,
            sftp_stat,
            sftp_upload_file,
            sftp_upload_path,
            sftp_download_file,
            sftp_download_dir,
            sftp_download_path,
            sftp_transfer_control,
            start_tunnel,
            stop_tunnel,
            list_tunnels,
            preview_ssh_config,
            export_hosts_ssh_config,
            list_ssh_keys,
            generate_ssh_key,
            import_ssh_key,
            delete_ssh_key,
            list_known_hosts,
            delete_known_host,
            submit_ssh_auth_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("运行 SnailSSH 客户端失败");
}
