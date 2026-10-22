//! russh 连接层：TCP 建连 / 握手 / 主机密钥校验 / 身份认证 / 一次性 exec / SFTP 会话。
//!
//! 与旧 libssh2 实现相比有两点关键差异：
//! 1. libssh2 在 Windows 上走 WinCNG 后端，只支持 RSA 主机密钥、且不支持 OpenSSH
//!    证书主机密钥，遇到只提供 `ssh-ed25519-cert-v01@openssh.com` 的服务器会握手失败；
//!    russh 原生支持 ed25519 / 证书主机密钥，因此连接层整体切到 russh。
//! 2. 主机密钥校验改为自己读写 `~/.ssh/known_hosts`（libssh2 的 KnownHosts
//!    绑定在它自己的会话对象上），保持 OpenSSH 的记录格式与 `[host]:port` 语义。

use crate::rt;
use crate::ssh::{HostKeyInfo, SshConnectConfig};
use base64::Engine as _;
use parking_lot::Mutex;
use russh::client::{self, Handle};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::{
    certificate::CertType, public::KeyData, Algorithm, Certificate, HashAlg,
};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{Channel, ChannelMsg, Preferred};
use russh_sftp::client::SftpSession;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// TCP 建连超时（与旧实现一致）
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 一次性 exec 的读取超时：超时返回已读到的内容，避免探测脚本挂住调用方
pub const EXEC_TIMEOUT: Duration = Duration::from_secs(5);
/// SFTP 子系统的单次操作超时
const SFTP_TIMEOUT_SECS: u64 = 30;
/// 服务端长时间无数据时的保活间隔
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

pub type ShellChannel = Channel<client::Msg>;

/// 客户端身份校验回调：在密钥交换阶段确认主机（或主机证书）是否可信，
/// 结果通过 Arc 槽位回传给建连流程（russh 只允许返回 bool）。
pub struct HostKeyVerifier {
    config: SshConnectConfig,
    rejection: Arc<Mutex<Option<String>>>,
    info: Arc<Mutex<Option<HostKeyInfo>>>,
}

impl client::Handler for HostKeyVerifier {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        match verify_host_key(&self.config, server_public_key) {
            Ok(info) => {
                *self.info.lock() = Some(info);
                Ok(true)
            }
            Err(reason) => {
                *self.rejection.lock() = Some(reason);
                Ok(false)
            }
        }
    }
}

/// 每条 SSH 连接（终端 shell 与 SFTP 各一条，与旧实现保持同样的连接拓扑）
pub struct Conn {
    handle: Handle<HostKeyVerifier>,
    host_key: HostKeyInfo,
    sftp: Mutex<Option<Arc<SftpSession>>>,
}

impl Conn {
    /// 同步建连（内部驱动异步实现），供阻塞命令线程调用。
    pub fn connect(config: &SshConnectConfig) -> Result<Arc<Conn>, String> {
        rt::block_on(Self::connect_async(config.clone()))
    }

    pub async fn connect_async(config: SshConnectConfig) -> Result<Arc<Conn>, String> {
        let tcp = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio::net::TcpStream::connect((config.host.as_str(), config.port)),
        )
        .await
        .map_err(|_| "TCP 连接超时（10s）".to_string())?
        .map_err(|e| format!("TCP 连接失败: {}", e))?;
        // 关闭 Nagle，降低交互式输入的往返延迟
        let _ = tcp.set_nodelay(true);

        let rejection: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let info: Arc<Mutex<Option<HostKeyInfo>>> = Arc::new(Mutex::new(None));
        let handler = HostKeyVerifier {
            config: config.clone(),
            rejection: Arc::clone(&rejection),
            info: Arc::clone(&info),
        };

        let mut handle = client::connect_stream(client_config(), tcp, handler)
            .await
            .map_err(|e| {
                // 主机密钥未通过时 russh 只报“连接被拒绝”，真实原因在这里
                let reason = rejection.lock().clone();
                reason.unwrap_or_else(|| format!("SSH 握手失败: {}", e))
            })?;

        authenticate(&mut handle, &config).await?;

        let host_key = info.lock().clone().unwrap_or(HostKeyInfo {
            fingerprint: String::new(),
            status: "trusted".into(),
        });

        Ok(Arc::new(Conn {
            handle,
            host_key,
            sftp: Mutex::new(None),
        }))
    }

    pub fn host_key(&self) -> &HostKeyInfo {
        &self.host_key
    }

    pub fn handle(&self) -> &Handle<HostKeyVerifier> {
        &self.handle
    }

    /// 申请 PTY 并启动交互式 shell
    pub async fn open_shell(
        handle: &Handle<HostKeyVerifier>,
        cols: u32,
        rows: u32,
    ) -> Result<ShellChannel, String> {
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开 Shell 通道失败: {}", e))?;
        channel
            .request_pty(
                true,
                "xterm-256color",
                cols.max(1),
                rows.max(1),
                0,
                0,
                &[],
            )
            .await
            .map_err(|e| format!("申请 PTY 失败: {}", e))?;
        channel
            .request_shell(true)
            .await
            .map_err(|e| format!("启动 Shell 失败: {}", e))?;
        Ok(channel)
    }

    /// 同步执行一次性命令并读回输出（通道开在本连接上，CWD 探测依赖这一点）。
    ///
    /// 只能在阻塞线程（`run_blocking` / 普通线程）里调用：内部用 `block_on` 驱动，
    /// 在异步任务内调用会 panic。异步上下文请用 `exec_async`。
    pub fn exec(&self, command: &str) -> Result<String, String> {
        self.exec_with_timeout(command, EXEC_TIMEOUT)
    }

    pub fn exec_with_timeout(&self, command: &str, timeout: Duration) -> Result<String, String> {
        rt::block_on(self.exec_async(command, timeout))
    }

    pub async fn exec_async(&self, command: &str, timeout: Duration) -> Result<String, String> {
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开命令通道失败: {}", e))?;
        channel
            .exec(true, command)
            .await
            .map_err(|e| format!("执行命令失败: {}", e))?;

        let mut output: Vec<u8> = Vec::new();
        let deadline = Instant::now() + timeout;
        loop {
            let remain = deadline.saturating_duration_since(Instant::now());
            if remain.is_zero() {
                break;
            }
            match tokio::time::timeout(remain, channel.wait()).await {
                Err(_) => break,
                Ok(None) => break,
                Ok(Some(ChannelMsg::Data { data })) => output.extend_from_slice(&data),
                Ok(Some(ChannelMsg::ExtendedData { data, .. })) => {
                    output.extend_from_slice(&data)
                }
                Ok(Some(ChannelMsg::Eof)) | Ok(Some(ChannelMsg::Close)) => break,
                Ok(Some(_)) => {}
            }
        }
        let _ = channel.close().await;
        Ok(String::from_utf8_lossy(&output).to_string())
    }

    /// 获取（必要时懒加载）本连接的 SFTP 子系统。
    /// 与 `exec` 相同：只能在阻塞线程里调用。
    pub fn sftp(&self) -> Result<crate::sftp_compat::Sftp, String> {
        if let Some(existing) = self.sftp.lock().as_ref() {
            return Ok(crate::sftp_compat::Sftp::new(Arc::clone(existing)));
        }
        let session = rt::block_on(self.open_sftp_session())?;
        let mut slot = self.sftp.lock();
        if let Some(existing) = slot.as_ref() {
            return Ok(crate::sftp_compat::Sftp::new(Arc::clone(existing)));
        }
        *slot = Some(Arc::clone(&session));
        Ok(crate::sftp_compat::Sftp::new(session))
    }

    async fn open_sftp_session(&self) -> Result<Arc<SftpSession>, String> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
        let session = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| format!("初始化 SFTP 失败: {}", e))?;
        session.set_timeout(SFTP_TIMEOUT_SECS);
        Ok(Arc::new(session))
    }

    /// 主动断开：忽略错误（远端可能已经关了）
    pub fn disconnect(&self) {
        rt::block_on(self.disconnect_async());
    }

    pub async fn disconnect_async(&self) {
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
    }
}

fn client_config() -> Arc<client::Config> {
    let mut preferred = Preferred::DEFAULT;
    // 先声明愿意接受的主机证书类型：服务器用证书证明身份时才会把证书发下来，
    // 由 verify_host_key 依据 known_hosts / CA 判断是否可信。
    preferred.host_key_certificates = Cow::Owned(vec![
        Algorithm::Ed25519,
        Algorithm::Ecdsa {
            curve: russh::keys::ssh_key::EcdsaCurve::NistP256,
        },
        Algorithm::Ecdsa {
            curve: russh::keys::ssh_key::EcdsaCurve::NistP384,
        },
        Algorithm::Rsa {
            hash: Some(HashAlg::Sha512),
        },
        Algorithm::Rsa {
            hash: Some(HashAlg::Sha256),
        },
    ]);
    Arc::new(client::Config {
        preferred,
        nodelay: true,
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        ..client::Config::default()
    })
}

fn load_private_key(config: &SshConnectConfig) -> Result<russh::keys::PrivateKey, String> {
    let raw = config.private_key.as_deref().unwrap_or("").trim();
    if raw.is_empty() {
        return Err("未配置私钥内容或文件路径".into());
    }
    let passphrase = config.passphrase.as_deref().filter(|p| !p.is_empty());

    // 私钥既可能是文件路径，也可能是粘贴的密钥内容
    if Path::new(raw).is_file() {
        russh::keys::load_secret_key(raw, passphrase)
            .map_err(|e| format!("读取私钥失败: {}", e))
    } else {
        let key = russh::keys::PrivateKey::from_openssh(raw)
            .map_err(|e| format!("解析私钥失败: {}", e))?;
        if key.is_encrypted() {
            let passphrase = passphrase.ok_or_else(|| "私钥已加密，请填写口令".to_string())?;
            key.decrypt(passphrase)
                .map_err(|e| format!("私钥解密失败: {}", e))
        } else {
            Ok(key)
        }
    }
}

/// 依次尝试认证方式。OpenSSH 客户端也会先试 `none`：部分服务端（如只认连接令牌的
/// 容器网关）不需要任何凭证，只要用户名对就放行。
async fn authenticate(
    handle: &mut Handle<HostKeyVerifier>,
    config: &SshConnectConfig,
) -> Result<(), String> {
    if let Ok(result) = handle.authenticate_none(&config.username).await {
        if result.success() {
            return Ok(());
        }
    }

    match config.auth_type.as_str() {
        "password" => {
            let result = handle
                .authenticate_password(&config.username, config.password.clone().unwrap_or_default())
                .await
                .map_err(|e| format!("密码认证失败: {}", e))?;
            if result.success() {
                Ok(())
            } else {
                Err("身份认证未通过，请检查账号密码".into())
            }
        }
        "key" => {
            let key = Arc::new(load_private_key(config)?);
            let is_rsa = key.algorithm().is_rsa();
            // 服务端会通过 server-sig-algs 告知可用的 RSA 签名哈希；
            // 该扩展缺失时按 OpenSSH 的默认优先级用 sha2-512，再退 sha2-256。
            let advertised = handle
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();
            let candidates: Vec<Option<HashAlg>> = if is_rsa {
                match advertised {
                    Some(alg) => vec![Some(alg)],
                    None => vec![Some(HashAlg::Sha512), Some(HashAlg::Sha256)],
                }
            } else {
                vec![None]
            };

            for hash_alg in candidates {
                let result = handle
                    .authenticate_publickey(
                        &config.username,
                        PrivateKeyWithHashAlg::new(Arc::clone(&key), hash_alg),
                    )
                    .await
                    .map_err(|e| format!("私钥认证失败: {}", e))?;
                if result.success() {
                    return Ok(());
                }
            }
            Err("身份认证未通过，请检查私钥是否已加入远端 authorized_keys".into())
        }
        "agent" => agent_authenticate(handle, config).await,
        other => Err(format!("不支持的认证方式: {}", other)),
    }
}

async fn agent_authenticate(
    handle: &mut Handle<HostKeyVerifier>,
    config: &SshConnectConfig,
) -> Result<(), String> {
    #[cfg(unix)]
    let agent = AgentClient::connect_env().await;
    #[cfg(windows)]
    // Windows 只有 Pageant 协议可用（russh 的 connect_env 仅 Unix 编译）
    let agent = AgentClient::connect_pageant().await;
    let mut agent = agent.map_err(|e| format!("SSH Agent 认证失败: 无法连接代理 ({})", e))?;

    let identities = agent
        .request_identities()
        .await
        .map_err(|e| format!("SSH Agent 认证失败: {}", e))?;
    if identities.is_empty() {
        return Err("SSH Agent 认证失败: 代理中没有任何身份".into());
    }

    for identity in identities {
        let (public_key, comment) = match identity {
            AgentIdentity::PublicKey { key, comment } => (key, comment),
            AgentIdentity::Certificate {
                certificate,
                comment,
            } => (
                russh::keys::PublicKey::new(certificate.public_key().clone(), ""),
                comment,
            ),
        };
        let result = handle
            .authenticate_publickey_with(&config.username, public_key, None, &mut agent)
            .await
            .map_err(|e| format!("SSH Agent 认证失败: {}", e))?;
        if result.success() {
            return Ok(());
        }
        eprintln!("[snail-ssh] Agent 身份被拒绝: {}", comment);
    }
    Err("SSH Agent 认证失败: 代理中的身份均未通过".into())
}

/// known_hosts 中的一条记录
struct KnownEntry {
    names: Vec<String>,
    marker: Option<String>,
    key_blob: Vec<u8>,
}

fn known_hosts_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("known_hosts")
}

fn parse_known_hosts(text: &str) -> Vec<KnownEntry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let first = match fields.next() {
            Some(value) => value,
            None => continue,
        };
        let (marker, names_field) = if first.starts_with('@') {
            (Some(first.to_string()), match fields.next() {
                Some(value) => value,
                None => continue,
            })
        } else {
            (None, first)
        };
        let key_type = match fields.next() {
            Some(value) => value,
            None => continue,
        };
        let key_b64 = match fields.next() {
            Some(value) => value,
            None => continue,
        };
        // 只保留 SSH 密钥/证书类型，其它行（如 PuTTY 变体）忽略
        if !key_type.starts_with("ssh-")
            && !key_type.starts_with("ecdsa-")
            && !key_type.starts_with("sk-")
        {
            continue;
        }
        let key_blob = match base64::engine::general_purpose::STANDARD.decode(key_b64) {
            Ok(blob) => blob,
            Err(_) => continue,
        };
        // 哈希存储的条目（`|1|salt|hash`）无法按主机名比对，只保留密钥供指纹比对
        let names = if names_field.starts_with("|1|") {
            Vec::new()
        } else {
            names_field.split(',').map(|s| s.to_string()).collect()
        };
        entries.push(KnownEntry {
            names,
            marker,
            key_blob,
        });
    }
    entries
}

/// known_hosts 里的主机名是否指向当前主机。
/// 语义对齐 OpenSSH：非默认端口的记录写作 `[host]:port`，裸主机名只在 22 端口匹配。
fn name_covers_host(name: &str, config: &SshConnectConfig) -> bool {
    // known_hosts 一条记录可以用逗号列多个别名
    name.split(',').map(str::trim).any(|part| name_part_covers_host(part, config))
}

fn name_part_covers_host(name: &str, config: &SshConnectConfig) -> bool {
    let port_entry = format!("[{}]:{}", config.host, config.port);
    if name == port_entry {
        return true;
    }
    if config.port == 22 && name == config.host {
        return true;
    }
    // 支持 OpenSSH 的通配写法（*.example.com / 10.0.0.?）
    if name.contains('*') || name.contains('?') {
        return wildcard_match(name, &config.host);
    }
    false
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let v: Vec<char> = value.chars().collect();
    let (mut pi, mut vi) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while vi < v.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == v[vi]) {
            pi += 1;
            vi += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = vi;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            vi = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn fingerprint_sha256(blob: &[u8]) -> String {
    let digest = Sha256::digest(blob);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    )
}

fn key_data_blob(data: &KeyData) -> Vec<u8> {
    russh::keys::PublicKey::new(data.clone(), "").to_bytes().unwrap_or_default()
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 证书是否对本机有效：类型为 host、主机名在 principals 内、且在有效期内。
fn certificate_covers_host(cert: &Certificate, config: &SshConnectConfig) -> bool {
    if cert.cert_type() != CertType::Host {
        return false;
    }
    let now = now_unix();
    if now < cert.valid_after() || now > cert.valid_before() {
        return false;
    }
    cert.valid_principals().iter().any(|principal| {
        principal == &config.host || wildcard_match(principal, &config.host)
    })
}

/// 校验远端主机密钥/证书：
/// - 一致（含 CA 证书签发）：放行，status = matched
/// - 未知：仅在用户已确认（trust_host_key）时写入 known_hosts，否则返回
///   `HOST_KEY_UNKNOWN|<指纹>` 由前端弹窗确认
/// - 不一致：默认返回 `HOST_KEY_MISMATCH|<新指纹>|<旧指纹...>`；
///   用户确认替换后移除同名旧条目并写入新密钥（等价于 ssh-keygen -R + 重新信任）
fn verify_host_key(
    config: &SshConnectConfig,
    presented: &PublicKeyOrCertificate,
) -> Result<HostKeyInfo, String> {
    let plain = presented.public_key();
    let plain_blob = plain
        .to_bytes()
        .map_err(|e| format!("无法解析远端主机公钥: {}", e))?;
    let fingerprint = fingerprint_sha256(&plain_blob);
    let certificate = presented.certificate();

    let kh_path = known_hosts_path();
    let text = std::fs::read_to_string(&kh_path).unwrap_or_default();
    let entries = parse_known_hosts(&text);

    let mut matched = false;
    let mut revoked = false;
    let mut old_fingerprints: Vec<String> = Vec::new();
    let mut ca_digests: Vec<Vec<u8>> = Vec::new();
    for entry in &entries {
        let covers = entry.names.iter().any(|name| name_covers_host(name, config));
        if !covers {
            continue;
        }
        match entry.marker.as_deref() {
            Some("@revoked") => {
                if entry.key_blob == plain_blob {
                    revoked = true;
                }
                continue;
            }
            Some("@cert-authority") => {
                ca_digests.push(entry.key_blob.clone());
                continue;
            }
            _ => {}
        }
        if entry.key_blob == plain_blob {
            matched = true;
        }
        let fp = fingerprint_sha256(&entry.key_blob);
        if !old_fingerprints.contains(&fp) && old_fingerprints.len() < 8 {
            old_fingerprints.push(fp);
        }
    }

    if revoked {
        return Err(format!(
            "该主机密钥已在 ~/.ssh/known_hosts 中标记为 @revoked（{}），拒绝连接",
            fingerprint
        ));
    }

    // 服务器用证书证明身份时，等价于「证书里的公钥」可比对：
    // 一是直接命中已知的裸公钥（OpenSSH 的 fallback 行为），二是由已知 CA 签发。
    if !matched {
        if let Some(cert) = certificate {
            let ca_blob = key_data_blob(cert.signature_key());
            if ca_digests.iter().any(|known| *known == ca_blob)
                && certificate_covers_host(cert, config)
            {
                matched = true;
            }
        }
    }

    if matched {
        return Ok(HostKeyInfo {
            fingerprint,
            status: "matched".into(),
        });
    }

    let host_entry = if config.port == 22 {
        config.host.clone()
    } else {
        format!("[{}]:{}", config.host, config.port)
    };

    if !old_fingerprints.is_empty() {
        if !config.trust_host_key {
            return Err(format!(
                "HOST_KEY_MISMATCH|{}|{}",
                fingerprint,
                old_fingerprints.join(";")
            ));
        }
        // 用户已确认替换：先移除同名旧条目，再写入新密钥
        rewrite_known_hosts_without(&kh_path, config, &text)?;
        append_known_host(&kh_path, &host_entry, &plain, &plain_blob)?;
        return Ok(HostKeyInfo {
            fingerprint,
            status: "replaced".into(),
        });
    }

    if !config.trust_host_key {
        return Err(format!("HOST_KEY_UNKNOWN|{}", fingerprint));
    }
    append_known_host(&kh_path, &host_entry, &plain, &plain_blob)?;
    Ok(HostKeyInfo {
        fingerprint,
        status: "trusted".into(),
    })
}

fn append_known_host(
    kh_path: &Path,
    host_entry: &str,
    plain: &russh::keys::PublicKey,
    plain_blob: &[u8],
) -> Result<(), String> {
    if let Some(parent) = kh_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 ~/.ssh 目录失败: {}", e))?;
    }
    let algorithm = plain.algorithm();
    let key_type = algorithm.as_str();
    let line = format!(
        "{} {} {}\n",
        host_entry,
        key_type,
        base64::engine::general_purpose::STANDARD.encode(plain_blob)
    );
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(kh_path)
        .map_err(|e| format!("打开 known_hosts 失败: {}", e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入 known_hosts 失败: {}", e))
}

/// 删除指向当前主机的明文条目后重写 known_hosts（哈希条目保持原样，与旧实现一致）
fn rewrite_known_hosts_without(
    kh_path: &Path,
    config: &SshConnectConfig,
    original: &str,
) -> Result<(), String> {
    let mut kept: Vec<&str> = Vec::new();
    for line in original.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // 行格式：[@marker] hostnames keytype base64 [comment]
        let name_index = if fields.first().is_some_and(|f| f.starts_with('@')) {
            1
        } else {
            0
        };
        let covers = fields
            .get(name_index)
            .map(|names| {
                !names.starts_with("|1|") && names.split(',').any(|name| name_covers_host(name, config))
            })
            .unwrap_or(false);
        if !covers {
            kept.push(line);
        }
    }
    let mut content = kept.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    std::fs::write(kh_path, content).map_err(|e| format!("更新 known_hosts 失败: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> SshConnectConfig {
        SshConnectConfig {
            host: "example.com".into(),
            port: 22,
            username: "root".into(),
            auth_type: "password".into(),
            password: None,
            private_key: None,
            passphrase: None,
            trust_host_key: false,
            cols: 80,
            rows: 24,
        }
    }

    #[test]
    fn fingerprint_matches_openssh_format_for_empty_input() {
        // SHA256("") 的 base64（无填充）是固定值，可用于校验指纹格式
        assert_eq!(
            fingerprint_sha256(b""),
            "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"
        );
    }

    #[test]
    fn name_covers_host_matches_plain_and_ported_entries() {
        let base = base_config();
        assert!(name_covers_host("example.com", &base));
        assert!(name_covers_host("example.com,10.0.0.1", &base));
        assert!(!name_covers_host("example.org", &base));

        let custom = SshConnectConfig {
            port: 2222,
            ..base
        };
        assert!(name_covers_host("[example.com]:2222", &custom));
        assert!(!name_covers_host("example.com", &custom));
    }

    #[test]
    fn name_covers_host_supports_wildcards() {
        let mut config = base_config();
        config.host = "api.example.com".into();
        assert!(name_covers_host("*.example.com", &config));
        assert!(name_covers_host("api.example.???", &config));
        assert!(!name_covers_host("*.example.org", &config));
    }

    #[test]
    fn parse_known_hosts_skips_comments_and_bad_lines() {
        let text = concat!(
            "# comment\n",
            "\n",
            "example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAA\n",
            "@cert-authority *.internal ecdsa-sha2-nistp256 AAAAE2VjZHNh\n",
            "bad-line-no-key\n",
        );
        let entries = parse_known_hosts(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].names, vec!["example.com"]);
        assert!(entries[0].marker.is_none());
        assert_eq!(entries[1].names, vec!["*.internal"]);
        assert_eq!(entries[1].marker.as_deref(), Some("@cert-authority"));
    }

    #[test]
    fn rewrite_known_hosts_drops_only_matching_entries() {
        let dir = std::env::temp_dir().join(format!("snail-kh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("known_hosts");
        let original = concat!(
            "[example.com]:2222 ssh-ed25519 AAAAOLD\n",
            "other.host ssh-ed25519 AAAAKEEP\n",
            "|1|hashsalt|hashvalue ssh-ed25519 AAAAHASHED\n",
        );
        let config = SshConnectConfig {
            port: 2222,
            ..base_config()
        };
        rewrite_known_hosts_without(&path, &config, original).unwrap();
        let kept = std::fs::read_to_string(&path).unwrap();
        assert!(!kept.contains("AAAAOLD"));
        assert!(kept.contains("AAAAKEEP"));
        assert!(kept.contains("AAAAHASHED"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::sftp_compat::{OpenFlags, OpenType};
    use std::io::{Read, Seek, SeekFrom, Write};

    fn live_config() -> Option<SshConnectConfig> {
        let host = std::env::var("SNAIL_TEST_HOST").ok()?;
        Some(SshConnectConfig {
            host,
            port: std::env::var("SNAIL_TEST_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(22),
            username: std::env::var("SNAIL_TEST_USER").unwrap_or_default(),
            auth_type: std::env::var("SNAIL_TEST_AUTH").unwrap_or_else(|_| "key".into()),
            password: std::env::var("SNAIL_TEST_PASSWORD").ok(),
            private_key: std::env::var("SNAIL_TEST_KEY").ok(),
            passphrase: None,
            trust_host_key: false,
            cols: 120,
            rows: 30,
        })
    }

    #[test]
    #[ignore]
    fn live_conn_exec_and_sftp_roundtrip() {
        let Some(config) = live_config() else {
            return;
        };
        let conn = Conn::connect(&config).expect("connect");
        println!("host_key = {:?}", conn.host_key());

        let out = conn.exec("uname -s; echo EXEC_OK").expect("exec");
        println!("exec output = {:?}", out);
        assert!(out.contains("EXEC_OK"));

        let sftp = conn.sftp().expect("sftp");
        let home = sftp.realpath(Path::new(".")).expect("realpath");
        println!("sftp home = {}", home.display());
        let entries = sftp.readdir(&home).expect("readdir");
        println!("readdir count = {}", entries.len());

        // 家目录可能是只读挂载，固定用 /tmp 做写读校验
        let dir = Path::new("/tmp").join(".snail-ssh-live-test");
        let file = dir.join("hello.txt");
        let _ = sftp.unlink(&file);
        let _ = sftp.rmdir(&dir);
        sftp.mkdir(&dir, 0o755).expect("mkdir");

        {
            let mut handle = sftp.create(&file).expect("create");
            handle.write_all(b"snail-ssh russh ok").expect("write");
            handle.flush().expect("flush");
        }
        // 断点续传路径：open_mode 打开（不带 TRUNC）+ seek 到已有长度后追加
        {
            let mut handle = sftp
                .open_mode(
                    &file,
                    OpenFlags::WRITE | OpenFlags::CREATE,
                    0o644,
                    OpenType::File,
                )
                .expect("open_mode");
            handle.seek(SeekFrom::Start(18)).expect("seek");
            handle.write_all(b"!").expect("append");
        }
        {
            let mut handle = sftp.open(&file).expect("open");
            let stat = handle.stat().expect("fstat");
            assert_eq!(stat.size, Some(19));
            let mut buf = String::new();
            handle.read_to_string(&mut buf).expect("read");
            assert_eq!(buf, "snail-ssh russh ok!");
        }
        let st = sftp.stat(&file).expect("stat");
        assert!(st.is_file());
        println!("file stat size={:?} perm={:?}", st.size, st.perm);

        let renamed = dir.join("renamed.txt");
        let _ = sftp.unlink(&renamed);
        sftp.rename(&file, &renamed, None).expect("rename");
        sftp.unlink(&renamed).expect("unlink");
        sftp.rmdir(&dir).expect("rmdir");

        conn.disconnect();
    }

}
