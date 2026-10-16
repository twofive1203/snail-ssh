use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
        return get_default_mock_hosts();
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Vec<HostItem>>(&content) {
            Ok(list) => list,
            Err(_) => get_default_mock_hosts(),
        },
        Err(_) => get_default_mock_hosts(),
    }
}

pub fn save_host(host: HostItem) -> Result<(), String> {
    let mut list = load_hosts();
    if let Some(idx) = list.iter().position(|h| h.id == host.id) {
        list[idx] = host;
    } else {
        list.insert(0, host);
    }
    persist_hosts(&list)
}

pub fn delete_host(id: &str) -> Result<(), String> {
    let mut list = load_hosts();
    list.retain(|h| h.id != id);
    persist_hosts(&list)
}

fn persist_hosts(list: &[HostItem]) -> Result<(), String> {
    let path = get_config_file_path();
    let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn get_default_mock_hosts() -> Vec<HostItem> {
    vec![
        HostItem {
            id: "prod-k8s-master-01".into(),
            name: "prod-k8s-master-01".into(),
            host: "47.98.112.24".into(),
            port: 22,
            username: "root".into(),
            auth_type: "key".into(),
            password: None,
            private_key: None,
            group: "production".into(),
            tags: vec!["k8s".into(), "master".into(), "aliyun-hz".into()],
            os: Some("ubuntu".into()),
            os_name: Some("Ubuntu 22.04 LTS".into()),
            status: Some("online".into()),
            ping: Some("24ms".into()),
            cpu: Some(28),
            mem: Some(64),
            disk: Some(42),
            updated_at: Some("2分钟前".into()),
        },
        HostItem {
            id: "aliyun-hk-edge".into(),
            name: "aliyun-hk-edge".into(),
            host: "43.154.201.89".into(),
            port: 2222,
            username: "deploy".into(),
            auth_type: "password".into(),
            password: None,
            private_key: None,
            group: "staging".into(),
            tags: vec!["nginx".into(), "gateway".into(), "hk-node".into()],
            os: Some("debian".into()),
            os_name: Some("Debian 12".into()),
            status: Some("online".into()),
            ping: Some("48ms".into()),
            cpu: Some(14),
            mem: Some(38),
            disk: Some(29),
            updated_at: Some("刚刚".into()),
        },
    ]
}
