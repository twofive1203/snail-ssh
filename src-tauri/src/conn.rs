//! russh 连接层：TCP 建连 / 握手 / 主机密钥校验 / 身份认证 / 一次性 exec / SFTP 会话。
//!
//! 与旧 libssh2 实现相比有两点关键差异：
//! 1. libssh2 在 Windows 上走 WinCNG 后端，只支持 RSA 主机密钥、且不支持 OpenSSH
//!    证书主机密钥，遇到只提供 `ssh-ed25519-cert-v01@openssh.com` 的服务器会握手失败；
//!    russh 原生支持 ed25519 / 证书主机密钥，因此连接层整体切到 russh。
//! 2. 主机密钥校验改为自己读写 `~/.ssh/known_hosts`（libssh2 的 KnownHosts
//!    绑定在它自己的会话对象上），保持 OpenSSH 的记录格式与 `[host]:port` 语义。

use crate::authprompt::{AuthPrompter, PromptField};
use crate::forward::{self, ForwardControl, RemoteTarget};
use crate::model::{expand_tilde, JumpHop};
use crate::net;
use crate::rt;
use crate::ssh::{HostKeyInfo, SshConnectConfig};
use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use parking_lot::Mutex;
use sha1::Sha1;
use russh::client::{self, Handle};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::{
    certificate::CertType, public::KeyData, Algorithm, Certificate, HashAlg,
};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{Channel, ChannelMsg, ChannelOpenFailure, MethodKind, Preferred};
use std::sync::Arc as StdArc;
use russh_sftp::client::SftpSession;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    forwards: StdArc<ForwardControl>,
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

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        if let Some(target) = self.forwards.lookup_remote(connected_port) {
            reply.accept().await;
            rt::spawn(async move {
                match tokio::net::TcpStream::connect((target.target_host.as_str(), target.target_port)).await {
                    Ok(tcp) => forward::bridge_channel_tcp(channel, tcp).await,
                    Err(_) => {
                        let _ = channel.close().await;
                    }
                }
            });
        } else {
            let _ = reply.reject(ChannelOpenFailure::AdministrativelyProhibited).await;
        }
        Ok(())
    }
}

/// 每条 SSH 连接（终端 shell 与 SFTP 各一条，与旧实现保持同样的连接拓扑）
pub struct Conn {
    handle: Handle<HostKeyVerifier>,
    host_key: HostKeyInfo,
    sftp: Mutex<Option<Arc<SftpSession>>>,
    /// 跳板连接必须和目标连接活得一样久，否则 direct-tcpip 通道会一起拆掉。
    upstreams: Vec<Handle<HostKeyVerifier>>,
    forwards: StdArc<ForwardControl>,
}

impl Conn {
    /// 同步建连（内部驱动异步实现），供阻塞命令线程调用。
    pub fn connect(config: &SshConnectConfig) -> Result<Arc<Conn>, String> {
        rt::block_on(Self::connect_async(config.clone(), None))
    }

    pub fn connect_prompted(
        config: &SshConnectConfig,
        prompter: AuthPrompter,
    ) -> Result<Arc<Conn>, String> {
        rt::block_on(Self::connect_async(config.clone(), Some(prompter)))
    }

    pub async fn connect_async(
        config: SshConnectConfig,
        prompter: Option<AuthPrompter>,
    ) -> Result<Arc<Conn>, String> {
        if config.jumps.len() > 8 {
            return Err("跳板机最多支持 8 跳".into());
        }
        let mut chain: Vec<SshConnectConfig> = config.jumps.iter().map(jump_to_config).collect();
        let mut target = config.clone();
        target.jumps.clear();
        chain.push(target);

        let mut prev: Option<Handle<HostKeyVerifier>> = None;
        let mut upstreams = Vec::new();
        let mut final_parts: Option<(Handle<HostKeyVerifier>, HostKeyInfo, StdArc<ForwardControl>)> = None;

        for (index, hop) in chain.iter().enumerate() {
            let is_last = index + 1 == chain.len();
            let forwards = ForwardControl::new();
            let opened = if let Some(prev_handle) = prev.as_ref() {
                let channel = prev_handle
                    .channel_open_direct_tcpip(
                        hop.host.clone(),
                        u32::from(hop.port),
                        "127.0.0.1",
                        0,
                    )
                    .await
                    .map_err(|e| {
                        format!(
                            "经跳板打开到 {}:{} 的通道失败: {}",
                            hop.host, hop.port, e
                        )
                    })?;
                handshake(channel.into_stream(), hop, prompter.clone(), StdArc::clone(&forwards)).await
            } else {
                let tcp = net::dial(&hop.host, hop.port, config.proxy.as_ref()).await?;
                handshake(tcp, hop, prompter.clone(), StdArc::clone(&forwards)).await
            };
            let (handle, host_key) = match opened {
                Ok(parts) => parts,
                Err(err) => return Err(annotate_host_key_error(err, &hop.host, hop.port)),
            };
            if let Some(old) = prev.take() {
                upstreams.push(old);
            }
            if is_last {
                final_parts = Some((handle, host_key, forwards));
            } else {
                prev = Some(handle);
            }
        }

        let (handle, host_key, forwards) = final_parts.ok_or("没有可连接的目标主机")?;
        Ok(Arc::new(Conn {
            handle,
            host_key,
            sftp: Mutex::new(None),
            upstreams,
            forwards,
        }))
    }

    /// 启动本连接上已启用的端口转发。失败的条目记在返回值里，不拆掉已经建立的 SSH 会话。
    pub async fn start_forwards(self: &Arc<Self>, specs: &[crate::model::PortForward]) -> Vec<String> {
        let mut notes = Vec::new();
        for spec in specs.iter().filter(|item| item.enabled) {
            if let Err(err) = forward::validate_forward(spec) {
                notes.push(format!("{}: {}", spec.id, err));
                continue;
            }
            let result = match spec.kind.as_str() {
                "local" => self.spawn_local_forward(spec).await,
                "dynamic" => self.spawn_dynamic_forward(spec).await,
                "remote" => self.spawn_remote_forward(spec).await,
                other => Err(format!("未知转发类型: {}", other)),
            };
            match result {
                Ok(note) => notes.push(note),
                Err(err) => notes.push(format!("{}: {}", spec.id, err)),
            }
        }
        notes
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
        self.forwards.stop();
        forward::release_owner(&forward_owner(self));
        for (addr, port) in self.forwards.remote_binds() {
            let _ = self.handle.cancel_tcpip_forward(addr, port).await;
        }
        let _ = self
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await;
        for upstream in &self.upstreams {
            let _ = upstream
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
    }

    async fn spawn_local_forward(self: &Arc<Self>, spec: &crate::model::PortForward) -> Result<String, String> {
        let bind = format!("{}:{}", spec.bind_host, spec.bind_port);
        forward::reserve_local_bind(&bind, &forward_owner(self))?;
        let listener = tokio::net::TcpListener::bind((spec.bind_host.as_str(), spec.bind_port))
            .await
            .map_err(|e| {
                forward::release_bind(&bind);
                format!("绑定 {} 失败: {}", bind, e)
            })?;
        let conn = Arc::clone(self);
        let target_host = spec.target_host.clone();
        let target_port = spec.target_port;
        let mut stop = self.forwards.subscribe();
        rt::spawn(async move {
            loop {
                tokio::select! {
                    changed = stop.changed() => {
                        if changed.is_err() || *stop.borrow() {
                            break;
                        }
                    }
                    accepted = listener.accept() => {
                        let Ok((tcp, peer)) = accepted else { break; };
                        let conn = Arc::clone(&conn);
                        let target_host = target_host.clone();
                        rt::spawn(async move {
                            let opened = conn.handle().channel_open_direct_tcpip(
                                target_host,
                                u32::from(target_port),
                                peer.ip().to_string(),
                                u32::from(peer.port()),
                            ).await;
                            if let Ok(channel) = opened {
                                forward::bridge_channel_tcp(channel, tcp).await;
                            }
                        });
                    }
                }
            }
        });
        Ok(format!("本地转发 {} → {}:{}", bind, spec.target_host, spec.target_port))
    }

    async fn spawn_dynamic_forward(self: &Arc<Self>, spec: &crate::model::PortForward) -> Result<String, String> {
        let bind = format!("{}:{}", spec.bind_host, spec.bind_port);
        forward::reserve_local_bind(&bind, &forward_owner(self))?;
        let listener = tokio::net::TcpListener::bind((spec.bind_host.as_str(), spec.bind_port))
            .await
            .map_err(|e| {
                forward::release_bind(&bind);
                format!("绑定 {} 失败: {}", bind, e)
            })?;
        let conn = Arc::clone(self);
        let mut stop = self.forwards.subscribe();
        rt::spawn(async move {
            loop {
                tokio::select! {
                    changed = stop.changed() => {
                        if changed.is_err() || *stop.borrow() {
                            break;
                        }
                    }
                    accepted = listener.accept() => {
                        let Ok((mut tcp, peer)) = accepted else { break; };
                        let conn = Arc::clone(&conn);
                        rt::spawn(async move {
                            let request = match forward::read_socks5_request(&mut tcp).await {
                                Ok(request) => request,
                                Err(_) => return,
                            };
                            let opened = conn.handle().channel_open_direct_tcpip(
                                request.0.clone(),
                                u32::from(request.1),
                                peer.ip().to_string(),
                                u32::from(peer.port()),
                            ).await;
                            match opened {
                                Ok(channel) => {
                                    let _ = forward::write_socks5_reply(&mut tcp, true).await;
                                    forward::bridge_channel_tcp(channel, tcp).await;
                                }
                                Err(_) => {
                                    let _ = forward::write_socks5_reply(&mut tcp, false).await;
                                }
                            }
                        });
                    }
                }
            }
        });
        Ok(format!("动态转发（SOCKS5）{}", bind))
    }

    async fn spawn_remote_forward(&self, spec: &crate::model::PortForward) -> Result<String, String> {
        let assigned = self
            .handle
            .tcpip_forward(spec.bind_host.clone(), u32::from(spec.bind_port))
            .await
            .map_err(|e| format!("远端拒绝监听 {}:{}: {}", spec.bind_host, spec.bind_port, e))?;
        let port = if assigned == 0 { u32::from(spec.bind_port) } else { assigned };
        self.forwards.register_remote(
            port,
            RemoteTarget {
                listen_host: spec.bind_host.clone(),
                target_host: spec.target_host.clone(),
                target_port: spec.target_port,
            },
        );
        Ok(format!(
            "远端转发 {}:{} → {}:{}",
            spec.bind_host, port, spec.target_host, spec.target_port
        ))
    }
}

fn forward_owner(conn: &Conn) -> String {
    format!("{:p}", conn)
}

fn jump_to_config(hop: &JumpHop) -> SshConnectConfig {
    SshConnectConfig {
        host: hop.host.clone(),
        port: hop.port,
        username: hop.username.clone(),
        auth_type: hop.auth_type.clone(),
        password: hop.password.clone(),
        private_key: hop.private_key.clone(),
        passphrase: hop.passphrase.clone(),
        trust_host_key: hop.trust_host_key,
        cols: 80,
        rows: 24,
        jumps: Vec::new(),
        proxy: None,
        forwards: Vec::new(),
    }
}

fn annotate_host_key_error(err: String, host: &str, port: u16) -> String {
    if err.starts_with("HOST_KEY_UNKNOWN|") && err.matches('|').count() == 1 {
        return format!("{}|{}|{}", err, host, port);
    }
    if err.starts_with("HOST_KEY_MISMATCH|") && err.matches('|').count() == 2 {
        return format!("{}|{}|{}", err, host, port);
    }
    err
}

async fn handshake<R>(
    stream: R,
    config: &SshConnectConfig,
    prompter: Option<AuthPrompter>,
    forwards: StdArc<ForwardControl>,
) -> Result<(Handle<HostKeyVerifier>, HostKeyInfo), String>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let rejection: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let info: Arc<Mutex<Option<HostKeyInfo>>> = Arc::new(Mutex::new(None));
    let handler = HostKeyVerifier {
        config: config.clone(),
        rejection: Arc::clone(&rejection),
        info: Arc::clone(&info),
        forwards,
    };
    let mut handle = client::connect_stream(client_config(), stream, handler)
        .await
        .map_err(|e| {
            rejection
                .lock()
                .clone()
                .unwrap_or_else(|| format!("SSH 握手失败: {}", e))
        })?;
    let label = format!("{}@{}:{}", config.username, config.host, config.port);
    authenticate(&mut handle, config, prompter.as_ref(), &label).await?;
    let host_key = info.lock().clone().unwrap_or(HostKeyInfo {
        fingerprint: String::new(),
        status: "trusted".into(),
    });
    Ok((handle, host_key))
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
    let expanded = expand_tilde(raw);
    let passphrase = config.passphrase.as_deref().filter(|p| !p.is_empty());

    // 私钥既可能是文件路径，也可能是粘贴的密钥内容。`~` 要先展开，ssh config 导入常用这种写法。
    if Path::new(&expanded).is_file() {
        russh::keys::load_secret_key(&expanded, passphrase)
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
    prompter: Option<&AuthPrompter>,
    host_label: &str,
) -> Result<(), String> {
    if let Ok(result) = handle.authenticate_none(&config.username).await {
        if result.success() {
            return Ok(());
        }
    }

    if config.auth_type == "keyboard" {
        return keyboard_interactive(handle, config, prompter, host_label).await;
    }

    match config.auth_type.as_str() {
        "password" => {
            let result = handle
                .authenticate_password(&config.username, config.password.clone().unwrap_or_default())
                .await
                .map_err(|e| format!("密码认证失败: {}", e))?;
            if result.success() {
                return Ok(());
            }
            if needs_keyboard_followup(&result) {
                return keyboard_interactive(handle, config, prompter, host_label).await;
            }
            Err("身份认证未通过，请检查账号密码".into())
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

fn needs_keyboard_followup(result: &russh::client::AuthResult) -> bool {
    match result {
        russh::client::AuthResult::Failure {
            remaining_methods,
            partial_success,
        } => {
            *partial_success
                && remaining_methods
                    .iter()
                    .any(|method| *method == MethodKind::KeyboardInteractive)
        }
        _ => false,
    }
}

/// 服务端用 keyboard-interactive 要密码或 OTP。空提问直接回空答案，有的网关会先发一轮空包。
async fn keyboard_interactive(
    handle: &mut Handle<HostKeyVerifier>,
    config: &SshConnectConfig,
    prompter: Option<&AuthPrompter>,
    host_label: &str,
) -> Result<(), String> {
    let mut reply = handle
        .authenticate_keyboard_interactive_start(&config.username, None)
        .await
        .map_err(|e| format!("交互式认证失败: {}", e))?;
    for _ in 0..8 {
        match reply {
            client::KeyboardInteractiveAuthResponse::Success => return Ok(()),
            client::KeyboardInteractiveAuthResponse::Failure { .. } => {
                return Err("交互式认证未通过".into());
            }
            client::KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let fields: Vec<PromptField> = prompts
                    .iter()
                    .map(|prompt| PromptField {
                        prompt: prompt.prompt.clone(),
                        secret: !prompt.echo,
                    })
                    .collect();
                let answers = if fields.is_empty() {
                    Vec::new()
                } else {
                    let prompter = prompter.ok_or_else(|| {
                        "服务器要求交互式认证（如 OTP），但当前没有界面可提问".to_string()
                    })?;
                    prompter.ask(host_label, &name, &instructions, &fields).await?
                };
                reply = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(|e| format!("交互式认证失败: {}", e))?;
            }
        }
    }
    Err("交互式认证轮次过多，已停止".into())
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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHostEntry {
    pub hosts: String,
    pub marker: Option<String>,
    pub key_type: String,
    pub fingerprint: String,
}

pub fn list_known_hosts() -> Result<Vec<KnownHostEntry>, String> {
    let path = known_hosts_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取 known_hosts 失败: {}", e))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let raw = line.trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let mut fields = raw.split_whitespace();
        let first = match fields.next() {
            Some(value) => value,
            None => continue,
        };
        let (marker, hosts) = if first.starts_with('@') {
            (Some(first.to_string()), match fields.next() {
                Some(value) => value.to_string(),
                None => continue,
            })
        } else {
            (None, first.to_string())
        };
        let key_type = match fields.next() {
            Some(value) => value.to_string(),
            None => continue,
        };
        let key_b64 = match fields.next() {
            Some(value) => value,
            None => continue,
        };
        let Ok(blob) = base64::engine::general_purpose::STANDARD.decode(key_b64) else {
            continue;
        };
        out.push(KnownHostEntry {
            hosts,
            marker,
            key_type,
            fingerprint: fingerprint_sha256(&blob),
        });
    }
    Ok(out)
}

pub fn delete_known_host(hosts: &str, fingerprint: &str) -> Result<(), String> {
    let path = known_hosts_path();
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读取 known_hosts 失败: {}", e))?;
    let mut kept = Vec::new();
    let mut removed = false;
    for line in text.lines() {
        let raw = line.trim();
        let drop_line = list_known_hosts_line_matches(raw, hosts, fingerprint);
        if drop_line {
            removed = true;
            continue;
        }
        kept.push(line);
    }
    if !removed {
        return Err("没有找到对应的 known_hosts 记录".into());
    }
    let mut content = kept.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    std::fs::write(path, content).map_err(|e| format!("更新 known_hosts 失败: {}", e))
}

fn list_known_hosts_line_matches(raw: &str, hosts: &str, fingerprint: &str) -> bool {
    if raw.is_empty() || raw.starts_with('#') {
        return false;
    }
    let mut fields = raw.split_whitespace();
    let first = match fields.next() {
        Some(value) => value,
        None => return false,
    };
    let names = if first.starts_with('@') {
        match fields.next() {
            Some(value) => value,
            None => return false,
        }
    } else {
        first
    };
    if names != hosts {
        return false;
    }
    let _key_type = fields.next();
    let Some(key_b64) = fields.next() else {
        return false;
    };
    let Ok(blob) = base64::engine::general_purpose::STANDARD.decode(key_b64) else {
        return false;
    };
    fingerprint_sha256(&blob) == fingerprint
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
        // `|1|salt|mac` 留在名字里，匹配时用 HMAC-SHA1 正向比对，不需要还原主机名。
        let names: Vec<String> = names_field
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
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
    if name.starts_with("|1|") {
        return hashed_name_covers(name, config);
    }
    let token = known_host_token(config);
    if name == token {
        return true;
    }
    // 通配符对着同一条记号匹配：`*.example.com` 只覆盖 22 端口，
    // 非默认端口要写成 `[*.example.com]:2222` 或 `[*.example.com]:*`。
    if name.contains('*') || name.contains('?') {
        return wildcard_match(name, &token);
    }
    false
}

fn plain_key_line_covers(line: &str, config: &SshConnectConfig) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }
    let Some(first) = trimmed.split_whitespace().next() else {
        return false;
    };
    if first.starts_with('@') {
        return false;
    }
    name_covers_host(first, config)
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

type HmacSha1 = Hmac<Sha1>;

/// known_hosts 里用来比对的主机记号。非 22 端口必须带 `[host]:port`，
/// 和 OpenSSH 写入、哈希时用的字符串一致。
fn known_host_token(config: &SshConnectConfig) -> String {
    if config.port == 22 {
        config.host.clone()
    } else {
        format!("[{}]:{}", config.host, config.port)
    }
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .ok()
        .or_else(|| base64::engine::general_purpose::STANDARD_NO_PAD.decode(text).ok())
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> Option<[u8; 20]> {
    let mut mac = HmacSha1::new_from_slice(key).ok()?;
    mac.update(data);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 20];
    out.copy_from_slice(bytes.as_slice());
    Some(out)
}

/// `|1|salt|mac` 是 HMAC-SHA1(salt, host-token)。盐和摘要都解不出来就当对不上。
fn hashed_name_covers(token: &str, config: &SshConnectConfig) -> bool {
    let Some(rest) = token.strip_prefix("|1|") else {
        return false;
    };
    let Some((salt_b64, mac_b64)) = rest.split_once('|') else {
        return false;
    };
    let Some(salt) = b64_decode(salt_b64) else {
        return false;
    };
    let Some(expected) = b64_decode(mac_b64) else {
        return false;
    };
    let Some(digest) = hmac_sha1(&salt, known_host_token(config).as_bytes()) else {
        return false;
    };
    if expected.len() != digest.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in expected.iter().zip(digest.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

/// 写入 known_hosts 的主机名只允许字面主机。换行、逗号、通配符会变成额外信任行。
fn host_label_can_be_recorded(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 255
        && !host.starts_with('-')
        && !host.ends_with('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%'))
}

/// 受信 CA 已经对上之后，签名、critical options、类型、有效期和 principal 都必须通过。
/// 签不过或主机名不符时直接拒绝，不能再掉回「首次信任」让用户把伪造密钥钉进去。
fn authorize_host_certificate(
    cert: &Certificate,
    ca_blobs: &[Vec<u8>],
    revoked_blobs: &[Vec<u8>],
    config: &SshConnectConfig,
) -> Result<bool, String> {
    let ca_blob = key_data_blob(cert.signature_key());
    if ca_blob.is_empty() {
        return Ok(false);
    }
    if revoked_blobs.iter().any(|blob| blob == &ca_blob) {
        return Err("签发该主机证书的 CA 已在 ~/.ssh/known_hosts 中标记为 @revoked，拒绝连接".into());
    }
    if !ca_blobs.iter().any(|blob| blob == &ca_blob) {
        return Ok(false);
    }
    if cert.verify_signature().is_err() {
        return Err("主机证书签名校验失败，拒绝连接".into());
    }
    // source-address 等 critical option 不认识就必须拒绝，不能当成没写。
    if !cert.critical_options().is_empty() {
        return Err(
            "主机证书带有本客户端未执行的 critical options（例如 source-address），拒绝连接".into(),
        );
    }
    if cert.cert_type() != CertType::Host {
        return Err("远端出示的不是主机证书，拒绝连接".into());
    }
    let now = now_unix();
    // PROTOCOL.certkeys：valid_before 是开区间上界。
    if now < cert.valid_after() || now >= cert.valid_before() {
        return Err("主机证书不在有效期内，拒绝连接".into());
    }
    let principal_ok = cert.valid_principals().iter().any(|principal| {
        principal == &config.host || wildcard_match(principal, &config.host)
    });
    if !principal_ok {
        return Err("主机证书的 principal 不包含当前主机，拒绝连接".into());
    }
    Ok(true)
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
    let mut ca_blobs: Vec<Vec<u8>> = Vec::new();
    let mut revoked_blobs: Vec<Vec<u8>> = Vec::new();
    for entry in &entries {
        if !entry.names.iter().any(|name| name_covers_host(name, config)) {
            continue;
        }
        match entry.marker.as_deref() {
            Some("@revoked") => {
                if entry.key_blob == plain_blob {
                    revoked = true;
                }
                revoked_blobs.push(entry.key_blob.clone());
                continue;
            }
            Some("@cert-authority") => {
                ca_blobs.push(entry.key_blob.clone());
                continue;
            }
            // 不认识的标记不当成普通主机密钥，避免把未知指令解释成已信任。
            Some(_) => continue,
            None => {}
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

    // 裸公钥命中仍然算信任。证书只有在 CA 公钥、签名和 principal 都通过时才算。
    if !matched {
        if let Some(cert) = certificate {
            if authorize_host_certificate(cert, &ca_blobs, &revoked_blobs, config)? {
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

    let host_entry = known_host_token(config);

    if !old_fingerprints.is_empty() {
        if !config.trust_host_key {
            return Err(format!(
                "HOST_KEY_MISMATCH|{}|{}",
                fingerprint,
                old_fingerprints.join(";")
            ));
        }
        ensure_host_recordable(&config.host)?;
        // 用户已确认替换：只删这条主机的普通密钥（含能对上的哈希行），不动 CA 和吊销行。
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
    ensure_host_recordable(&config.host)?;
    append_known_host(&kh_path, &host_entry, &plain, &plain_blob)?;
    Ok(HostKeyInfo {
        fingerprint,
        status: "trusted".into(),
    })
}

fn ensure_host_recordable(host: &str) -> Result<(), String> {
    if host_label_can_be_recorded(host) {
        Ok(())
    } else {
        Err("主机名含有不能写入 known_hosts 的字符，已拒绝保存".into())
    }
}

fn append_known_host(
    kh_path: &Path,
    host_entry: &str,
    plain: &russh::keys::PublicKey,
    plain_blob: &[u8],
) -> Result<(), String> {
    if host_entry.chars().any(|c| c.is_whitespace() || c.is_control() || c == ',') {
        return Err("主机名含有不能写入 known_hosts 的字符，已拒绝保存".into());
    }
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

/// 用户确认更换密钥时，只删除这条主机的普通公钥行。
/// `@cert-authority` / `@revoked` 即使通配符盖住当前主机也保留。
/// 能用 HMAC 对上的 `|1|` 普通行一并删掉，否则旧密钥会继续被信任。
fn rewrite_known_hosts_without(
    kh_path: &Path,
    config: &SshConnectConfig,
    original: &str,
) -> Result<(), String> {
    let mut kept: Vec<&str> = Vec::new();
    for line in original.lines() {
        if plain_key_line_covers(line, config) {
            continue;
        }
        kept.push(line);
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
            jumps: Vec::new(),
            proxy: None,
            forwards: Vec::new(),
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

    #[test]
    fn wildcard_without_port_does_not_cover_other_ports() {
        let config = SshConnectConfig {
            host: "api.example.com".into(),
            port: 2222,
            ..base_config()
        };
        assert!(!name_covers_host("*.example.com", &config));
        assert!(name_covers_host("[*.example.com]:*", &config));
        assert!(name_covers_host("[*.example.com]:2222", &config));
    }

    #[test]
    fn hashed_known_host_matches_only_the_same_token() {
        let salt = b"snail-ssh-salt-20b!";
        let config = base_config();
        let mac = hmac_sha1(salt, b"example.com").unwrap();
        let token = format!(
            "|1|{}|{}",
            base64::engine::general_purpose::STANDARD.encode(salt),
            base64::engine::general_purpose::STANDARD.encode(mac)
        );
        assert!(name_covers_host(&token, &config));
        let other = SshConnectConfig {
            host: "other.example".into(),
            ..config
        };
        assert!(!name_covers_host(&token, &other));
        let ported = SshConnectConfig {
            port: 2222,
            ..base_config()
        };
        assert!(!name_covers_host(&token, &ported));
    }

    #[test]
    fn rewrite_keeps_ca_and_revoked_lines() {
        let dir = std::env::temp_dir().join(format!("snail-kh-keep-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("known_hosts");
        let salt = b"snail-ssh-salt-20b!";
        let mac = hmac_sha1(salt, b"example.com").unwrap();
        let hashed = format!(
            "|1|{}|{} ssh-ed25519 AAAAHASHED\n",
            base64::engine::general_purpose::STANDARD.encode(salt),
            base64::engine::general_purpose::STANDARD.encode(mac)
        );
        let original = format!(
            "example.com ssh-ed25519 AAAAOLD\n{hashed}@cert-authority *.example.com ssh-ed25519 AAAACA\n@revoked example.com ssh-ed25519 AAAAREVOKED\nother.host ssh-ed25519 AAAAKEEP\n"
        );
        rewrite_known_hosts_without(&path, &base_config(), &original).unwrap();
        let kept = std::fs::read_to_string(&path).unwrap();
        assert!(!kept.contains("AAAAOLD"));
        assert!(!kept.contains("AAAAHASHED"));
        assert!(kept.contains("AAAACA"));
        assert!(kept.contains("AAAAREVOKED"));
        assert!(kept.contains("AAAAKEEP"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn host_label_rejects_known_hosts_metacharacters() {
        assert!(host_label_can_be_recorded("example.com"));
        assert!(host_label_can_be_recorded("fe80::1"));
        assert!(!host_label_can_be_recorded("example.com\n*"));
        assert!(!host_label_can_be_recorded("example.com,*.bank.com"));
        assert!(!host_label_can_be_recorded("*.example.com"));
    }

    #[test]
    fn host_certificate_requires_a_valid_signature() {
        let mut rng = rand::rng();
        let ca = russh::keys::PrivateKey::random(&mut rng, russh::keys::Algorithm::Ed25519).unwrap();
        let subject =
            russh::keys::PrivateKey::random(&mut rng, russh::keys::Algorithm::Ed25519).unwrap();
        let now = now_unix();
        let mut builder = russh::keys::ssh_key::certificate::Builder::new_with_random_nonce(
            &mut rng,
            subject.public_key(),
            now.saturating_sub(30),
            now.saturating_add(3600),
        )
        .unwrap();
        builder.cert_type(CertType::Host).unwrap();
        builder.valid_principal("example.com").unwrap();
        let cert = builder.sign(&ca).unwrap();
        let ca_blob = key_data_blob(cert.signature_key());
        let config = base_config();

        assert_eq!(
            authorize_host_certificate(&cert, &[ca_blob.clone()], &[], &config).unwrap(),
            true
        );
        assert_eq!(
            authorize_host_certificate(&cert, &[], &[], &config).unwrap(),
            false
        );
        assert!(authorize_host_certificate(&cert, &[ca_blob.clone()], &[ca_blob.clone()], &config).is_err());

        let mut raw = cert.to_bytes().unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let forged = Certificate::from_bytes(raw.as_slice()).unwrap();
        assert!(authorize_host_certificate(&forged, &[ca_blob], &[], &config).is_err());

        let mut restricted = russh::keys::ssh_key::certificate::Builder::new_with_random_nonce(
            &mut rng,
            subject.public_key(),
            now.saturating_sub(30),
            now.saturating_add(3600),
        )
        .unwrap();
        restricted.cert_type(CertType::Host).unwrap();
        restricted.valid_principal("example.com").unwrap();
        restricted
            .critical_option("source-address", "10.0.0.0/8")
            .unwrap();
        let restricted = restricted.sign(&ca).unwrap();
        let restricted_ca = key_data_blob(restricted.signature_key());
        assert!(authorize_host_certificate(&restricted, &[restricted_ca], &[], &config).is_err());

        let mut other = russh::keys::ssh_key::certificate::Builder::new_with_random_nonce(
            &mut rng,
            subject.public_key(),
            now.saturating_sub(30),
            now.saturating_add(3600),
        )
        .unwrap();
        other.cert_type(CertType::Host).unwrap();
        other.valid_principal("other.example").unwrap();
        let other = other.sign(&ca).unwrap();
        let other_ca = key_data_blob(other.signature_key());
        assert!(authorize_host_certificate(&other, &[other_ca], &[], &config).is_err());
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
            jumps: Vec::new(),
            proxy: None,
            forwards: Vec::new(),
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
