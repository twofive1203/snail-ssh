mod sftp;
mod ssh;
mod storage;

use serde::{Deserialize, Serialize};
use sftp::SftpFileItem;
use ssh::{ConnectionTestResult, HostProbeResult, SshConnectConfig, SshManager};
use storage::HostItem;
use tauri::{AppHandle, State};

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
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || manager.close_session(&session_id)).await
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
async fn sftp_upload_file(
    session_id: String,
    local_path: String,
    remote_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_upload_file(session, &local_path, &remote_path, &task_id, app_handle)
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
) -> Result<(), String> {
    let manager = ssh_manager.inner().clone();
    run_blocking(move || {
        let session = manager.get_sftp_session(&session_id)?;
        sftp::sftp_download_file(session, &remote_path, &local_path, &task_id, app_handle)
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(SshManager::new())
        .invoke_handler(tauri::generate_handler![
            get_hosts,
            save_host,
            delete_host,
            create_ssh_session,
            write_ssh_session,
            resize_ssh_session,
            close_ssh_session,
            test_ssh_connection,
            probe_host_info,
            sftp_home_dir,
            sftp_read_dir,
            sftp_read_file,
            sftp_write_file,
            sftp_mkdir,
            sftp_remove_file,
            sftp_upload_file,
            sftp_download_file,
        ])
        .run(tauri::generate_context!())
        .expect("运行 SnailSSH 客户端失败");
}
