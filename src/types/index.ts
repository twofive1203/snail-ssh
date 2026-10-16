export type AuthType = 'password' | 'key' | 'agent'

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
  hostId: string
  title: string
  sub: string
  connected: boolean
  cwd?: string
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

export interface SftpProgressPayload {
  taskId: string
  progress: number
  speed: string
  status: 'running' | 'completed' | 'failed'
  transferred: number
  total: number
}

export interface TransferTask {
  id: string
  name: string
  type: 'upload' | 'download'
  progress: number
  speed: string
  size: string
  status: 'running' | 'completed' | 'failed'
}

/** 后端真实连通性测试结果 */
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
  autoFollowTerminal?: boolean
}

export interface ToastMessage {
  id: string
  text: string
  type: 'info' | 'success' | 'warning' | 'error'
}
