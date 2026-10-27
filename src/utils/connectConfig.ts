import type { HostItem, JumpHop, TrustedEndpoint } from '@/types'

export function isTrusted(trusted: TrustedEndpoint[], host: string, port: number): boolean {
  return trusted.some(item => item.host === host && item.port === port)
}

/** 把主机上的跳板 id 展开成带凭证的跳。顺序是先连最外层跳板，再连下一跳。 */
export function expandJumps(host: HostItem, hosts: HostItem[], trusted: TrustedEndpoint[]): JumpHop[] {
  const seen = new Set<string>([host.id])
  return walk(host, hosts, trusted, seen)
}

function walk(host: HostItem, hosts: HostItem[], trusted: TrustedEndpoint[], seen: Set<string>): JumpHop[] {
  const out: JumpHop[] = []
  for (const id of host.jumpHostIds || []) {
    if (seen.has(id)) {
      throw new Error('跳板机形成了循环，请检查主机的跳板设置')
    }
    const hop = hosts.find(item => item.id === id)
    if (!hop) {
      throw new Error('跳板机不存在或已被删除')
    }
    seen.add(id)
    out.push(...walk(hop, hosts, trusted, seen))
    out.push({
      host: hop.host,
      port: hop.port,
      username: hop.username,
      authType: hop.authType,
      password: hop.password || null,
      privateKey: hop.privateKey || null,
      passphrase: hop.passphrase || null,
      trustHostKey: isTrusted(trusted, hop.host, hop.port),
    })
  }
  return out
}

export function buildSshConfig(
  host: HostItem,
  hosts: HostItem[],
  cols: number,
  rows: number,
  trusted: TrustedEndpoint[]
) {
  const proxy = host.proxy && host.proxy.host?.trim() ? host.proxy : null
  return {
    host: host.host,
    port: host.port,
    username: host.username,
    auth_type: host.authType,
    password: host.password || null,
    private_key: host.privateKey || null,
    passphrase: host.passphrase || null,
    trust_host_key: isTrusted(trusted, host.host, host.port),
    cols,
    rows,
    jumps: expandJumps(host, hosts, trusted),
    proxy,
    forwards: host.forwards || [],
  }
}
