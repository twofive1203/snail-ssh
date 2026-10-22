mod conn;
mod history;
mod rt;
mod settings;
mod sftp;
mod sftp_compat;
mod ssh;
mod storage;
mod transfer;

use conn::Conn;
use history::HistoryEntry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sftp::SftpFileItem;
use ssh::{ConnectionTestResult, HostProbeResult, RemoteSystemMetrics, SshConnectConfig, SshManager};
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
        result
    })
    .await
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
async fn test_ssh_connection(config: SshConnectConfig) -> Result<ConnectionTestResult, String> {
    run_blocking(move || ssh::test_connection(&config)).await
}

#[tauri::command]
async fn probe_host_info(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<HostProbeResult, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let config = manager.get_config(&session_id)?;
        ssh::probe_host_info(&config)
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

// ==========================================
// SFTP 远程文件管理命令
// ==========================================
#[tauri::command]
async fn sftp_home_dir(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_home_dir(session)
    })
    .await
}

#[tauri::command]
async fn sftp_read_dir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<Vec<SftpFileItem>, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_read_dir(session, &path)
    })
    .await
}

#[tauri::command]
async fn sftp_read_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_read_file(session, &path)
    })
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
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_write_file(session, &path, &content)
    })
    .await
}

#[tauri::command]
async fn sftp_mkdir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_mkdir(session, &path)
    })
    .await
}

#[tauri::command]
async fn sftp_remove_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_remove_file(session, &path)
    })
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
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_rename(session, &from, &to, overwrite)
    })
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
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_chmod(session, &path, mode)
    })
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
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_chown(session, &path, uid, gid)
    })
    .await
}

#[tauri::command]
async fn sftp_touch(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_touch(session, &path)
    })
    .await
}

#[tauri::command]
async fn sftp_stat(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<SftpFileItem, String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_stat(session, &path)
    })
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(SshManager::new())
        .manage(TransferRegistry::new())
        .setup(|app| {
            if let Some(icon) = app.default_window_icon() {
                for window in app.webview_windows().values() {
                    let _ = window.set_icon(icon.clone());
                }
            }
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
            reveal_in_file_manager,
            get_remote_metrics,
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
        ])
        .run(tauri::generate_context!())
        .expect("运行 SnailSSH 客户端失败");
}
