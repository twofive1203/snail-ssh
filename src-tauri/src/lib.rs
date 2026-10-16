mod sftp;
mod ssh;
mod storage;

use serde::{Deserialize, Serialize};
use sftp::SftpFileItem;
use ssh::{SshConnectConfig, SshManager};
use storage::HostItem;
use tauri::{AppHandle, State};

#[derive(Serialize, Deserialize)]
pub struct CreateSessionResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
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
fn create_ssh_session(
    config: SshConnectConfig,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
) -> Result<CreateSessionResponse, String> {
    let session_id = ssh_manager.create_session(config, app_handle)?;
    Ok(CreateSessionResponse { session_id })
}

#[tauri::command]
fn write_ssh_session(
    session_id: String,
    data: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    ssh_manager.write_session(&session_id, &data)
}

#[tauri::command]
fn resize_ssh_session(
    session_id: String,
    cols: u32,
    rows: u32,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    ssh_manager.resize_session(&session_id, cols, rows)
}

#[tauri::command]
fn close_ssh_session(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    ssh_manager.close_session(&session_id)
}

// ==========================================
// SFTP 远程文件管理命令
// ==========================================
#[tauri::command]
fn sftp_read_dir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<Vec<SftpFileItem>, String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_read_dir(raw_sess, &path)
}

#[tauri::command]
fn sftp_read_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_read_file(raw_sess, &path)
}

#[tauri::command]
fn sftp_write_file(
    session_id: String,
    path: String,
    content: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_write_file(raw_sess, &path, &content)
}

#[tauri::command]
fn sftp_mkdir(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_mkdir(raw_sess, &path)
}

#[tauri::command]
fn sftp_remove_file(
    session_id: String,
    path: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_remove_file(raw_sess, &path)
}

#[tauri::command]
fn sftp_upload_file(
    session_id: String,
    local_path: String,
    remote_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_upload_file(raw_sess, &local_path, &remote_path, &task_id, app_handle)
}

#[tauri::command]
fn sftp_download_file(
    session_id: String,
    remote_path: String,
    local_path: String,
    task_id: String,
    ssh_manager: State<'_, SshManager>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let raw_sess = ssh_manager
        .get_raw_session(&session_id)
        .ok_or_else(|| "会话未找到或未连接".to_string())?;
    sftp::sftp_download_file(raw_sess, &remote_path, &local_path, &task_id, app_handle)
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
