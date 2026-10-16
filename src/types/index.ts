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

export interface TransferTask {
  id: string
  name: string
  type: 'upload' | 'download'
  progress: number
  speed: string
  size: string
  status: 'running' | 'completed' | 'failed'
}

export interface ToastMessage {
  id: string
  text: string
  type: 'info' | 'success' | 'warning' | 'error'
}
