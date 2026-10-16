use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 钥匙串中的服务名与账号前缀：账号 = "host:<host id>"
const KEYRING_SERVICE: &str = "snail-ssh";
const KEYRING_ACCOUNT_PREFIX: &str = "host:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostItem {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(rename = "authType")]
    pub auth_type: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default, rename = "privateKey")]
    pub private_key: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    pub group: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default, rename = "osName")]
    pub os_name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub ping: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    #[serde(default)]
    pub mem: Option<u32>,
    #[serde(default)]
    pub disk: Option<u32>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: Option<String>,
}

/// 单台主机的敏感凭证集合，序列化后整体存入系统钥匙串的一条凭据中，
/// 避免逐字段占用多条钥匙串记录。
#[derive(Debug, Default, Serialize, Deserialize)]
struct HostSecrets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(default, rename = "privateKey", skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    passphrase: Option<String>,
}

impl HostSecrets {
    fn from_host(host: &HostItem) -> Self {
        Self {
            password: host.password.clone(),
            private_key: host.private_key.clone(),
            passphrase: host.passphrase.clone(),
        }
    }

    fn is_empty(&self) -> bool {
        self.password.is_none() && self.private_key.is_none() && self.passphrase.is_none()
    }

    /// 以钥匙串为准回填内存中的主机对象
    fn apply_to(&self, host: &mut HostItem) {
        host.password = self.password.clone();
        host.private_key = self.private_key.clone();
        host.passphrase = self.passphrase.clone();
    }
}

fn keyring_account(host_id: &str, index: usize) -> String {
    if index == 0 {
        format!("{}{}", KEYRING_ACCOUNT_PREFIX, host_id)
    } else {
        format!("{}{}#{}", KEYRING_ACCOUNT_PREFIX, host_id, index)
    }
}

/// Windows 凭据管理器单条凭据上限 2560 字节（macOS/Linux 后端也各有上限），
/// 因此按 2000 字节切片存储，保证 RSA-4096 等较大私钥也能进钥匙串。
const KEYRING_CHUNK_SIZE: usize = 2000;
/// 单台主机最多允许的分片数（≈128KB 凭证）
const MAX_SECRET_CHUNKS: usize = 64;

/// 按 UTF-8 字符边界切分，避免把多字节字符切断
fn chunk_payload(payload: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < payload.len() {
        let mut end = (start + KEYRING_CHUNK_SIZE).min(payload.len());
        while end < payload.len() && !payload.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&payload[start..end]);
        start = end;
    }
    chunks
}

/// 把主机凭证分片写入系统钥匙串；凭证为空时删除旧记录，避免删除密码后残留。
fn store_secrets(host: &HostItem) -> Result<(), String> {
    let secrets = HostSecrets::from_host(host);
    if secrets.is_empty() {
        delete_secrets(&host.id);
        return Ok(());
    }

    let payload = serde_json::to_string(&secrets).map_err(|e| e.to_string())?;
    let chunks = chunk_payload(&payload);
    if chunks.len() > MAX_SECRET_CHUNKS {
        return Err("凭证体积超出钥匙串存储上限".into());
    }

    for (index, chunk) in chunks.iter().enumerate() {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&host.id, index))
            .map_err(|e| format!("访问系统钥匙串失败: {}", e))?;
        entry
            .set_password(chunk)
            .map_err(|e| format!("写入系统钥匙串失败: {}", e))?;
    }

    // 凭证变短时清理上一次遗留的更多分片
    for index in chunks.len()..MAX_SECRET_CHUNKS {
        match keyring::Entry::new(KEYRING_SERVICE, &keyring_account(&host.id, index)) {
            // 分片已不存在说明没有更多旧分片
            Ok(entry) => {
                if entry.delete_credential().is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

/// 从系统钥匙串逐片读取并拼接凭证，读不到时返回 None。
fn load_secrets(host_id: &str) -> Option<HostSecrets> {
    let mut payload = String::new();
    for index in 0..MAX_SECRET_CHUNKS {
        let entry = match keyring::Entry::new(KEYRING_SERVICE, &keyring_account(host_id, index)) {
            Ok(entry) => entry,
            Err(_) => break,
        };
        match entry.get_password() {
            Ok(chunk) => payload.push_str(&chunk),
            // 分片序号断裂即表示已读完
            Err(_) => break,
        }
    }
    if payload.is_empty() {
        return None;
    }
    serde_json::from_str(&payload).ok()
}

fn delete_secrets(host_id: &str) {
    for index in 0..MAX_SECRET_CHUNKS {
        match keyring::Entry::new(KEYRING_SERVICE, &keyring_account(host_id, index)) {
            Ok(entry) => {
                if entry.delete_credential().is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn get_config_file_path() -> PathBuf {
    let base_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".snail-ssh");
    if !base_dir.exists() {
        let _ = fs::create_dir_all(&base_dir);
    }
    base_dir.join("hosts.json")
}

pub fn load_hosts() -> Vec<HostItem> {
    let path = get_config_file_path();
    if !path.exists() {
        return Vec::new();
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Vec<HostItem>>(&content) {
            Ok(list) => rehydrate_secrets(list),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

/// 用系统钥匙串中的凭证覆盖文件里的值。
/// 钥匙串条目不存在时保留文件中的历史值，兼容升级前明文保存的配置。
fn rehydrate_secrets(mut list: Vec<HostItem>) -> Vec<HostItem> {
    for host in list.iter_mut() {
        if let Some(secrets) = load_secrets(&host.id) {
            secrets.apply_to(host);
        }
    }
    list
}

/// 配置文件里绝不落明文凭证：敏感字段交给系统钥匙串保管。
/// 仅当钥匙串不可用时才回退到文件保存，避免凭据直接丢失。
fn strip_secrets(host: &HostItem) -> HostItem {
    let mut stored = host.clone();
    match store_secrets(host) {
        Ok(()) => {
            stored.password = None;
            stored.private_key = None;
            stored.passphrase = None;
        }
        Err(err) => {
            eprintln!(
                "[snail-ssh] 系统钥匙串不可用（{}），主机 {} 的凭证将暂存于 hosts.json",
                err, host.id
            );
        }
    }
    stored
}

pub fn save_host(host: HostItem) -> Result<(), String> {
    let stored = strip_secrets(&host);
    let mut list = load_hosts();
    if let Some(idx) = list.iter().position(|h| h.id == stored.id) {
        list[idx] = stored;
    } else {
        list.insert(0, stored);
    }
    persist_hosts(&list)
}

pub fn delete_host(id: &str) -> Result<(), String> {
    let mut list = load_hosts();
    list.retain(|h| h.id != id);
    delete_secrets(id);
    persist_hosts(&list)
}

fn persist_hosts(list: &[HostItem]) -> Result<(), String> {
    let path = get_config_file_path();
    let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_payload_roundtrips_ascii() {
        let payload = "a".repeat(KEYRING_CHUNK_SIZE * 2 + 7);
        let chunks = chunk_payload(&payload);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.len() <= KEYRING_CHUNK_SIZE));
        assert_eq!(chunks.concat(), payload);
    }

    #[test]
    fn chunk_payload_keeps_utf8_boundaries() {
        // 中文每字 3 字节，强制跨切片位置，验证不会被切断
        let payload = "密钥".repeat(2000);
        let chunks = chunk_payload(&payload);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= KEYRING_CHUNK_SIZE));
        assert_eq!(chunks.concat(), payload);
    }

    #[test]
    fn strip_secrets_keeps_metrics_and_drops_credentials() {
        let host = HostItem {
            id: "unit-test-ephemeral".into(),
            name: "t".into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth_type: "password".into(),
            password: None,
            private_key: None,
            passphrase: None,
            group: "production".into(),
            tags: vec![],
            os: Some("ubuntu".into()),
            os_name: Some("Ubuntu 22.04".into()),
            status: Some("online".into()),
            ping: Some("10ms".into()),
            cpu: Some(12),
            mem: Some(34),
            disk: Some(56),
            updated_at: Some("刚刚".into()),
        };
        // 无凭证时不应报错，且探测指标保留
        let stored = strip_secrets(&host);
        assert_eq!(stored.os_name.as_deref(), Some("Ubuntu 22.04"));
        assert_eq!(stored.cpu, Some(12));
    }
}
