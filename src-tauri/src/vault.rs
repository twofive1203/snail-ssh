//! 本机密钥保险箱与 known_hosts 视图。
//!
//! 生成的私钥写在 `~/.snail-ssh/keys/`，主机配置只保存路径，密钥内容仍不进 hosts.json。
//! 这里不提供“把私钥内容返回给前端再展示”的接口，列表只给公钥、指纹和路径。

use crate::model::expand_tilde;
use crate::storage::app_data_dir;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKeyInfo {
    pub id: String,
    pub name: String,
    pub algorithm: String,
    pub fingerprint: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    pub path: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GeneratedKey {
    pub info: SshKeyInfo,
    /// 只在生成当时返回一次，方便用户复制到远端 authorized_keys
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

pub fn list_keys() -> Result<Vec<SshKeyInfo>, String> {
    let index = read_index()?;
    Ok(index)
}

pub fn generate_key(name: &str, algorithm: &str, passphrase: Option<&str>) -> Result<GeneratedKey, String> {
    let name = clean_name(name)?;
    let alg = match algorithm {
        "ed25519" | "" => russh::keys::Algorithm::Ed25519,
        "rsa" => russh::keys::Algorithm::Rsa {
            hash: Some(russh::keys::HashAlg::Sha256),
        },
        other => return Err(format!("不支持生成该算法: {}", other)),
    };
    let mut rng = rand::rng();
    let key = russh::keys::PrivateKey::random(&mut rng, alg)
        .map_err(|e| format!("生成密钥失败: {}", e))?;
    let key = encrypt_if_needed(key, passphrase)?;
    let info = write_key(&name, &key)?;
    let public_key = info.public_key.clone();
    Ok(GeneratedKey { info, public_key })
}

pub fn import_key(name: &str, material: &str, passphrase: Option<&str>) -> Result<SshKeyInfo, String> {
    let name = clean_name(name)?;
    let raw = material.trim();
    if raw.is_empty() {
        return Err("没有私钥内容或路径".into());
    }
    let expanded = expand_tilde(raw);
    let key = if Path::new(&expanded).is_file() {
        russh::keys::load_secret_key(&expanded, passphrase.filter(|p| !p.is_empty()))
            .map_err(|e| format!("读取私钥失败: {}", e))?
    } else {
        let parsed = russh::keys::PrivateKey::from_openssh(raw)
            .map_err(|e| format!("解析私钥失败: {}", e))?;
        if parsed.is_encrypted() {
            let pass = passphrase.filter(|p| !p.is_empty()).ok_or("私钥已加密，请填写口令")?;
            parsed.decrypt(pass).map_err(|e| format!("私钥解密失败: {}", e))?
        } else {
            parsed
        }
    };
    write_key(&name, &key)
}

pub fn delete_key(id: &str) -> Result<(), String> {
    let mut index = read_index()?;
    let Some(pos) = index.iter().position(|item| item.id == id) else {
        return Err("密钥不存在".into());
    };
    let removed = index.remove(pos);
    let path = PathBuf::from(&removed.path);
    if path.starts_with(keys_dir()) {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(pub_path(&path));
    }
    write_index(&index)
}

fn write_key(name: &str, key: &russh::keys::PrivateKey) -> Result<SshKeyInfo, String> {
    let dir = keys_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建密钥目录失败: {}", e))?;
    let id = format!("key-{}", uuid::Uuid::new_v4());
    let path = dir.join(&id);
    let openssh = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .map_err(|e| format!("编码私钥失败: {}", e))?;
    write_private(&path, openssh.as_bytes())?;
    let public_key = key
        .public_key()
        .to_openssh()
        .map_err(|e| format!("编码公钥失败: {}", e))?;
    fs::write(pub_path(&path), format!("{}\n", public_key.trim()))
        .map_err(|e| format!("写入公钥失败: {}", e))?;
    let info = SshKeyInfo {
        id,
        name: name.to_string(),
        algorithm: algorithm_name(key),
        fingerprint: fingerprint(key),
        public_key: public_key.trim().to_string(),
        path: path.to_string_lossy().to_string(),
        created_at: now_stamp(),
    };
    let mut index = read_index().unwrap_or_default();
    index.insert(0, info.clone());
    write_index(&index)?;
    Ok(info)
}

fn encrypt_if_needed(
    key: russh::keys::PrivateKey,
    passphrase: Option<&str>,
) -> Result<russh::keys::PrivateKey, String> {
    let Some(pass) = passphrase.filter(|p| !p.is_empty()) else {
        return Ok(key);
    };
    key.encrypt(&mut rand::rng(), pass)
        .map_err(|e| format!("加密私钥失败: {}", e))
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("写入私钥失败: {}", e))?;
        file.write_all(bytes).map_err(|e| format!("写入私钥失败: {}", e))?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes).map_err(|e| format!("写入私钥失败: {}", e))
    }
}

fn keys_dir() -> PathBuf {
    app_data_dir().join("keys")
}

fn index_path() -> PathBuf {
    keys_dir().join("index.json")
}

fn pub_path(private_path: &Path) -> PathBuf {
    let mut name = private_path.as_os_str().to_os_string();
    name.push(".pub");
    PathBuf::from(name)
}

fn read_index() -> Result<Vec<SshKeyInfo>, String> {
    let path = index_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("密钥索引损坏: {}", e))
}

fn write_index(index: &[SshKeyInfo]) -> Result<(), String> {
    fs::create_dir_all(keys_dir()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(index_path(), text).map_err(|e| e.to_string())
}

fn algorithm_name(key: &russh::keys::PrivateKey) -> String {
    key.algorithm().to_string()
}

fn fingerprint(key: &russh::keys::PrivateKey) -> String {
    let blob = key.public_key().to_bytes().unwrap_or_default();
    let digest = Sha256::digest(&blob);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    )
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("请填写密钥名称".into());
    }
    if name.chars().count() > 64 {
        return Err("密钥名称过长".into());
    }
    Ok(name.to_string())
}

fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}
