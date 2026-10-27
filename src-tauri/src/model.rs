//! 连接配置的共享结构：主机资产、建连参数和导入结果都用同一套字段，
//! 避免跳板机 / 代理 / 转发在前后端各写一份。

use serde::{Deserialize, Serialize};

fn default_bind_host() -> String {
    "127.0.0.1".into()
}

fn default_true() -> bool {
    true
}

/// 出站代理。只包住第一跳 TCP，不在已经建立的 direct-tcpip 上再套一层。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyConfig {
    /// `socks5` 或 `http`（HTTP CONNECT）
    pub kind: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// 敏感字段：落盘时由 storage 挪进钥匙串
    #[serde(default)]
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn is_active(&self) -> bool {
        !self.host.trim().is_empty() && self.port > 0 && !self.kind.trim().is_empty()
    }
}

/// 一条端口转发。`enabled == false` 只保存，连接时不启动。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortForward {
    pub id: String,
    /// `local`（本地监听，远端目标）/ `remote`（远端监听，本地目标）/ `dynamic`（本地 SOCKS5）
    pub kind: String,
    #[serde(default = "default_bind_host", rename = "bindHost")]
    pub bind_host: String,
    #[serde(rename = "bindPort")]
    pub bind_port: u16,
    #[serde(default, rename = "targetHost")]
    pub target_host: String,
    #[serde(default, rename = "targetPort")]
    pub target_port: u16,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 跳板机的一份可独立认证的连接参数。密钥是否信任只作用于这一跳，
/// 避免用户确认第一跳指纹时把后面没看过的跳也一起信任。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JumpHop {
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
    #[serde(default, rename = "trustHostKey")]
    pub trust_host_key: bool,
}

/// 把 `~/foo` 展开成家目录。Windows 上 OpenSSH 配置也常用这种写法。
pub fn expand_tilde(raw: &str) -> String {
    let raw = raw.trim();
    let rest = if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        rest
    } else if raw == "~" {
        ""
    } else {
        return raw.to_string();
    };
    match dirs::home_dir() {
        Some(home) if rest.is_empty() => home.to_string_lossy().to_string(),
        Some(home) => home.join(rest).to_string_lossy().to_string(),
        None => raw.to_string(),
    }
}
