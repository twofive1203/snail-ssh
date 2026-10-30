import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type {
  AppSettings,
  AuthPromptEvent,
  CommandHistoryEntry,
  ConnectionTestResult,
  FileDragDropEvent,
  HostItem,
  HostProbeResult,
  ImportPreview,
  KnownHostEntry,
  RemoteSystemMetrics,
  SftpFileItem,
  SftpProgressPayload,
  SshKeyInfo,
  StorageHealth,
  TrustedEndpoint,
  TunnelStatus
} from '@/types'
import { buildSshConfig } from '@/utils/connectConfig'

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

/**
 * 存储健康状态（钥匙串可用性 / hosts.json 是否损坏并回退备份）。
 * 浏览器预览返回 null；后端未注册该命令时也返回 null——调用方只能按「未知」展示，
 * 否则旧版本后端会让每个界面都弹出一次错误提示。
 */
export async function getStorageHealthApi(): Promise<StorageHealth | null> {
  if (!isTauri()) return null
  try {
    return await invoke<StorageHealth>('get_storage_health')
  } catch (err) {
    console.warn('Tauri get_storage_health failed:', err)
    return null
  }
}

// ==========================================
// 应用设置（~/.snail-ssh/settings.json）
// ==========================================
/** 读取设置；桌面端文件不存在时返回 null，由前端使用默认值 */
export async function getSettingsApi(): Promise<Partial<AppSettings> | null> {
  if (!isTauri()) return null
  return await invoke<Partial<AppSettings> | null>('get_settings')
}

/** 整块覆盖写入设置（后端只做 JSON 安全落盘，结构由前端约束） */
export async function saveSettingsApi(settings: AppSettings): Promise<void> {
  if (!isTauri()) return
  await invoke('save_settings', { settings })
}

// ==========================================
// SSH 终端真实连接与通道 API
// ==========================================
export async function createSshSessionApi(
  host: HostItem,
  cols: number,
  rows: number,
  trusted: TrustedEndpoint[] = [],
  hosts: HostItem[] = []
): Promise<{ sessionId: string }> {
  if (isTauri()) {
    return await invoke<{ sessionId: string }>('create_ssh_session', {
      config: buildSshConfig(host, hosts.length ? hosts : [host], cols, rows, trusted)
    })
  }
  // 浏览器预览也需保证 sessionId 唯一，否则同一主机多开时 tab 会互相覆盖
  return { sessionId: `mock-session-${Date.now()}-${Math.floor(Math.random() * 100000)}` }
}

// 真实连通性测试：TCP + 握手 + 主机密钥校验 + 认证，返回真实耗时（仅 Tauri 环境可用）
export async function testSshConnectionApi(
  host: HostItem,
  trusted: TrustedEndpoint[] = [],
  hosts: HostItem[] = []
): Promise<ConnectionTestResult> {
  if (!isTauri()) {
    throw new Error('浏览器预览模式无法测试真实连接，请使用 Tauri 桌面客户端')
  }
  return await invoke<ConnectionTestResult>('test_ssh_connection', {
    config: buildSshConfig(host, hosts.length ? hosts : [host], 80, 24, trusted)
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

export interface LocalSessionInfo {
  sessionId: string
  label: string
  kind: string
  program: string
}

export interface CreateLocalSessionInput {
  program?: string | null
  args?: string[]
  cwd?: string | null
  cols: number
  rows: number
}

/** 非 Tauri 环境直接拒绝，调用方不应在预览里 invoke。 */
export async function createLocalSessionApi(input: CreateLocalSessionInput): Promise<LocalSessionInfo> {
  if (!isTauri()) {
    throw new Error('预览模式不会启动本机 PowerShell')
  }
  return await invoke<LocalSessionInfo>('create_local_session', {
    request: {
      program: input.program || null,
      args: input.args ?? [],
      cwd: input.cwd || null,
      cols: input.cols,
      rows: input.rows
    }
  })
}

export async function writeLocalSessionApi(sessionId: string, data: string): Promise<void> {
  if (!isTauri()) return
  await invoke('write_local_session', { sessionId, data })
}

export async function resizeLocalSessionApi(sessionId: string, cols: number, rows: number): Promise<void> {
  if (!isTauri()) return
  await invoke('resize_local_session', { sessionId, cols, rows })
}

export async function closeLocalSessionApi(sessionId: string): Promise<void> {
  if (!isTauri()) return
  await invoke('close_local_session', { sessionId })
}

export async function takeLocalSessionBannerApi(sessionId: string): Promise<string> {
  if (!isTauri()) return ''
  return await invoke<string>('take_local_session_banner', { sessionId })
}

export async function onLocalStdout(sessionId: string, callback: (data: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  return await listen<string>(`local-stdout-${sessionId}`, (event) => {
    callback(event.payload)
  })
}

export async function onLocalClosed(sessionId: string, callback: (reason: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  return await listen<string>(`local-closed-${sessionId}`, (event) => {
    callback(event.payload)
  })
}

export async function setLocalSessionLoggingApi(sessionId: string, enabled: boolean, label: string): Promise<string | null> {
  if (!isTauri()) return null
  return await invoke<string | null>('set_local_session_logging', { sessionId, enabled, label })
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

// 最后一次下发的采样目标。采样目标是后端全局状态，组件卸载收尾前要先看一眼
// 是否已有别的会话接手，避免自己把新目标抹掉。
let currentMetricsTarget: string | null = null
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

/**
 * 告诉后端当前该为哪个会话采样指标（null = 停止采样）。
 * 采样后由后端主动推送 ssh-metrics-<sessionId>，前端不必再轮询；
 * 该命令可能随后端改造尚未注册，失败只影响指标刷新，不能打断界面。
 */
export async function setMetricsTargetApi(sessionId: string | null): Promise<void> {
  currentMetricsTarget = sessionId
  if (!isTauri()) return
  try {
    await invoke('set_metrics_target', { sessionId })
  } catch (err) {
    console.warn('Tauri set_metrics_target failed:', err)
  }
}

/** 当前前端认为的采样目标（仅用于卸载时判断是否该由自己清空） */
export function getMetricsTargetApi(): string | null {
  return currentMetricsTarget
}

/** 订阅后端推送的远端指标；浏览器预览没有事件，由调用方用 getRemoteMetricsApi 兜底 */
export async function onSshMetrics(
  sessionId: string,
  callback: (metrics: RemoteSystemMetrics) => void
): Promise<UnlistenFn> {
  if (isTauri()) {
    return await listen<RemoteSystemMetrics>(`ssh-metrics-${sessionId}`, (event) => {
      callback(event.payload)
    })
  }
  return () => {}
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
// 命令历史 (命令补全 + 联想)
// ==========================================
const HISTORY_MOCK_KEY = 'snail_ssh_history'

/** 与后端 history.rs 保持一致的敏感命令过滤（仅浏览器预览降级使用） */
function isSensitiveCommand(cmd: string): boolean {
  const lower = cmd.toLowerCase()
  if (lower.includes('password') || lower.includes('passwd') || lower.includes('token=') || lower.includes('--token')) {
    return true
  }
  const tokens = lower.split(/\s+/)
  const looksLikeSecret = (v: string) => v.length > 0 && !/^\d+$/.test(v)
  return tokens.some((token, i) => {
    if (token.startsWith('--') || !token.startsWith('-p')) return false
    const value = token.slice(2)
    return value ? looksLikeSecret(value) : looksLikeSecret(tokens[i + 1] ?? '')
  })
}

function shouldRecordCommand(cmd: string): boolean {
  return cmd.trim().length > 0 && cmd.length <= 4096 && !cmd.startsWith(' ') && !isSensitiveCommand(cmd)
}

function readMockHistoryMap(): Record<string, CommandHistoryEntry[]> {
  try {
    const raw = localStorage.getItem(HISTORY_MOCK_KEY)
    return raw ? JSON.parse(raw) || {} : {}
  } catch {
    return {}
  }
}

function writeMockHistoryMap(map: Record<string, CommandHistoryEntry[]>): void {
  try {
    localStorage.setItem(HISTORY_MOCK_KEY, JSON.stringify(map))
  } catch {
    // localStorage 不可用时仅本次内存生效
  }
}

function sortHistory(list: CommandHistoryEntry[]): CommandHistoryEntry[] {
  return [...list].sort((a, b) => b.count - a.count || b.lastUsedAt - a.lastUsedAt)
}

export async function getCommandHistoryApi(hostKey: string): Promise<CommandHistoryEntry[]> {
  if (isTauri()) {
    return await invoke<CommandHistoryEntry[]>('get_command_history', { hostKey })
  }
  return sortHistory(readMockHistoryMap()[hostKey] || [])
}

export async function pushCommandHistoryApi(hostKey: string, cmd: string): Promise<CommandHistoryEntry[]> {
  if (isTauri()) {
    return await invoke<CommandHistoryEntry[]>('push_command_history', { hostKey, cmd })
  }
  const map = readMockHistoryMap()
  const list = map[hostKey] || (map[hostKey] = [])
  if (shouldRecordCommand(cmd)) {
    const trimmed = cmd.trim()
    const existing = list.find(e => e.cmd === trimmed)
    if (existing) {
      existing.count += 1
      existing.lastUsedAt = Date.now()
    } else {
      list.push({ cmd: trimmed, count: 1, lastUsedAt: Date.now() })
    }
  }
  writeMockHistoryMap(map)
  return sortHistory(list)
}

/** 批量导入远端 ~/.bash_history：仅插入不存在的项，不累加频次 */
export async function importCommandHistoryApi(hostKey: string, cmds: string[]): Promise<void> {
  if (isTauri()) {
    await invoke('import_command_history', { hostKey, cmds })
    return
  }
  const map = readMockHistoryMap()
  const list = map[hostKey] || (map[hostKey] = [])
  const now = Date.now()
  for (const raw of cmds) {
    if (!shouldRecordCommand(raw)) continue
    const cmd = raw.trim()
    if (!list.some(e => e.cmd === cmd)) list.push({ cmd, count: 1, lastUsedAt: now })
  }
  writeMockHistoryMap(map)
}

export async function clearCommandHistoryApi(hostKey?: string): Promise<void> {
  if (isTauri()) {
    await invoke('clear_command_history', { hostKey: hostKey ?? null })
    return
  }
  if (hostKey) {
    const map = readMockHistoryMap()
    delete map[hostKey]
    writeMockHistoryMap(map)
  } else {
    writeMockHistoryMap({})
  }
}

/** 取走建连时扣下的 shell 首屏输出（Last login / MOTD / 首个提示符；只返回一次） */
export async function takeSshSessionBannerApi(sessionId: string): Promise<string> {
  if (!isTauri()) return ''
  return await invoke<string>('take_ssh_session_banner', { sessionId })
}

/** 在会话连接上执行一次性命令（远端历史拉取等；浏览器预览返回空串） */
export async function execSshCommandApi(sessionId: string, command: string): Promise<string> {
  if (!isTauri()) return ''
  return await invoke<string>('exec_ssh_command', { sessionId, command })
}

// ==========================================
// 会话日志
// ==========================================
/** 开/关会话日志；开启或关闭均返回日志文件路径（后端会话不存在时抛错） */
export async function setSessionLoggingApi(sessionId: string, enabled: boolean, hostLabel: string): Promise<string | null> {
  if (!isTauri()) return null
  return await invoke<string | null>('set_session_logging', { sessionId, enabled, hostLabel })
}

export async function getSessionLogPathApi(sessionId: string): Promise<string | null> {
  if (!isTauri()) return null
  return await invoke<string | null>('get_session_log_path', { sessionId })
}

/** 日志根目录 `~/.snail-ssh/logs`（不存在时后端会创建） */
export async function getLogDirApi(): Promise<string | null> {
  if (!isTauri()) return null
  return await invoke<string>('get_log_dir')
}

/** 在系统文件管理器中打开路径（仅限应用数据目录内） */
export async function revealInFileManagerApi(path: string): Promise<void> {
  if (!isTauri()) return
  await invoke('reveal_in_file_manager', { path })
}

// ==========================================
// 本地文件选择 / 保存路径选择 (Tauri Dialog)
// ==========================================
export async function pickLocalFile(): Promise<string | null> {
  if (!isTauri()) return null
  const selected = await openDialog({ multiple: false, directory: false })
  return typeof selected === 'string' ? selected : null
}

/** 多选本地文件（拖拽上传之外的批量入口） */
export async function pickLocalFiles(): Promise<string[]> {
  if (!isTauri()) return []
  const selected = await openDialog({ multiple: true, directory: false })
  if (Array.isArray(selected)) return selected
  return typeof selected === 'string' ? [selected] : []
}

// ==========================================
// OS 级文件拖放（拖拽上传）
// ==========================================
/** 监听 webview 级文件拖放；坐标由物理像素换算为 CSS 像素，便于 elementFromPoint 命中 */
export async function onFileDragDrop(handler: (e: FileDragDropEvent) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  const webview = getCurrentWebview()
  return await webview.onDragDropEvent((event) => {
    const payload = event.payload
    if (payload.type === 'leave') {
      handler({ type: 'leave', paths: [], x: 0, y: 0 })
      return
    }
    const ratio = window.devicePixelRatio || 1
    handler({
      type: payload.type,
      paths: payload.type === 'over' ? [] : payload.paths,
      x: payload.position.x / ratio,
      y: payload.position.y / ratio
    })
  })
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

/** 统一上传入口：文件/目录都由后端按元数据分派，`remoteDir` 为目标父目录 */
export async function sftpUploadPathApi(sessionId: string, localPath: string, remoteDir: string, taskId: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_upload_path', { sessionId, localPath, remoteDir, taskId })
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

/** 统一下载入口：文件/目录由后端按远端类型分派 */
export async function sftpDownloadPathApi(sessionId: string, remotePath: string, localPath: string, taskId: string): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_download_path', { sessionId, remotePath, localPath, taskId })
  }
}

/** 传输控制：pause / resume / cancel。任务不存在时后端返回明确错误 */
export async function sftpTransferControlApi(taskId: string, action: 'pause' | 'resume' | 'cancel'): Promise<void> {
  if (isTauri()) {
    await invoke('sftp_transfer_control', { taskId, action })
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

export async function onSshAuthPrompt(callback: (payload: AuthPromptEvent) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  return await listen<AuthPromptEvent>('ssh-auth-prompt', (event) => callback(event.payload))
}

export async function onForwardStatus(callback: (payload: { sessionId?: string; notes: string[] }) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {}
  return await listen<{ sessionId?: string; notes: string[] }>('ssh-forward-status', (event) => callback(event.payload))
}

export async function submitSshAuthPromptApi(requestId: string, responses: string[] | null): Promise<void> {
  if (!isTauri()) return
  await invoke('submit_ssh_auth_prompt', { requestId, responses })
}

export async function previewSshConfigApi(path?: string): Promise<ImportPreview> {
  if (!isTauri()) throw new Error('浏览器预览无法读取本机 ssh config')
  return await invoke<ImportPreview>('preview_ssh_config', { path: path || null })
}

export async function exportHostsSshConfigApi(path: string): Promise<void> {
  if (!isTauri()) throw new Error('浏览器预览无法导出文件')
  await invoke('export_hosts_ssh_config', { path })
}

export async function listSshKeysApi(): Promise<SshKeyInfo[]> {
  if (!isTauri()) return []
  return await invoke<SshKeyInfo[]>('list_ssh_keys')
}

export async function generateSshKeyApi(name: string, algorithm: string, passphrase?: string): Promise<SshKeyInfo & { publicKey: string }> {
  if (!isTauri()) throw new Error('浏览器预览无法生成密钥')
  return await invoke('generate_ssh_key', { name, algorithm, passphrase: passphrase || null })
}

export async function importSshKeyApi(name: string, material: string, passphrase?: string): Promise<SshKeyInfo> {
  if (!isTauri()) throw new Error('浏览器预览无法导入密钥')
  return await invoke<SshKeyInfo>('import_ssh_key', { name, material, passphrase: passphrase || null })
}

export async function deleteSshKeyApi(id: string): Promise<void> {
  if (!isTauri()) return
  await invoke('delete_ssh_key', { id })
}

export async function listKnownHostsApi(): Promise<KnownHostEntry[]> {
  if (!isTauri()) return []
  return await invoke<KnownHostEntry[]>('list_known_hosts')
}

export async function deleteKnownHostApi(hosts: string, fingerprint: string): Promise<void> {
  if (!isTauri()) return
  await invoke('delete_known_host', { hosts, fingerprint })
}

export async function startTunnelApi(host: HostItem, hosts: HostItem[], trusted: TrustedEndpoint[] = []): Promise<TunnelStatus> {
  if (!isTauri()) throw new Error('浏览器预览无法启动隧道')
  return await invoke<TunnelStatus>('start_tunnel', {
    config: buildSshConfig(host, hosts, 80, 24, trusted)
  })
}

export async function stopTunnelApi(id: string): Promise<void> {
  if (!isTauri()) return
  await invoke('stop_tunnel', { id })
}

export async function listTunnelsApi(): Promise<TunnelStatus[]> {
  if (!isTauri()) return []
  return await invoke<TunnelStatus[]>('list_tunnels')
}
