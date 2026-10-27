export interface HostKeyChallenge {
  kind: 'unknown' | 'mismatch'
  fingerprint: string
  oldFingerprints: string[]
  host?: string
  port?: number
}

/** 解析后端主机密钥确认错误。带 host/port 时只信任这一跳，不能把整条链路一起放行。 */
export function parseHostKeyError(message: string): HostKeyChallenge | null {
  if (message.startsWith('HOST_KEY_MISMATCH|')) {
    const parts = message.split('|')
    const port = Number(parts[4])
    return {
      kind: 'mismatch',
      fingerprint: parts[1] || '未知',
      oldFingerprints: (parts[2] || '').split(';').filter(Boolean),
      host: parts[3] || undefined,
      port: Number.isFinite(port) && parts[4] ? port : undefined,
    }
  }
  if (message.startsWith('HOST_KEY_UNKNOWN|')) {
    const parts = message.split('|')
    const port = Number(parts[3])
    return {
      kind: 'unknown',
      fingerprint: parts[1] || '未知',
      oldFingerprints: [],
      host: parts[2] || undefined,
      port: Number.isFinite(port) && parts[3] ? port : undefined,
    }
  }
  return null
}
