//! SFTP 兼容层：把 russh-sftp 的异步 API 包成与原 libssh2 版本同形的同步接口。
//!
//! `sftp.rs` 里的业务逻辑（列表、上传下载、续传、权限修改）都按 libssh2 的
//! `Sftp` / `File` / `FileStat` 语义写成，这里只做薄适配，避免为了换 SSH 库
//! 重写整层文件操作：
//! - 路径参数统一用 `&Path`，返回值仍是 `Result<_, String>`（错误文案沿用原格式）
//! - `File` 实现 `std::io::{Read, Write, Seek}`，续传逻辑可以直接沿用
//! - `FileStat` 保留 ssh2 的字段名（size/uid/gid/perm/atime/mtime）。

use crate::rt;
use russh_sftp::client::{fs::File as InnerFile, SftpSession};
use russh_sftp::protocol::{FileAttributes, OpenFlags as SftpOpenFlags};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 等价于 `ssh2::FileStat`：字段名保持一致，`sftp.rs` 无需改取值逻辑
#[derive(Debug, Clone, Default)]
pub struct FileStat {
    pub size: Option<u64>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub perm: Option<u32>,
    pub atime: Option<u64>,
    pub mtime: Option<u64>,
}

/// 等价于 `ssh2::FileType`（只区分业务用到的几类）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Directory,
    RegularFile,
    Symlink,
    Other,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        matches!(self, FileType::Directory)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, FileType::RegularFile)
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self, FileType::Symlink)
    }
}

impl FileStat {
    pub fn file_type(&self) -> FileType {
        match self.perm.map(|p| p & 0o170000) {
            Some(0o040000) => FileType::Directory,
            Some(0o100000) => FileType::RegularFile,
            Some(0o120000) => FileType::Symlink,
            _ => FileType::Other,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.file_type().is_dir()
    }

    pub fn is_file(&self) -> bool {
        self.file_type().is_file()
    }
}

fn from_attributes(attrs: FileAttributes) -> FileStat {
    FileStat {
        size: attrs.size,
        uid: attrs.uid,
        gid: attrs.gid,
        perm: attrs.permissions,
        atime: attrs.atime.map(u64::from),
        mtime: attrs.mtime.map(u64::from),
    }
}

fn to_attributes(stat: FileStat) -> FileAttributes {
    FileAttributes {
        size: stat.size,
        uid: stat.uid,
        gid: stat.gid,
        permissions: stat.perm,
        atime: stat.atime.map(|v| v as u32),
        mtime: stat.mtime.map(|v| v as u32),
        ..Default::default()
    }
}

/// 等价于 `ssh2::OpenFlags`
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags(u32);

impl OpenFlags {
    pub const READ: OpenFlags = OpenFlags(0b0000_0001);
    pub const WRITE: OpenFlags = OpenFlags(0b0000_0010);
    pub const APPEND: OpenFlags = OpenFlags(0b0000_0100);
    pub const CREATE: OpenFlags = OpenFlags(0b0000_1000);
    pub const TRUNCATE: OpenFlags = OpenFlags(0b0001_0000);
}

impl std::ops::BitOr for OpenFlags {
    type Output = OpenFlags;
    fn bitor(self, other: OpenFlags) -> OpenFlags {
        OpenFlags(self.0 | other.0)
    }
}

impl OpenFlags {
    fn to_sftp(self) -> SftpOpenFlags {
        let mut flags = SftpOpenFlags::empty();
        if self.0 & Self::READ.0 != 0 {
            flags |= SftpOpenFlags::READ;
        }
        if self.0 & Self::WRITE.0 != 0 {
            flags |= SftpOpenFlags::WRITE;
        }
        if self.0 & Self::APPEND.0 != 0 {
            flags |= SftpOpenFlags::APPEND;
        }
        if self.0 & Self::CREATE.0 != 0 {
            flags |= SftpOpenFlags::CREATE;
        }
        if self.0 & Self::TRUNCATE.0 != 0 {
            flags |= SftpOpenFlags::TRUNCATE;
        }
        flags
    }
}

/// 等价于 `ssh2::OpenType`（当前只用到普通文件）
#[derive(Debug, Clone, Copy)]
pub enum OpenType {
    File,
}

/// 等价于 `ssh2::RenameFlags`：SFTPv3 的 rename 本身就是覆盖语义，
/// 这里只保留调用方的表达方式，不做额外处理。
#[derive(Debug, Clone, Copy)]
pub struct RenameFlags;

impl RenameFlags {
    pub const OVERWRITE: RenameFlags = RenameFlags;
    pub fn empty() -> RenameFlags {
        RenameFlags
    }
}

/// 同步 SFTP 门面，方法签名对齐 libssh2 版本
#[derive(Clone)]
pub struct Sftp {
    session: Arc<SftpSession>,
}

impl Sftp {
    pub fn new(session: Arc<SftpSession>) -> Self {
        Self { session }
    }

    pub fn readdir(&self, path: &Path) -> Result<Vec<(PathBuf, FileStat)>, String> {
        let parent = path_to_string(path);
        let dir = block(self.session.read_dir(parent.clone()))?;
        let prefix = if parent.ends_with('/') {
            parent
        } else {
            format!("{}/", parent)
        };
        let mut entries = Vec::new();
        for entry in dir {
            // 用 POSIX 拼接，避免 Windows 的反斜杠污染远端路径
            let child = PathBuf::from(format!("{}{}", prefix, entry.file_name()));
            let stat = from_attributes(entry.metadata());
            entries.push((child, stat));
        }
        Ok(entries)
    }

    pub fn stat(&self, path: &Path) -> Result<FileStat, String> {
        block(self.session.metadata(path_to_string(path))).map(from_attributes)
    }

    pub fn lstat(&self, path: &Path) -> Result<FileStat, String> {
        block(self.session.symlink_metadata(path_to_string(path))).map(from_attributes)
    }

    pub fn setstat(&self, path: &Path, stat: FileStat) -> Result<(), String> {
        block(self.session.set_metadata(path_to_string(path), to_attributes(stat)))
    }

    pub fn mkdir(&self, path: &Path, _mode: u32) -> Result<(), String> {
        block(self.session.create_dir(path_to_string(path)))
    }

    pub fn rmdir(&self, path: &Path) -> Result<(), String> {
        block(self.session.remove_dir(path_to_string(path)))
    }

    pub fn unlink(&self, path: &Path) -> Result<(), String> {
        block(self.session.remove_file(path_to_string(path)))
    }

    pub fn rename(&self, from: &Path, to: &Path, _flags: Option<RenameFlags>) -> Result<(), String> {
        // SFTPv3 的 rename 本身就是覆盖语义，与 OVERWRITE 一致
        block(self.session.rename(path_to_string(from), path_to_string(to)))
    }

    pub fn realpath(&self, path: &Path) -> Result<PathBuf, String> {
        block(self.session.canonicalize(path_to_string(path))).map(PathBuf::from)
    }

    pub fn create(&self, path: &Path) -> Result<File, String> {
        block(self.session.create(path_to_string(path))).map(File::new)
    }

    pub fn open(&self, path: &Path) -> Result<File, String> {
        block(self.session.open(path_to_string(path))).map(File::new)
    }

    pub fn open_mode(
        &self,
        path: &Path,
        flags: OpenFlags,
        mode: u32,
        _typ: OpenType,
    ) -> Result<File, String> {
        let attrs = FileAttributes {
            permissions: Some(mode),
            ..Default::default()
        };
        block(
            self.session
                .open_with_flags_and_attributes(path_to_string(path), flags.to_sftp(), attrs),
        )
        .map(File::new)
    }
}

/// 远端文件句柄：同步读写/定位，行为与 `ssh2::File` 一致
pub struct File {
    inner: InnerFile,
}

impl File {
    fn new(inner: InnerFile) -> Self {
        Self { inner }
    }

    /// fstat：读取当前句柄的文件信息
    pub fn stat(&mut self) -> Result<FileStat, String> {
        block(self.inner.metadata()).map(from_attributes)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        block_io(tokio::io::AsyncReadExt::read(&mut self.inner, buf))
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        block_io(tokio::io::AsyncWriteExt::write(&mut self.inner, buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        block_io(tokio::io::AsyncWriteExt::flush(&mut self.inner))
    }
}

impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        block_io(tokio::io::AsyncSeekExt::seek(&mut self.inner, pos))
    }
}

/// 远端路径始终是 POSIX 形式：Windows 上 `Path::join` 会引入反斜杠，
/// 直接发给服务端会被当成文件名的一部分（表现为莫名其妙的 Permission denied）。
fn path_to_string(path: &Path) -> String {
    let separator = char::from(92u8);
    path.to_string_lossy()
        .chars()
        .map(|c| if c == separator { '/' } else { c })
        .collect()
}

fn block<T, F>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, russh_sftp::client::error::Error>>,
{
    rt::block_on(future).map_err(|e| e.to_string())
}

fn block_io<T, F>(future: F) -> std::io::Result<T>
where
    F: std::future::Future<Output = std::io::Result<T>>,
{
    rt::block_on(future)
}
