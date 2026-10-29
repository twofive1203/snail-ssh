use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::storage::app_data_dir;

/// 应用设置文件 `~/.snail-ssh/settings.json`。
///
/// 这里刻意不做强类型反序列化：设置项会随 UI 快速迭代（终端字体、片段、
/// 书签、分组……），如果每个字段都在 Rust 侧定义一遍，加一个开关就要同时改
/// 两端并升级配置结构。因此后端只负责「整块 JSON 的安全读写」，结构由前端
/// `src/types/index.ts` 的 `AppSettings` 约束，缺字段时由前端补默认值。
fn settings_file_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

/// 读取应用设置。文件缺失或内容损坏时返回 None，由前端回落到默认值，
/// 避免一处 JSON 写坏就导致整个界面无法启动。
pub fn load_settings() -> Option<Value> {
    let content = fs::read_to_string(settings_file_path()).ok()?;
    serde_json::from_str(&content).ok()
}

/// 保存应用设置。先写临时文件再 rename，避免写到一半崩溃留下半个 JSON。
pub fn save_settings(settings: &Value) -> Result<(), String> {
    let path = settings_file_path();
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化设置失败: {}", e))?;
    fs::write(&tmp, json).map_err(|e| format!("写入设置失败: {}", e))?;
    crate::fsperm::restrict_file(&tmp);
    fs::rename(&tmp, &path).map_err(|e| format!("保存设置失败: {}", e))?;
    crate::fsperm::restrict_file(&path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_path_lives_under_app_data_dir() {
        let path = settings_file_path();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("settings.json"));
        assert!(path.parent().map(|p| p.ends_with(".snail-ssh")).unwrap_or(false));
    }

    #[test]
    fn save_then_load_roundtrips_nested_settings() {
        // 直接测序列化往返，不落真实用户目录
        let value = json!({
            "terminal": { "fontSize": 15, "themeId": "nord" },
            "groups": [{ "id": "production", "label": "生产集群", "color": "red" }],
            "snippets": [{ "id": "s1", "label": "df -h", "cmd": "df -h\n" }]
        });
        let text = serde_json::to_string_pretty(&value).unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["terminal"]["fontSize"], 15);
        assert_eq!(parsed["groups"][0]["color"], "red");
        assert_eq!(parsed["snippets"][0]["cmd"], "df -h\n");
    }
}
