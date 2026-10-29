//! 把应用自己的目录和敏感文件收成仅当前用户可读写。
//!
//! Unix 用 0700 / 0600。Windows 用户配置目录通常已经只给当前用户，
//! 这里再调一次 icacls 去掉继承。失败只记日志：权限收紧不能反过来让保存失败。

use std::path::Path;

pub fn restrict_dir(path: &Path) {
    #[cfg(unix)]
    restrict_unix(path, 0o700);
    #[cfg(windows)]
    restrict_windows(path, true);
}

pub fn restrict_file(path: &Path) {
    #[cfg(unix)]
    restrict_unix(path, 0o600);
    #[cfg(windows)]
    restrict_windows(path, false);
}

#[cfg(unix)]
fn restrict_unix(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(err) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
        eprintln!(
            "[snail-ssh] 收紧权限失败 {}: {}",
            path.display(),
            err
        );
    }
}

/// icacls 的参数是独立 argv，不经过 shell。用户名里如果带 icacls 的元字符就放弃，
/// 避免把路径权限改到意料之外的账户上。
#[cfg(windows)]
fn restrict_windows(path: &Path, is_dir: bool) {
    let Some(user) = std::env::var("USERNAME").ok() else {
        return;
    };
    if user.is_empty()
        || user
            .chars()
            .any(|c| matches!(c, ':' | '"' | '*' | '?' | '<' | '>' | '|' | '/' | '\\'))
    {
        eprintln!("[snail-ssh] 当前用户名无法安全用于 icacls，跳过 ACL 收紧");
        return;
    }
    let grant = if is_dir {
        format!("{user}:(OI)(CI)F")
    } else {
        format!("{user}:(F)")
    };
    use std::os::windows::process::CommandExt;
    // 0x08000000 = CREATE_NO_WINDOW，避免图形界面每次保存都闪一下控制台。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    match std::process::Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(grant)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "[snail-ssh] icacls 未能收紧 {}: {}",
                path.display(),
                detail.trim()
            );
        }
        Err(err) => eprintln!(
            "[snail-ssh] 收紧 ACL 失败 {}: {}",
            path.display(),
            err
        ),
    }
}
