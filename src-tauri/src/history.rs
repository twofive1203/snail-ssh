use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage;

/// 单台主机的命令历史上限（按使用频次 + 最近使用时间裁剪）
const MAX_ENTRIES_PER_HOST: usize = 500;
/// 超过该长度的命令不入库（多半是误粘贴的整段文本）
const MAX_COMMAND_LEN: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub cmd: String,
    pub count: u32,
    #[serde(rename = "lastUsedAt")]
    pub last_used_at: u64,
}

type HistoryMap = HashMap<String, Vec<HistoryEntry>>;

/// 历史文件位置：`~/.snail-ssh/history.json`
fn history_file_path() -> PathBuf {
    storage::app_data_dir().join("history.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 敏感命令不入库：密码、令牌一旦落盘等同于明文泄露。
/// 规则为启发式（无需额外正则依赖）：命中 password/passwd/token 关键词，
/// 或出现 `-p<值>` / `-p <值>` 形式的密码参数（`-p22` 这类端口号不误伤）。
fn is_sensitive(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    if lower.contains("password")
        || lower.contains("passwd")
        || lower.contains("token=")
        || lower.contains("--token")
    {
        return true;
    }

    let tokens: Vec<&str> = lower.split_whitespace().collect();
    let looks_like_secret = |v: &str| !v.is_empty() && !v.chars().all(|c| c.is_ascii_digit());
    for (index, token) in tokens.iter().enumerate() {
        // 只看单个 -p：--prefix 之类的长选项直接跳过
        if token.starts_with("--") {
            continue;
        }
        let Some(value) = token.strip_prefix("-p") else {
            continue;
        };
        if !value.is_empty() {
            if looks_like_secret(value) {
                return true;
            }
        } else if let Some(next) = tokens.get(index + 1) {
            if looks_like_secret(next) {
                return true;
            }
        }
    }
    false
}

/// 通用过滤：空白、超长、以空格开头（bash 的 HISTCONTROL=ignorespace 约定）、敏感命令均不入库
fn should_record(cmd: &str) -> bool {
    !cmd.trim().is_empty()
        && cmd.len() <= MAX_COMMAND_LEN
        && !cmd.starts_with(' ')
        && !is_sensitive(cmd)
}

/// 超上限时按 (count desc, lastUsedAt desc) 保留前 N 条
fn trim_entries(list: &mut Vec<HistoryEntry>) {
    if list.len() <= MAX_ENTRIES_PER_HOST {
        return;
    }
    list.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used_at.cmp(&a.last_used_at)));
    list.truncate(MAX_ENTRIES_PER_HOST);
}

/// 一次命令提交：已存在则累加 count 并刷新时间，否则插入
fn record_entry(list: &mut Vec<HistoryEntry>, cmd: &str, at: u64) {
    if let Some(entry) = list.iter_mut().find(|e| e.cmd == cmd) {
        entry.count = entry.count.saturating_add(1);
        entry.last_used_at = at;
    } else {
        list.push(HistoryEntry {
            cmd: cmd.to_string(),
            count: 1,
            last_used_at: at,
        });
    }
    trim_entries(list);
}

/// 远端历史导入：已存在则原样保留（不吃掉本地累计的频次），不存在时以 count=1 插入
fn import_entry(list: &mut Vec<HistoryEntry>, cmd: &str, at: u64) {
    if list.iter().any(|e| e.cmd == cmd) {
        return;
    }
    list.push(HistoryEntry {
        cmd: cmd.to_string(),
        count: 1,
        last_used_at: at,
    });
    trim_entries(list);
}

fn read_map(path: &Path) -> HistoryMap {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<HistoryMap>(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn write_map(path: &Path, map: &HistoryMap) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    fs::write(path, &json).map_err(|e| e.to_string())?;
    crate::fsperm::restrict_file(path);
    Ok(())
}

/// 读取某主机的命令历史，按 (count desc, lastUsedAt desc) 排序返回
pub fn load_history(host_key: &str) -> Vec<HistoryEntry> {
    let map = read_map(&history_file_path());
    let mut list = map.get(host_key).cloned().unwrap_or_default();
    list.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used_at.cmp(&a.last_used_at)));
    list
}

/// 记录一条命令，返回该主机更新后的完整历史（供前端就地刷新缓存，省一次 IPC）
pub fn push_history(host_key: &str, cmd: &str) -> Result<Vec<HistoryEntry>, String> {
    if !should_record(cmd) {
        return Ok(load_history(host_key));
    }
    let path = history_file_path();
    let mut map = read_map(&path);
    let list = map.entry(host_key.to_string()).or_default();
    record_entry(list, cmd.trim(), now_secs());
    let updated = list.clone();
    write_map(&path, &map)?;
    let mut updated = updated;
    updated.sort_by(|a, b| b.count.cmp(&a.count).then(b.last_used_at.cmp(&a.last_used_at)));
    Ok(updated)
}

/// 批量导入远端 `~/.bash_history` 作为词库（仅插入不存在的项，不累加频次）
pub fn import_history(host_key: &str, cmds: Vec<String>) -> Result<(), String> {
    let path = history_file_path();
    let mut map = read_map(&path);
    let at = now_secs();
    let list = map.entry(host_key.to_string()).or_default();
    for cmd in cmds {
        if should_record(&cmd) {
            import_entry(list, cmd.trim(), at);
        }
    }
    write_map(&path, &map)
}

/// 清空指定主机或全部历史
pub fn clear_history(host_key: Option<&str>) -> Result<(), String> {
    let path = history_file_path();
    if let Some(key) = host_key {
        let mut map = read_map(&path);
        map.remove(key);
        return write_map(&path, &map);
    }
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_record_filters_blank_leading_space_and_oversize() {
        assert!(!should_record(""));
        assert!(!should_record("   "));
        // HISTCONTROL=ignorespace：以空格开头的命令不入库
        assert!(!should_record(" ls -al"));
        assert!(should_record("ls -al"));
        let long = "a".repeat(MAX_COMMAND_LEN + 1);
        assert!(!should_record(&long));
        assert!(should_record(&"a".repeat(MAX_COMMAND_LEN)));
    }

    #[test]
    fn should_record_rejects_sensitive_commands() {
        assert!(!should_record("mysql -uroot -psecret"));
        assert!(!should_record("sshpass -p mypass ssh root@1.2.3.4"));
        assert!(!should_record("curl -d token=abcdef http://x"));
        assert!(!should_record("echo PASSWORD"));
        // 端口号不误伤
        assert!(should_record("ssh -p 2222 root@1.2.3.4"));
        assert!(should_record("ssh -p2222 root@1.2.3.4"));
    }

    #[test]
    fn record_entry_dedups_and_increments() {
        let mut list = Vec::new();
        record_entry(&mut list, "docker ps", 10);
        record_entry(&mut list, "docker ps", 20);
        record_entry(&mut list, "ls", 15);
        assert_eq!(list.len(), 2);
        let docker = list.iter().find(|e| e.cmd == "docker ps").unwrap();
        assert_eq!(docker.count, 2);
        assert_eq!(docker.last_used_at, 20);
    }

    #[test]
    fn trim_entries_keeps_most_used() {
        let mut list: Vec<HistoryEntry> = (0..MAX_ENTRIES_PER_HOST + 20)
            .map(|i| HistoryEntry {
                cmd: format!("cmd-{}", i),
                count: i as u32,
                last_used_at: i as u64,
            })
            .collect();
        trim_entries(&mut list);
        assert_eq!(list.len(), MAX_ENTRIES_PER_HOST);
        // 频次最高的 cmd-519 必须保留
        assert!(list.iter().any(|e| e.cmd == format!("cmd-{}", MAX_ENTRIES_PER_HOST + 19)));
    }

    #[test]
    fn import_entry_does_not_touch_existing_count() {
        let mut list = vec![HistoryEntry {
            cmd: "ls".into(),
            count: 7,
            last_used_at: 1,
        }];
        import_entry(&mut list, "ls", 99);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].count, 7);
        assert_eq!(list[0].last_used_at, 1);
        import_entry(&mut list, "pwd", 99);
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().find(|e| e.cmd == "pwd").unwrap().count, 1);
    }
}
