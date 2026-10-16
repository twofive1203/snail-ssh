import type { Terminal, IDecoration, IDisposable, ILink, ILinkProvider } from '@xterm/xterm'

/**
 * 终端关键字高亮。
 *
 * 实现约束（务必保持）：
 * - 绝不改写 chunk 文本，只在写入解析完成后用 marker + decoration 做后置扫描，
 *   否则 ANSI 序列 / 多字节字符会在 chunk 边界被切断导致乱码。
 * - 只扫「新增行」，每 tick 最多扫 MAX_SCAN_LINES_PER_TICK 行，用 requestAnimationFrame 节流，
 *   保证 `tail -f` 大日志时不卡 UI。
 * - decoration 必须挂在 marker（相对位置）上，不能用绝对行号，否则滚动后错位。
 */

export interface TerminalHighlightRule {
  key: string
  pattern: RegExp
  foregroundColor?: string
  backgroundColor?: string
}

/** 内置规则集，后续做成设置项时直接复用 */
export const DEFAULT_HIGHLIGHT_RULES: TerminalHighlightRule[] = [
  {
    key: 'error',
    pattern: /\b(ERROR|FATAL|FAIL(?:ED|URE)?|PANIC|Exception|Traceback)\b/i,
    foregroundColor: '#fecaca',
    backgroundColor: '#7f1d1d'
  },
  {
    key: 'warn',
    pattern: /\b(WARN(?:ING)?|DEPRECAT(?:ED|ION))\b/i,
    foregroundColor: '#fbbf24'
  },
  {
    key: 'success',
    pattern: /\b(SUCCESS|PASSED|Done)\b/i,
    foregroundColor: '#4ade80'
  }
]

/** IP / file:line 可点击（点击复制），不做整行着色 */
const LINK_PATTERNS: RegExp[] = [
  /\b\d{1,3}(?:\.\d{1,3}){3}\b/g,
  /[A-Za-z0-9_./~-]+\.[A-Za-z0-9]+:\d+(?::\d+)?/g
]

export const HIGHLIGHT_STORAGE_KEY = 'snail_ssh_highlight_enabled'

const MAX_SCAN_LINES_PER_TICK = 200
const MAX_DECORATIONS = 2000

export interface UseTerminalHighlightOptions {
  onToast?: (msg: string, type?: 'info' | 'success' | 'warning' | 'error') => void
  rules?: TerminalHighlightRule[]
}

export interface TerminalHighlight {
  setEnabled(enabled: boolean): void
  isEnabled(): boolean
  /** 开启时从「当前光标行往前回溯这么多行」开始扫，避免重放整个 scrollback */
  dispose(): void
}

const matchRule = (text: string, rules: TerminalHighlightRule[]): TerminalHighlightRule | null => {
  for (const rule of rules) {
    // 每次重置 lastIndex，避免全局正则的状态残留
    rule.pattern.lastIndex = 0
    if (rule.pattern.test(text)) return rule
  }
  return null
}

export function useTerminalHighlight(term: Terminal, options: UseTerminalHighlightOptions = {}): TerminalHighlight {
  const rules = options.rules ?? DEFAULT_HIGHLIGHT_RULES
  const onToast = options.onToast

  let enabled = false
  let disposed = false
  let lastScannedAbsoluteLine = 0
  let rafId: number | null = null
  const decorations: IDecoration[] = []
  let linkProviderDisposable: IDisposable | null = null
  let writeParsedDisposable: IDisposable | null = null
  let resizeDisposable: IDisposable | null = null

  /** 尺寸变化后行列重排，decoration 宽度会失效，清空后按最近 200 行重建 */
  const resetForResize = () => {
    if (!enabled) return
    clearDecorations()
    lastScannedAbsoluteLine = Math.max(0, currentAbsoluteLine() - MAX_SCAN_LINES_PER_TICK)
    scheduleScan()
  }

  const currentAbsoluteLine = () => {
    const buffer = term.buffer.active
    return buffer.baseY + buffer.cursorY
  }

  const clearDecorations = () => {
    // 先整体移出，否则 marker.onDispose 里的 splice 会干扰遍历
    const list = decorations.splice(0, decorations.length)
    for (const dec of list) {
      try {
        dec.dispose()
      } catch {
        // 缓冲区回收时可能已经 dispose，忽略
      }
    }
  }

  const scan = () => {
    rafId = null
    if (disposed || !enabled) return
    const buffer = term.buffer.active
    // alt buffer（vim/less/top）不做高亮，避免整屏花掉
    if (buffer.type !== 'normal') {
      lastScannedAbsoluteLine = currentAbsoluteLine()
      return
    }

    const cursorLine = buffer.baseY + buffer.cursorY
    let i = lastScannedAbsoluteLine
    let scanned = 0

    while (i <= cursorLine && scanned < MAX_SCAN_LINES_PER_TICK) {
      // 跳过光标所在行（当前输入行），否则用户打字时会闪烁
      if (i < cursorLine) {
        const line = buffer.getLine(i)
        const text = line ? line.translateToString(true) : ''
        if (text) {
          const rule = matchRule(text, rules)
          if (rule) {
            // marker 使用相对光标行偏移，滚动/回收时自动跟随
            const marker = term.registerMarker(i - cursorLine)
            if (marker) {
              const dec = term.registerDecoration({
                marker,
                // width 默认只有 1 格，不指定的话整行只会高亮首字符
                width: term.cols,
                backgroundColor: rule.backgroundColor,
                foregroundColor: rule.foregroundColor,
                layer: 'top'
              })
              if (dec) {
                decorations.push(dec)
                marker.onDispose(() => {
                  dec.dispose()
                  const idx = decorations.indexOf(dec)
                  if (idx >= 0) decorations.splice(idx, 1)
                })
              } else {
                marker.dispose()
              }
            }
          }
        }
      }
      i++
      scanned++
    }

    lastScannedAbsoluteLine = i

    // 数量上限：超出时丢弃最早的载体，防止长会话内存膨胀
    while (decorations.length > MAX_DECORATIONS) {
      const dec = decorations.shift()
      try {
        dec?.dispose()
      } catch {
        // ignore
      }
    }

    // 本 tick 没扫完，下一帧继续
    if (i <= cursorLine) scheduleScan()
  }

  const scheduleScan = () => {
    if (disposed || !enabled || rafId !== null) return
    rafId = requestAnimationFrame(scan)
  }

  const buildLinkProvider = (): ILinkProvider => ({
    provideLinks(bufferLineNumber, callback) {
      if (disposed || !enabled) {
        callback(undefined)
        return
      }
      const lineIndex = bufferLineNumber - 1
      const line = term.buffer.active.getLine(lineIndex)
      const text = line ? line.translateToString(true) : ''
      if (!text) {
        callback(undefined)
        return
      }

      const found: ILink[] = []
      for (const pattern of LINK_PATTERNS) {
        pattern.lastIndex = 0
        let m: RegExpExecArray | null
        while ((m = pattern.exec(text)) !== null) {
          const value = m[0]
          if (!value) {
            pattern.lastIndex++
            continue
          }
          const start = m.index
          found.push({
            range: {
              start: { x: start, y: lineIndex },
              end: { x: start + value.length, y: lineIndex }
            },
            text: value,
            activate: () => {
              navigator.clipboard.writeText(value)
                .then(() => onToast?.(`已复制: ${value}`, 'success'))
                .catch(() => onToast?.(`复制失败，请手动选中: ${value}`, 'warning'))
            }
          })
        }
      }

      if (found.length === 0) {
        callback(undefined)
        return
      }
      // 按起点排序并去掉互相重叠的范围（例如 file:line 中恰好含 IP）
      found.sort((a, b) => a.range.start.x - b.range.start.x)
      const result: ILink[] = []
      let lastEnd = -1
      for (const link of found) {
        if (link.range.start.x < lastEnd) continue
        result.push(link)
        lastEnd = link.range.end.x
      }
      callback(result)
    }
  })

  // onWriteParsed 在 5.5 中可用；缺失时退化为包装 write（升级 xterm 后的兜底）
  if (typeof (term as any).onWriteParsed === 'function') {
    writeParsedDisposable = term.onWriteParsed(() => scheduleScan())
  } else {
    const originalWrite = term.write.bind(term)
    ;(term as any).write = (data: string | Uint8Array, callback?: () => void) => {
      const result = originalWrite(data, callback)
      scheduleScan()
      return result
    }
  }

  resizeDisposable = term.onResize(() => resetForResize())

  const setEnabled = (next: boolean) => {
    if (disposed) return
    if (next === enabled) return
    enabled = next

    if (enabled) {
      // 从最近 200 行开始扫，既不重放全部历史，又能覆盖当前屏幕
      const cursorLine = currentAbsoluteLine()
      lastScannedAbsoluteLine = Math.max(0, cursorLine - MAX_SCAN_LINES_PER_TICK)
      linkProviderDisposable = term.registerLinkProvider(buildLinkProvider())
      scheduleScan()
    } else {
      if (rafId !== null) {
        cancelAnimationFrame(rafId)
        rafId = null
      }
      linkProviderDisposable?.dispose()
      linkProviderDisposable = null
      clearDecorations()
    }
  }

  return {
    setEnabled,
    isEnabled: () => enabled,
    dispose() {
      disposed = true
      enabled = false
      if (rafId !== null) {
        cancelAnimationFrame(rafId)
        rafId = null
      }
      linkProviderDisposable?.dispose()
      linkProviderDisposable = null
      writeParsedDisposable?.dispose()
      writeParsedDisposable = null
      resizeDisposable?.dispose()
      resizeDisposable = null
      clearDecorations()
    }
  }
}
