use crate::model::{PortForward, ProxyConfig};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// 钥匙串中的服务名与账号前缀：账号 = "host:<host id>"
const KEYRING_SERVICE: &str = "snail-ssh";
const KEYRING_ACCOUNT_PREFIX: &str = "host:";

const HOSTS_FILE_NAME: &str = "hosts.json";
/// hosts.json 的信封版本号。裸数组加信封是为了区分「旧格式」和「被写坏的文件」：
/// 没有版本号时两者无从分辨，也就无法在损坏时决定要不要回退备份。
const HOSTS_SCHEMA_VERSION: u32 = 1;
/// 自动备份保留份数：hosts.json.bak.1 是最新一份，序号越大越旧。
const MAX_HOSTS_BACKUPS: usize = 3;

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
    /// 按顺序经过的跳板主机 id。连接时由前端展开成带凭证的跳，避免这里再存一份密钥。
    #[serde(default, rename = "jumpHostIds")]
    pub jump_host_ids: Vec<String>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub forwards: Vec<PortForward>,
}

/// hosts.json 的磁盘结构。只有结构不兼容地变化时才升版本；新增可选字段靠
/// `#[serde(default)]` 兼容，不必改动版本号。
#[derive(Debug, Serialize, Deserialize)]
struct HostsEnvelope {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    hosts: Vec<HostItem>,
}

/// 存储健康状态：设置中心据此把静默降级暴露给用户。
/// 钥匙串不可用时凭证会明文落进 hosts.json，hosts.json 损坏时会回退备份，
/// 这两种情况只写日志用户永远看不到，必须能在界面上告警。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageHealth {
    pub keyring_ok: bool,
    // 前端类型是可选字段，用 skip 序列化才能落到 `undefined` 而不是 `null`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyring_error: Option<String>,
    pub hosts_file_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts_file_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_from_backup: Option<String>,
    pub backup_count: usize,
}

/// 只记录「最近一次操作观察到的结果」，不做持久化：重启后重新探测即可，
/// 否则过期状态会变成假告警。
#[derive(Debug)]
struct HealthState {
    keyring_ok: bool,
    keyring_error: Option<String>,
    hosts_file_ok: bool,
    hosts_file_error: Option<String>,
    restored_from_backup: Option<String>,
}

impl Default for HealthState {
    fn default() -> Self {
        // 尚未观察到失败前默认健康，避免界面启动瞬间闪一条假告警
        Self {
            keyring_ok: true,
            keyring_error: None,
            hosts_file_ok: true,
            hosts_file_error: None,
            restored_from_backup: None,
        }
    }
}

fn health_state() -> &'static Mutex<HealthState> {
    static HEALTH: LazyLock<Mutex<HealthState>> =
        LazyLock::new(|| Mutex::new(HealthState::default()));
    &HEALTH
}

/// 写盘串行化：临时文件名固定为 hosts.json.tmp，两个命令线程同时保存会互相
/// 覆盖同一份临时文件，反而造出当初想避免的坏文件。
fn hosts_write_lock() -> &'static Mutex<()> {
    static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    &LOCK
}

fn record_keyring_ok() {
    let mut state = health_state().lock();
    state.keyring_ok = true;
    state.keyring_error = None;
}

fn record_keyring_error(err: &str) {
    let mut state = health_state().lock();
    state.keyring_ok = false;
    state.keyring_error = Some(err.to_string());
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
    #[serde(default, rename = "proxyPassword", skip_serializing_if = "Option::is_none")]
    proxy_password: Option<String>,
}

impl HostSecrets {
    fn from_host(host: &HostItem) -> Self {
        Self {
            password: host.password.clone(),
            private_key: host.private_key.clone(),
            passphrase: host.passphrase.clone(),
            proxy_password: host.proxy.as_ref().and_then(|p| p.password.clone()),
        }
    }

    fn is_empty(&self) -> bool {
        self.password.is_none()
            && self.private_key.is_none()
            && self.passphrase.is_none()
            && self.proxy_password.is_none()
    }

    /// 以钥匙串为准回填内存中的主机对象
    fn apply_to(&self, host: &mut HostItem) {
        host.password = self.password.clone();
        host.private_key = self.private_key.clone();
        host.passphrase = self.passphrase.clone();
        if let Some(password) = &self.proxy_password {
            if let Some(proxy) = host.proxy.as_mut() {
                proxy.password = Some(password.clone());
            }
        }
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

/// 应用数据目录 `~/.snail-ssh`：主机配置、命令历史等本地文件都落在这里。
pub(crate) fn app_data_dir() -> PathBuf {
    let base_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".snail-ssh");
    if !base_dir.exists() {
        let _ = fs::create_dir_all(&base_dir);
    }
    // 目录可能是以前以默认权限建出来的，进程内收紧一次即可。
    static TIGHTENED: std::sync::Once = std::sync::Once::new();
    TIGHTENED.call_once(|| crate::fsperm::restrict_dir(&base_dir));
    base_dir
}

fn hosts_file_path_in(dir: &Path) -> PathBuf {
    dir.join(HOSTS_FILE_NAME)
}

fn hosts_backup_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("{}.bak.{}", HOSTS_FILE_NAME, index))
}

/// 先按信封解析，失败再按旧裸数组解析：升级后第一次读到的仍是裸数组，
/// 靠这个回退分支完成迁移，而不是直接当损坏处理。
/// 返回的 bool 表示内容是否为旧裸数组（需要回写信封）。
fn parse_hosts_content(content: &str) -> Result<(Vec<HostItem>, bool), String> {
    if let Ok(envelope) = serde_json::from_str::<HostsEnvelope>(content) {
        return Ok((envelope.hosts, false));
    }
    match serde_json::from_str::<Vec<HostItem>>(content) {
        Ok(list) => Ok((list, true)),
        Err(err) => Err(format!("hosts.json 解析失败: {}", err)),
    }
}

struct HostsLoadOutcome {
    hosts: Vec<HostItem>,
    /// 主文件解析/读取失败的原因；即使已从备份恢复也保留，供健康状态告警
    parse_error: Option<String>,
    restored_from: Option<PathBuf>,
}

fn load_hosts_from_dir(dir: &Path) -> HostsLoadOutcome {
    let path = hosts_file_path_in(dir);
    if !path.exists() {
        // 首次运行不是损坏，不触发备份回退也不告警
        return HostsLoadOutcome {
            hosts: Vec::new(),
            parse_error: None,
            restored_from: None,
        };
    }

    let failure = match fs::read_to_string(&path) {
        Ok(content) => match parse_hosts_content(&content) {
            Ok((hosts, legacy)) => {
                if legacy {
                    // 迁移写失败不回退读取结果，用户至少还能看到已经解析出来的主机
                    if let Err(err) = write_hosts_atomic(dir, &hosts) {
                        eprintln!("[snail-ssh] hosts.json 迁移到信封格式失败（{}）", err);
                    }
                }
                return HostsLoadOutcome {
                    hosts,
                    parse_error: None,
                    restored_from: None,
                };
            }
            Err(err) => err,
        },
        Err(err) => format!("读取 hosts.json 失败: {}", err),
    };

    match restore_hosts_from_backup(dir) {
        Some((hosts, backup_path)) => HostsLoadOutcome {
            hosts,
            parse_error: Some(failure),
            restored_from: Some(backup_path),
        },
        None => {
            eprintln!(
                "[snail-ssh] hosts.json 损坏且无可用备份（{}），本次以空列表启动",
                failure
            );
            HostsLoadOutcome {
                hosts: Vec::new(),
                parse_error: Some(failure),
                restored_from: None,
            }
        }
    }
}

/// 按 .bak.1 → .bak.3 找第一个能完整解析的备份，并写回主文件。
/// 恢复写主文件时刻意不轮转备份：此时主文件是坏文件，拿它覆盖 .bak.1
/// 会把最旧但最完好的那份备份挤掉。
fn restore_hosts_from_backup(dir: &Path) -> Option<(Vec<HostItem>, PathBuf)> {
    for index in 1..=MAX_HOSTS_BACKUPS {
        let backup = hosts_backup_path(dir, index);
        let content = match fs::read_to_string(&backup) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let (hosts, _) = match parse_hosts_content(&content) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        if let Err(err) = write_hosts_atomic(dir, &hosts) {
            eprintln!(
                "[snail-ssh] 备份 {} 恢复到 hosts.json 失败（{}）",
                backup.display(),
                err
            );
            return None;
        }
        eprintln!(
            "[snail-ssh] hosts.json 损坏，已从 {} 恢复 {} 台主机",
            backup.display(),
            hosts.len()
        );
        return Some((hosts, backup));
    }
    None
}

fn load_hosts_outcome() -> HostsLoadOutcome {
    let outcome = load_hosts_from_dir(&app_data_dir());
    {
        let mut state = health_state().lock();
        state.hosts_file_ok = outcome.parse_error.is_none();
        state.hosts_file_error = outcome.parse_error.clone();
        // 恢复痕迹不清空：它表示「本次会话发生过损坏回退」，下次成功读取不该抹掉告警
        if let Some(path) = outcome.restored_from.as_ref() {
            state.restored_from_backup = Some(path.display().to_string());
        }
    }
    outcome
}

pub fn load_hosts() -> Vec<HostItem> {
    rehydrate_secrets(load_hosts_outcome().hosts)
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
            record_keyring_ok();
            stored.password = None;
            stored.private_key = None;
            stored.passphrase = None;
            if let Some(proxy) = stored.proxy.as_mut() {
                proxy.password = None;
            }
        }
        Err(err) => {
            record_keyring_error(&err);
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
    let result = persist_hosts_to_dir(&app_data_dir(), list);
    let mut state = health_state().lock();
    state.hosts_file_ok = result.is_ok();
    state.hosts_file_error = result.as_ref().err().cloned();
    result
}

fn persist_hosts_to_dir(dir: &Path, list: &[HostItem]) -> Result<(), String> {
    // 备份失败不阻塞主写入：备份是兜底手段，不能反过来成为写盘失败的原因
    if let Err(err) = rotate_hosts_backups(dir) {
        eprintln!(
            "[snail-ssh] hosts.json 自动备份失败（{}），继续写主文件",
            err
        );
    }
    write_hosts_atomic(dir, list)
}

/// 轮转：主文件复制为 .bak.1，旧的依次后移，超出 MAX_HOSTS_BACKUPS 的自然被挤掉。
fn rotate_hosts_backups(dir: &Path) -> Result<(), String> {
    let source = hosts_file_path_in(dir);
    if !source.exists() {
        return Ok(());
    }
    for index in (1..MAX_HOSTS_BACKUPS).rev() {
        let from = hosts_backup_path(dir, index);
        if !from.exists() {
            continue;
        }
        let to = hosts_backup_path(dir, index + 1);
        // Windows 上 rename 不能覆盖已存在的目标，先删再改名
        let _ = fs::remove_file(&to);
        if let Err(err) = fs::rename(&from, &to) {
            eprintln!(
                "[snail-ssh] 备份轮转 {} → {} 失败（{}）",
                from.display(),
                to.display(),
                err
            );
        }
    }
    let target = hosts_backup_path(dir, 1);
    let _ = fs::remove_file(&target);
    fs::copy(&source, &target).map_err(|e| format!("备份 hosts.json 失败: {}", e))?;
    crate::fsperm::restrict_file(&target);
    Ok(())
}

/// 先写 hosts.json.tmp 再 rename，避免断电/被杀进程留下半个 JSON。
/// 与 settings.rs 同一手法；多一层兜底是因为 Windows 上目标被占用时 rename 会失败。
fn write_hosts_atomic(dir: &Path, list: &[HostItem]) -> Result<(), String> {
    let _guard = hosts_write_lock().lock();
    let path = hosts_file_path_in(dir);
    let tmp = path.with_extension("json.tmp");
    let envelope = HostsEnvelope {
        schema_version: HOSTS_SCHEMA_VERSION,
        hosts: list.to_vec(),
    };
    let json = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("序列化主机配置失败: {}", e))?;
    fs::write(&tmp, json).map_err(|e| format!("写入主机配置失败: {}", e))?;
    crate::fsperm::restrict_file(&tmp);
    let renamed = match fs::rename(&tmp, &path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = fs::remove_file(&path);
            fs::rename(&tmp, &path).map_err(|e| format!("保存主机配置失败: {}", e))
        }
    };
    if renamed.is_ok() {
        crate::fsperm::restrict_file(&path);
    }
    renamed
}

fn count_hosts_backups(dir: &Path) -> usize {
    (1..=MAX_HOSTS_BACKUPS)
        .filter(|index| hosts_backup_path(dir, *index).exists())
        .count()
}

/// 主动探测钥匙串是否真正可用。
/// 不这样做的话健康状态只能靠「保存主机时写失败」来发现：全新安装、只读资产、
/// 一次没保存过的用户，即使钥匙串完全不可用也永远显示正常——而这恰恰是最该告警的场景。
/// 探测用独立的探测账号，读不到（NoEntry）本身就是成功的一半：说明后端可访问。
fn probe_keyring() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, "health-probe")
        .map_err(|e| format!("访问系统钥匙串失败: {}", e))?;
    match entry.get_password() {
        // 探测账号本来就该不存在
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!("读取系统钥匙串失败: {}", err)),
    }
}

pub fn get_health() -> StorageHealth {
    // 先取目录再上锁：app_data_dir 可能创建目录，不应持锁做 IO
    let backup_count = count_hosts_backups(&app_data_dir());
    // 探测会真的碰一次系统钥匙串（Linux 上可能拉起 Secret Service），放在锁外做
    match probe_keyring() {
        Ok(()) => record_keyring_ok(),
        Err(err) => record_keyring_error(&err),
    }
    let state = health_state().lock();
    StorageHealth {
        keyring_ok: state.keyring_ok,
        keyring_error: state.keyring_error.clone(),
        hosts_file_ok: state.hosts_file_ok,
        hosts_file_error: state.hosts_file_error.clone(),
        restored_from_backup: state.restored_from_backup.clone(),
        backup_count,
    }
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
            jump_host_ids: Vec::new(),
            proxy: None,
            forwards: Vec::new(),
        };
        // 无凭证时不应报错，且探测指标保留
        let stored = strip_secrets(&host);
        assert_eq!(stored.os_name.as_deref(), Some("Ubuntu 22.04"));
        assert_eq!(stored.cpu, Some(12));
    }

    /// 临时目录隔离：这些用例会真的写 hosts.json 与备份，绝不能落到用户目录
    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("snail-hosts-{}-{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_host(id: &str) -> HostItem {
        HostItem {
            id: id.into(),
            name: id.into(),
            host: "127.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth_type: "password".into(),
            password: None,
            private_key: None,
            passphrase: None,
            group: "default".into(),
            tags: Vec::new(),
            os: None,
            os_name: None,
            status: None,
            ping: None,
            cpu: None,
            mem: None,
            disk: None,
            updated_at: None,
            jump_host_ids: Vec::new(),
            proxy: None,
            forwards: Vec::new(),
        }
    }

    fn host_ids(list: &[HostItem]) -> Vec<&str> {
        list.iter().map(|h| h.id.as_str()).collect()
    }

    #[test]
    fn hosts_envelope_roundtrips_through_disk() {
        let dir = temp_dir("roundtrip");
        persist_hosts_to_dir(&dir, &[sample_host("a"), sample_host("b")]).unwrap();

        let text = fs::read_to_string(hosts_file_path_in(&dir)).unwrap();
        assert!(
            text.contains("\"schemaVersion\": 1"),
            "写盘必须是信封格式: {}",
            text
        );

        let outcome = load_hosts_from_dir(&dir);
        assert!(outcome.parse_error.is_none());
        assert!(outcome.restored_from.is_none());
        assert_eq!(host_ids(&outcome.hosts), vec!["a", "b"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_bare_array_is_read_and_migrated_in_place() {
        let dir = temp_dir("legacy");
        let path = hosts_file_path_in(&dir);
        fs::write(
            &path,
            serde_json::to_string_pretty(&[sample_host("legacy-1")]).unwrap(),
        )
        .unwrap();

        let outcome = load_hosts_from_dir(&dir);
        assert!(outcome.parse_error.is_none());
        assert_eq!(host_ids(&outcome.hosts), vec!["legacy-1"]);

        // 迁移必须落盘，否则每次启动都要重新解析旧格式
        let migrated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(migrated["schemaVersion"], 1);
        assert_eq!(migrated["hosts"][0]["id"], "legacy-1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_main_file_is_restored_from_bak1() {
        let dir = temp_dir("restore");
        persist_hosts_to_dir(&dir, &[sample_host("first")]).unwrap();
        // 第二次写入会把上一份内容轮转成 .bak.1
        persist_hosts_to_dir(&dir, &[sample_host("second")]).unwrap();
        fs::write(hosts_file_path_in(&dir), "{ 半个 JSON").unwrap();

        let outcome = load_hosts_from_dir(&dir);
        assert!(outcome.parse_error.is_some());
        assert_eq!(host_ids(&outcome.hosts), vec!["first"]);
        let restored = outcome.restored_from.as_ref().unwrap();
        assert!(restored.ends_with("hosts.json.bak.1"), "{:?}", restored);

        // 恢复要把主文件写成可解析的信封，否则下次启动还会再回退一次
        let again = load_hosts_from_dir(&dir);
        assert!(again.parse_error.is_none());
        assert_eq!(host_ids(&again.hosts), vec!["first"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_main_file_without_backup_degrades_to_empty_list() {
        let dir = temp_dir("nobackup");
        fs::write(hosts_file_path_in(&dir), "not json at all").unwrap();

        let outcome = load_hosts_from_dir(&dir);
        assert!(outcome.hosts.is_empty());
        assert!(outcome.parse_error.is_some());
        assert!(outcome.restored_from.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_rotation_keeps_exactly_three_files() {
        let dir = temp_dir("rotation");
        for id in ["h1", "h2", "h3", "h4", "h5"] {
            persist_hosts_to_dir(&dir, &[sample_host(id)]).unwrap();
        }

        assert_eq!(count_hosts_backups(&dir), MAX_HOSTS_BACKUPS);
        assert!(!hosts_backup_path(&dir, MAX_HOSTS_BACKUPS + 1).exists());

        // .bak.1 保存的是倒数第二次写入，最早的 h1 应已被挤掉
        let latest: HostsEnvelope =
            serde_json::from_str(&fs::read_to_string(hosts_backup_path(&dir, 1)).unwrap()).unwrap();
        assert_eq!(latest.schema_version, HOSTS_SCHEMA_VERSION);
        assert_eq!(host_ids(&latest.hosts), vec!["h4"]);
        let oldest: HostsEnvelope =
            serde_json::from_str(&fs::read_to_string(hosts_backup_path(&dir, 3)).unwrap()).unwrap();
        assert_eq!(host_ids(&oldest.hosts), vec!["h2"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn storage_health_serializes_camel_case_with_optional_fields_omitted() {
        let health = StorageHealth {
            keyring_ok: false,
            keyring_error: Some("no secret service".into()),
            hosts_file_ok: true,
            hosts_file_error: None,
            restored_from_backup: None,
            backup_count: 2,
        };
        let value = serde_json::to_value(&health).unwrap();
        assert_eq!(value["keyringOk"], false);
        assert_eq!(value["keyringError"], "no secret service");
        assert_eq!(value["hostsFileOk"], true);
        assert_eq!(value["backupCount"], 2);
        // 前端类型是可选字段，None 必须整个键缺失而不是 null
        assert!(value.get("hostsFileError").is_none());
        assert!(value.get("restoredFromBackup").is_none());
    }
}
