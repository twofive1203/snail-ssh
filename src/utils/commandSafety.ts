/**
 * 危险命令识别（用于生产环境的发送前二次确认）。
 *
 * 只做启发式匹配，不追求覆盖所有写法：目的是拦住「手滑把 rm -rf / 发到生产机」
 * 这类高频事故，而不是做一个完整的 shell 解析器。误报的代价是一次确认弹窗，
 * 漏报的代价是不可逆的数据丢失，因此宁可略偏保守。
 */

interface DangerRule {
  /** 规则名，展示在确认弹窗里 */
  label: string
  pattern: RegExp
}

const DANGER_RULES: DangerRule[] = [
  {
    label: '删除根目录 / 家目录 / 系统关键目录',
    // 只拦“删到系统层面”的目标，/tmp/x、./build 这类正常工作目录不报警
    pattern: /\brm\s+(-[^\s]+\s+)*(\/|~\/?|\/\*|\/etc|\/usr|\/var|\/home|\/boot|\/root)(\s|$)/
  },
  { label: '递归强制删除（rm -rf）', pattern: /\brm\s+-[a-zA-Z]*(r[a-zA-Z]*f|f[a-zA-Z]*r)[a-zA-Z]*\b/ },
  { label: '格式化文件系统', pattern: /\bmkfs(\.[a-z0-9]+)?\b|\bdd\s+.*\bof=\/dev\// },
  { label: '清空磁盘或分区', pattern: /\b(shred|wipefs)\b|>\s*\/dev\/(sd|nvme|vd)/ },
  { label: '强制关机 / 重启', pattern: /\b(shutdown|reboot|halt|poweroff)\b/ },
  { label: '停止关键服务', pattern: /\bsystemctl\s+(stop|disable|mask)\s+(ssh|sshd|network|networking|firewalld|ufw)\b/ },
  { label: '清空防火墙规则', pattern: /\biptables\s+-F\b|\bufw\s+disable\b|\bfirewall-cmd\s+--panic-on\b/ },
  { label: '直接操作块设备', pattern: /\b(fdisk|parted|sgdisk)\s+\/dev\// },
  { label: '删除全部容器或镜像', pattern: /\bdocker\s+(system|volume|image|container)\s+prune\b|\bdocker\s+rmi\s+.*\$\(/ },
  { label: '覆盖关键系统文件', pattern: />\s*\/(etc|boot|usr|lib)\// },
  { label: '杀死全部进程', pattern: /\bkillall5\b|\bkill\s+-9\s+-1\b|\bpkill\s+-9\b/ },
  { label: '卸载挂载点', pattern: /\bumount\s+(-[a-zA-Z]+\s+)*\/(\s|$)/ }
]

export interface DangerMatch {
  label: string
  /** 命中的原始片段，便于用户确认自己到底写了什么 */
  snippet: string
}

/**
 * 返回命中的危险规则；安全命令返回 null。
 * 多行草稿按行拆分，任意一行命中即视为危险（整段发送同样会执行）。
 */
export const detectDangerousCommand = (text: string): DangerMatch | null => {
  if (!text.trim()) return null
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    for (const rule of DANGER_RULES) {
      const matched = trimmed.match(rule.pattern)
      if (matched) return { label: rule.label, snippet: trimmed.slice(0, 160) }
    }
  }
  return null
}
