import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import type {
  ConnectionTestResult,
  HostItem,
  HostProbeResult,
  RemoteSystemMetrics,
  SftpFileItem,
  SftpProgressPayload
} from '@/types'

// 检测是否处于 Tauri 环境
export const isTauri = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

// 模拟初始数据 (浏览器预览/未连真实 SSH 降级使用)
const MOCK_STORAGE_KEY = 'snail_ssh_hosts'
const defaultMockHosts: HostItem[] = [
  {
    id: 'prod-k8s-master-01',
    name: 'prod-k8s-master-01',
    host: '47.98.112.24',
    port: 22,
    username: 'root',
    authType: 'key',
    group: 'production',
    tags: ['k8s', 'master', 'aliyun-hz'],
    os: 'ubuntu',
    osName: 'Ubuntu 22.04 LTS',
    status: 'online',
    ping: '24ms',
    cpu: 28,
    mem: 64,
    disk: 42,
    updatedAt: '2分钟前'
  },
  {
    id: 'prod-k8s-node-02',
    name: 'prod-k8s-node-02',
    host: '47.98.112.25',
    port: 22,
    username: 'root',
    authType: 'key',
    group: 'production',
    tags: ['k8s', 'worker', 'docker'],
    os: 'ubuntu',
    osName: 'Ubuntu 22.04 LTS',
    status: 'online',
    ping: '26ms',
    cpu: 56,
    mem: 81,
    disk: 65,
    updatedAt: '1分钟前'
  },
  {
    id: 'aliyun-hk-edge',
    name: 'aliyun-hk-edge',
    host: '43.154.201.89',
    port: 2222,
    username: 'deploy',
    authType: 'password',
    group: 'staging',
    tags: ['nginx', 'gateway', 'hk-node'],
    os: 'debian',
    osName: 'Debian 12 (Bookworm)',
    status: 'online',
    ping: '48ms',
    cpu: 14,
    mem: 38,
    disk: 29,
    updatedAt: '刚刚'
  },
  {
    id: 'staging-api-service',
    name: 'staging-api-service',
    host: '118.24.89.14',
    port: 22,
    username: 'admin',
    authType: 'key',
    group: 'staging',
    tags: ['golang', 'redis', 'staging'],
    os: 'centos',
    osName: 'Rocky Linux 9.2',
    status: 'online',
    ping: '32ms',
    cpu: 19,
    mem: 52,
    disk: 38,
    updatedAt: '5分钟前'
  },
  {
    id: 'vps-us-west-bench',
    name: 'vps-us-west-bench',
    host: '142.171.8.60',
    port: 22,
    username: 'root',
    authType: 'key',
    group: 'cloud',
    tags: ['vps', 'bench', 'us-west'],
    os: 'alpine',
    osName: 'Alpine Linux 3.19',
    status: 'online',
    ping: '145ms',
    cpu: 8,
    mem: 24,
    disk: 18,
    updatedAt: '12分钟前'
  },
  {
    id: 'home-lab-macmini',
    name: 'home-lab-macmini',
    host: '192.168.31.80',
    port: 22,
    username: 'chong',
    authType: 'agent',
    group: 'cloud',
    tags: ['apple-silicon', 'lan', 'm2-pro'],
    os: 'apple',
    osName: 'macOS Sonoma 14.3',
    status: 'online',
    ping: '2ms',
    cpu: 11,
    mem: 45,
    disk: 52,
    updatedAt: '刚刚'
  }
]

// ==========================================
// 主机资产存储 API
// ==========================================
export async function getHostsApi(): Promise<HostItem[]> {
  if (isTauri()) {
    try {
      return await invoke<HostItem[]>('get_hosts')
    } catch (err) {
      console.warn('Tauri get_hosts failed, fallback to mock:', err)
    }
  }
  const raw = localStorage.getItem(MOCK_STORAGE_KEY)
  if (!raw) {
    localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(defaultMockHosts))
    return defaultMockHosts
  }
  try {
    return JSON.parse(raw)
  } catch {
    return defaultMockHosts
  }
}

export async function saveHostApi(host: HostItem): Promise<void> {
  if (isTauri()) {
    try {
      await invoke('save_host', { host })
      return
    } catch (err) {
      console.warn('Tauri save_host failed:', err)
    }
  }
  const list = await getHostsApi()
  const idx = list.findIndex(h => h.id === host.id)
  if (idx >= 0) {
    list[idx] = host
  } else {
    list.unshift(host)
  }
  localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(list))
}

export async function deleteHostApi(id: string): Promise<void> {
  if (isTauri()) {
    try {
      await invoke('delete_host', { id })
      return
    } catch (err) {
      console.warn('Tauri delete_host failed:', err)
    }
  }
  const list = await getHostsApi()
  const filtered = list.filter(h => h.id !== id)
  localStorage.setItem(MOCK_STORAGE_KEY, JSON.stringify(filtered))
}

// ==========================================
// SSH 终端真实连接与通道 API
// ==========================================
export async function createSshSessionApi(
  host: HostItem,
  cols: number,
  rows: number,
  trustHostKey = false
): Promise<{ sessionId: string }> {
  if (isTauri()) {
    return await invoke<{ sessionId: string }>('create_ssh_session', {
      config: {
        host: host.host,
        port: host.port,
        username: host.username,
        auth_type: host.authType,
        password: host.password || null,
        private_key: host.privateKey || null,
        passphrase: host.passphrase || null,
        trust_host_key: trustHostKey,
        cols,
        rows,
      }
    })
  }
  // 浏览器预览也需保证 sessionId 唯一，否则同一主机多开时 tab 会互相覆盖
  return { sessionId: `mock-session-${Date.now()}-${Math.floor(Math.random() * 100000)}` }
}

// 真实连通性测试：TCP + 握手 + 主机密钥校验 + 认证，返回真实耗时（仅 Tauri 环境可用）
export async function testSshConnectionApi(
  host: HostItem,
  trustHostKey = false
): Promise<ConnectionTestResult> {
  if (!isTauri()) {
    throw new Error('浏览器预览模式无法测试真实连接，请使用 Tauri 桌面客户端')
  }
  return await invoke<ConnectionTestResult>('test_ssh_connection', {
    config: {
      host: host.host,
      port: host.port,
      username: host.username,
      auth_type: host.authType,
      password: host.password || null,
      private_key: host.privateKey || null,
      passphrase: host.passphrase || null,
      trust_host_key: trustHostKey,
      cols: 80,
      rows: 24,
    }
  })
}

// 采集远端真实指标（发行版 / CPU / 内存 / 磁盘 / 延迟）
export async function probeHostInfoApi(sessionId: string): Promise<HostProbeResult> {
  if (!isTauri()) {
    return {}
  }
  return await invoke<HostProbeResult>('probe_host_info', { sessionId })
}

export async function writeSshSessionApi(sessionId: string, data: string): Promise<void> {
  if (isTauri()) {
    await invoke('write_ssh_session', { sessionId, data })
  }
}

export async function resizeSshSessionApi(sessionId: string, cols: number, rows: number): Promise<void> {
  if (isTauri()) {
    await invoke('resize_ssh_session', { sessionId, cols, rows })
  }
}

export async function closeSshSessionApi(sessionId: string): Promise<void> {
  if (isTauri()) {
    await invoke('close_ssh_session', { sessionId })
  }
}

// 模拟终端路径字典 (非 Tauri 降级使用)
const mockTerminalCwdMap = new Map<string, string>()

export function setMockTerminalCwd(sessionId: string, cwd: string): void {
  mockTerminalCwdMap.set(sessionId, cwd)
}

export async function getSshSessionCwdApi(sessionId: string): Promise<string> {
  if (isTauri()) {
    return await invoke<string>('get_ssh_session_cwd', { sessionId })
  }
  return mockTerminalCwdMap.get(sessionId) || '/etc/nginx'
}

let mockCpuTick = 1
export async function getRemoteMetricsApi(sessionId: string): Promise<RemoteSystemMetrics> {
  if (isTauri()) {
    return await invoke<RemoteSystemMetrics>('get_remote_metrics', { sessionId })
  }
  mockCpuTick = mockCpuTick + Math.floor(Math.random() * 5 - 2)
  if (mockCpuTick < 1) mockCpuTick = 1
  if (mockCpuTick > 25) mockCpuTick = 12
  return {
    hostname: 'quickea144',
    osName: 'Red Hat Enterprise Linux 9.2',
    uptime: '111 days',
    users: ['root', 'root', 'root', 'root', 'root'],
    cpu: mockCpuTick,
    memUsedBytes: 15354572800,
    memTotalBytes: 16825729024,
    memUsagePercent: 91,
    netUploadSpeed: 52428 + Math.floor(Math.random() * 10240),
    netDownloadSpeed: 20971 + Math.floor(Math.random() * 5120),
    disks: [
      { mountPoint: '/', usePercent: 9, total: '100.0 GB', used: '9.0 GB' },
      { mountPoint: '/boot', usePercent: 58, total: '1.0 GB', used: '580 MB' },
      { mountPoint: '/data', usePercent: 34, total: '500.0 GB', used: '170.0 GB' }
    ]
  }
}

export async function onSshStdout(sessionId: string, callback: (data: string) => void): Promise<UnlistenFn> {
  if (isTauri()) {
    return await listen<string>(`ssh-stdout-${sessionId}`, (event) => {
      callback(event.payload)
    })
  }
  return () => {}
}

// 远端主动关闭 Shell / 连接中断通知
export async function onSshClosed(sessionId: string, callback: (reason: string) => void): Promise<UnlistenFn> {
  if (isTauri()) {
    return await listen<string>(`ssh-closed-${sessionId}`, (event) => {
      callback(event.payload)
    })
  }
  return () => {}
}

// ==========================================
// 本地文件选择 / 保存路径选择 (Tauri Dialog)
// ==========================================
export async function pickLocalFile(): Promise<string | null> {
  if (!isTauri()) return null
  const selected = await openDialog({ multiple: false, directory: false })
  return typeof selected === 'string' ? selected : null
}

export async function pickSavePath(defaultFileName: string): Promise<string | null> {
  if (!isTauri()) return null
  const selected = await saveDialog({ defaultPath: defaultFileName })
  return selected ?? null
}

// 批量下载时选择目标目录
export async function pickDirectory(): Promise<string | null> {
  if (!isTauri()) return null
  const selected = await openDialog({ multiple: false, directory: true })
  return typeof selected === 'string' ? selected : null
}

// ==========================================
// SFTP 远程文件系统 API
// ==========================================
export async function sftpHomeDirApi(sessionId: string): Promise<string> {
  if (isTauri()) {
    return await invoke<string>('sftp_home_dir', { sessionId })
  }
  return '/etc/nginx'
}

export async function sftpReadDirApi(sessionId: string, path: string): Promise<SftpFileItem[]> {
  if (isTauri()) {
    try {
      return await invoke<SftpFileItem[]>('sftp_read_dir', { sessionId, path })
    } catch (err) {
      console.error('sftp_read_dir error:', err)
      throw err
    }
  }

  // 模拟目录内容
  return [
    { name: '..', isDir: true, size: '-', sizeBytes: 0, permissions: 'drwxr-xr-x (755)', owner: 'root', group: 'root', modifiedTime: '2025-02-18 10:24', editable: false, isHidden: false },
    { name: 'conf.d', isDir: true, size: '4.0 KB', sizeBytes: 4096, permissions: 'drwxr-xr-x (755)', owner: 'root', group: 'root', modifiedTime: '2025-02-19 14:32', editable: false, isHidden: false },
    { name: 'sites-available', isDir: true, size: '4.0 KB', sizeBytes: 4096, permissions: 'drwxr-xr-x (755)', owner: 'root', group: 'root', modifiedTime: '2025-02-10 09:15', editable: false, isHidden: false },
    { name: 'sites-enabled', isDir: true, size: '4.0 KB', sizeBytes: 4096, permissions: 'lrwxrwxrwx (777)', owner: 'root', group: 'root', modifiedTime: '2025-02-10 09:16', editable: false, isHidden: false },
    { name: 'nginx.conf', isDir: false, size: '2.8 KB', sizeBytes: 2842, permissions: '-rw-r--r-- (644)', owner: 'root', group: 'root', modifiedTime: '2025-02-20 16:45', editable: true, isHidden: false },
    { name: 'mime.types', isDir: false, size: '5.2 KB', sizeBytes: 5320, permissions: '-rw-r--r-- (644)', owner: 'root', group: 'root', modifiedTime: '2024-11-20 03:12', editable: true, isHidden: false },
    { name: 'proxy_params', isDir: false, size: '1.1 KB', sizeBytes: 1120, permissions: '-rw-r--r-- (644)', owner: 'root', group: 'root', modifiedTime: '2025-01-14 11:20', editable: true, isHidden: false },
    { name: 'app-upstream.yaml', isDir: false, size: '890 B', sizeBytes: 890, permissions: '-rw-r--r-- (644)', owner: 'root', group: 'root', modifiedTime: '2025-02-19 22:10', editable: true, isHidden: false },
    { name: 'ssl-bundle.crt', isDir: false, size: '3.6 KB', sizeBytes: 3680, permissions: '-rw------- (600)', owner: 'root', group: 'root', modifiedTime: '2025-01-01 00:00', editable: false, isHidden: false },
    { name: 'backup-20250218.tar.gz', isDir: false, size: '42.5 MB', sizeBytes: 44564480, permissions: '-rw-r--r-- (644)', owner: 'root', group: 'root', modifiedTime: '2025-02-18 03:00', editable: false, isHidden: false },
    { name: '.env.local', isDir: false, size: '420 B', sizeBytes: 420, permissions: '-rw------- (600)', owner: 'root', group: 'root', modifiedTime: '2025-02-12 18:22', editable: true, isHidden: true },
  ]
}

export async function sftpReadFileApi(sessionId: string, path: string): Promise<string> {
  if (isTauri()) {
    return await invoke<string>('sftp_read_file', { sessionId, path })
  }
  return `# ==============================================
# SnailSSH 远程实时配置文件编辑器
# Path: ${path}
# ==============================================

user www-data;
worker_processes auto;
pid /run/nginx.pid;
include /etc/nginx/modules-enabled/*.conf;

events {
    worker_connections 2048;
    multi_accept on;
    use epoll;
}

http {
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;

    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    access_log /var/log/nginx/access.log;
    error_log /var/log/nginx/error.log warn;

    include /etc/nginx/conf.d/*.conf;
    include /etc/nginx/sites-enabled/*;
}
`
}

export async function sftpWriteFileApi(sessionId: string, path: string, content: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_write_file', { sessionId, path, content })
  }
}

export async function sftpMkdirApi(sessionId: string, path: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_mkdir', { sessionId, path })
  }
}

export async function sftpRemoveFileApi(sessionId: string, path: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_remove_file', { sessionId, path })
  }
}

// 重命名 / 移动：overwrite=false 时目标已存在会报错，由调用方决定是否重试覆盖
export async function sftpRenameApi(sessionId: string, from: string, to: string, overwrite = false): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_rename', { sessionId, from, to, overwrite })
  }
}

// 修改权限位（八进制数值，如 0o755）
export async function sftpChmodApi(sessionId: string, path: string, mode: number): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_chmod', { sessionId, path, mode })
  }
}

// 修改属主 / 属组（数字 id，未传的项不变）
export async function sftpChownApi(sessionId: string, path: string, uid?: number, gid?: number): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_chown', { sessionId, path, uid, gid })
  }
}

// 新建空白文件（已存在则报错）
export async function sftpTouchApi(sessionId: string, path: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_touch', { sessionId, path })
  }
}

// 单条目属性（供属性弹窗与冲突检测复用）
export async function sftpStatApi(sessionId: string, path: string): Promise<SftpFileItem | null> {
  if (isTauri()) {
    return await invoke<SftpFileItem>('sftp_stat', { sessionId, path })
  }
  return null
}

export async function sftpUploadFileApi(sessionId: string, localPath: string, remotePath: string, taskId: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_upload_file', { sessionId, localPath, remotePath, taskId })
  }
}

export async function sftpDownloadFileApi(sessionId: string, remotePath: string, localPath: string, taskId: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_download_file', { sessionId, remotePath, localPath, taskId })
  }
}

// 目录递归下载：保留远端目录结构，进度按目录内文件总字节数统计
export async function sftpDownloadDirApi(sessionId: string, remotePath: string, localPath: string, taskId: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_download_dir', { sessionId, remotePath, localPath, taskId })
  }
}

// 监听单次传输任务的实时进度（bytes/进度/速度）
export async function onSftpProgress(taskId: string, callback: (payload: SftpProgressPayload) => void): Promise<UnlistenFn> {
  if (isTauri()) {
    return await listen<SftpProgressPayload>(`sftp-progress-${taskId}`, (event) => {
      callback(event.payload)
    })
  }
  return () => {}
}
