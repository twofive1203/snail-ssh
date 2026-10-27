export type AuthType = 'password' | 'key' | 'agent' | 'keyboard'

export interface ProxyConfig {
  kind: 'socks5' | 'http' | string
  host: string
  port: number
  username?: string
  password?: string
}

export interface JumpHop {
  host: string
  port: number
  username: string
  authType: string
  password?: string | null
  privateKey?: string | null
  passphrase?: string | null
  trustHostKey?: boolean
}

export interface PortForward {
  id: string
  kind: 'local' | 'remote' | 'dynamic' | string
  bindHost: string
  bindPort: number
  targetHost: string
  targetPort: number
  enabled: boolean
}

export interface HostItem {
  id: string
  name: string
  host: string
  port: number
  username: string
  authType: AuthType
  password?: string
  privateKey?: string
  passphrase?: string
  group: string
  tags: string[]
  /** 按连接顺序经过的跳板主机 id */
  jumpHostIds?: string[]
  proxy?: ProxyConfig | null
  forwards?: PortForward[]
  os?: string
  osName?: string
  status?: 'online' | 'offline' | 'unknown'
  ping?: string
  cpu?: number
  mem?: number
  disk?: number
  updatedAt?: string
}

export interface TerminalTab {
  id: string
  /**
   * 前端稳定的标签标识：重连会换 `id`（后端会话 id），但 UI key 与 xterm 实例
   * 靠 `uiId` 保持不变，这样重连后仍能看到断开前的回显历史。
   */
  uiId: string
  hostId: string
  title: string
  sub: string
  connected: boolean
  /** 断线原因（仅 `connected === false` 时有意义） */
  closeReason?: string
  /** 正在重建连接（避免重复点击重连） */
  reconnecting?: boolean
  cwd?: string
  /** 是否让左侧文件工作区跟随本标签页的终端目录（Tab 维度，默认关闭） */
  autoFollowTerminal?: boolean
}

/** 命令历史条目（与后端 history.rs 的 HistoryEntry 对应） */
export interface CommandHistoryEntry {
  cmd: string
  count: number
  lastUsedAt: number
}

/** OS 级文件拖放事件（Tauri webview 提供，坐标为 CSS 像素） */
export interface FileDragDropEvent {
  type: 'enter' | 'over' | 'drop' | 'leave'
  paths: string[]
  x: number
  y: number
}

/** 右键菜单项：divider 为 true 时渲染成分隔线，其余字段忽略 */
export interface ContextMenuItem {
  key?: string
  label?: string
  /** lucide 图标组件 */
  icon?: unknown
  /** 右侧灰色提示文案，如快捷键或 IP */
  hint?: string
  disabled?: boolean
  danger?: boolean
  divider?: boolean
}

export interface SftpFileItem {
  name: string
  isDir: boolean
  size: string
  sizeBytes: number
  permissions: string
  owner: string
  group: string
  modifiedTime: string
  editable: boolean
  isHidden: boolean
}

/** 传输任务状态：cancelled 与 failed 区分，暂停可继续 */
export type TransferStatus = 'running' | 'paused' | 'completed' | 'failed' | 'cancelled'

export interface SftpProgressPayload {
  taskId: string
  progress: number
  speed: string
  /** scanning 仅在目录上传枚举阶段出现（进度百分比尚不可知） */
  status: TransferStatus | 'scanning'
  transferred: number
  total: number
}

/** 重试所需上下文：原样重放即可从已传字节续传 */
export interface TransferContext {
  sessionId: string
  localPath: string
  /** 上传时为目标父目录；下载时为远端完整路径 */
  remotePath: string
  upload: boolean
}

export interface TransferTask {
  id: string
  name: string
  type: 'upload' | 'download'
  progress: number
  speed: string
  size: string
  status: TransferStatus
  context?: TransferContext
}

/** 后端真实连通性测试结果 */
export interface TrustedEndpoint {
  host: string
  port: number
}

export interface SshKeyInfo {
  id: string
  name: string
  algorithm: string
  fingerprint: string
  publicKey: string
  path: string
  createdAt: string
}

export interface KnownHostEntry {
  hosts: string
  marker?: string | null
  keyType: string
  fingerprint: string
}

export interface ImportPreview {
  hosts: HostItem[]
  warnings: string[]
  sourcePath: string
}

export interface TunnelStatus {
  id: string
  label: string
  notes: string[]
}

export interface AuthPromptField {
  prompt: string
  secret: boolean
}

export interface AuthPromptEvent {
  requestId: string
  hostLabel: string
  name: string
  instructions: string
  prompts: AuthPromptField[]
}

export interface ConnectionTestResult {
  latencyMs: number
  hostKeyFingerprint: string
  hostKeyStatus: 'matched' | 'trusted' | string
}

/** 远端主机实时指标探测结果 */
export interface HostProbeResult {
  os?: string
  osName?: string
  cpu?: number
  mem?: number
  disk?: number
  pingMs?: number
}

/** 远端磁盘使用率指标 */
export interface RemoteDiskUsage {
  mountPoint: string
  usePercent: number
  total: string
  used: string
}

/** 远端服务器实时系统监控指标 (MobaXterm 底部监控条) */
export interface RemoteSystemMetrics {
  hostname: string
  osName?: string
  uptime: string
  users: string[]
  cpu: number
  memUsedBytes: number
  memTotalBytes: number
  memUsagePercent: number
  netUploadSpeed: number
  netDownloadSpeed: number
  disks: RemoteDiskUsage[]
}

/** 批量传输时引用的远端文件 */
export interface RemoteFileRef {
  file: SftpFileItem
  remotePath: string
}

export interface FileTreeNode {
  path: string
  name: string
  isDir: boolean
  size: string
  sizeBytes: number
  permissions: string
  owner: string
  group: string
  modifiedTime: string
  editable: boolean
  isHidden: boolean
  children?: FileTreeNode[]
  expanded?: boolean
  loading?: boolean
  loaded?: boolean
}

export type WorkspaceMode = 'split' | 'sftp'

export interface HostSession {
  id: string
  hostId: string
  host: HostItem
  title: string
  terminalTabs: TerminalTab[]
  activeTerminalIndex: number
  sftpSessionId?: string
  currentPath: string
  sidebarCollapsed: boolean
  sidebarWidth: number
  viewMode: 'tree' | 'list'
  remoteMonitorOpen?: boolean
  /** 当前工作区模式：'split' 为终端分栏协同，'sftp' 为独立全屏 SFTP 文件管理 */
  workspaceMode?: WorkspaceMode
}

export interface ToastMessage {
  id: string
  text: string
  type: 'info' | 'success' | 'warning' | 'error'
}

// ==========================================
// 应用设置（~/.snail-ssh/settings.json）
// ==========================================

/** 终端外观设置：改动即时应用到所有已打开终端 */
export interface TerminalSettings {
  fontFamily: string
  fontSize: number
  lineHeight: number
  cursorStyle: 'block' | 'underline' | 'bar'
  cursorBlink: boolean
  /** 内置配色预设 id，见 composables/useAppSettings.ts 的 TERMINAL_THEMES */
  themeId: string
}

/** 常用指令片段：终端工具栏与命令面板共用 */
export interface SnippetItem {
  id: string
  label: string
  /** 发送到终端的原始文本，末尾是否带换行由内容决定 */
  cmd: string
}

/** SFTP 常用目录书签 */
export interface BookmarkItem {
  id: string
  label: string
  path: string
}

/** 主机分组：颜色用于标签页/工作区环境标识，danger 触发危险命令确认 */
export interface HostGroupItem {
  id: string
  label: string
  /** 调色板 key，见 GROUP_COLORS */
  color: string
  /** 生产等高风险环境：危险命令发送前二次确认 */
  danger?: boolean
}

export interface AppSettings {
  terminal: TerminalSettings
  snippets: SnippetItem[]
  bookmarks: BookmarkItem[]
  groups: HostGroupItem[]
  /** 底部独立命令输入框（对标 Xshell Compose Bar / FinalShell 命令输入框） */
  commandBarEnabled: boolean
  /** 历史补全：开启后终端底部固定留一行，避免候选显隐把终端高度挤来挤去 */
  historySuggestEnabled: boolean
  /** 新建会话时默认展开底部远程监控条；关闭后仍可从文件树工具栏按会话手动开启 */
  remoteMonitorEnabled: boolean
  /** 拖选 / 双击选词 / 三击选行后自动写入剪贴板，不弹提示 */
  selectToCopy: boolean
  /** 终端内右键直接粘贴；Shift 或 Ctrl 再右键仍打开菜单 */
  pasteOnRightClick: boolean
}
