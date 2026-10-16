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

export interface ToastMessage {
  id: string
  text: string
  type: 'info' | 'success' | 'warning' | 'error'
}
