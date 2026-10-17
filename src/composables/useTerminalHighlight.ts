import type { Terminal, IDecoration, IDisposable, IBufferLine, ILink, ILinkProvider } from '@xterm/xterm'

/**
 * 终端关键字高亮。
 *
 * 实现约束（务必保持）：
 * - 绝不改写 chunk 文本，只在写入解析完成后用 marker + decoration 做后置扫描，
 *   否则 ANSI 序列 / 多字节字符会在 chunk 边界被切断导致乱码。
 * - 只扫「新增行」，每 tick 最多扫 MAX_SCAN_LINES_PER_TICK 行，用 requestAnimationFrame 节流，
 *   保证 `tail -f` 大日志时不卡 UI。
 * - decoration 必须挂在 marker（相对位置）上，不能用绝对行号，否则滚动后错位。
 * - 默认只给命中的「词」上色（x + width），整行铺底需要规则显式声明 wholeLine。
 * - 有背景色时，前景色只在底层单元格 isFgDefault() 为真时才设置；
 *   否则会把 ls / grep --color / systemctl 等程序的 ANSI 着色抹平
 *   （xterm 的 decoration 前景色会直接覆盖 cell 的 $fg，见 CellColorResolver）。
 * - 列号必须按 cell 宽度换算：CJK / emoji 占 2 列，直接用 JS 字符串下标会横向偏移。
 */

export interface TerminalHighlightRule {
  key: string
  pattern: RegExp
  foregroundColor?: string
  backgroundColor?: string
  /** 整行着色。默认 false，只给命中词上色，避免整屏色块 */
  wholeLine?: boolean
}

/** 内置规则集，后续做成设置项时直接复用 */
export const DEFAULT_HIGHLIGHT_RULES: TerminalHighlightRule[] = [
  {
    key: 'error',
    pattern: /\b(ERROR|FATAL|FAIL(?:ED|URE)?|PANIC|Exception|Traceback)\b/i,
    foregroundColor: '#fecaca',
    backgroundColor: '#7f1d1d',
    // 错误整行铺底：日志里一眼定位，有 ANSI 着色时只铺背景不改字色
    wholeLine: true
  },
  {
    key: 'warn',
    pattern: /\b(WARN(?:ING)?|DEPRECAT(?:ED|ION))\b/i,
    foregroundColor: '#fde68a',
    backgroundColor: '#451a03'
  },
  {
    key: 'success',
    pattern: /\b(SUCCESS|PASSED|Done)\b/i,
    foregroundColor: '#bbf7d0',
    backgroundColor: '#052e16'
  }
]

/** IP / file:line 可点击（点击复制），不做整行着色 */
const LINK_PATTERNS: RegExp[] = [
  /\b\d{1,3}(?:\.\d{1,3}){3}\b/g,
  /[A-Za-z0-9_./~-]+\.[A-Za-z0-9]+:\d+(?::\d+)?/g
]

export const HIGHLIGHT_STORAGE_KEY = 'snail_ssh_highlight_enabled'

const MAX_SCAN_LINES_PER_TICK = 200
const MAX_DECORATIONS = 4000
const MAX_HITS_PER_LINE = 24

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

interface CompiledRule {
  rule: TerminalHighlightRule
  scan: RegExp
  priority: number
}

interface TextHit {
  start: number
  end: number
  rule: CompiledRule
}

const compileRules = (rules: TerminalHighlightRule[]): CompiledRule[] =>
  rules.map((rule, priority) => {
    // 复制成带 g 的正则用于逐行取所有匹配位置，不复用原对象避免 lastIndex 串扰
    const flags = rule.pattern.flags.includes('g') ? rule.pattern.flags : `${rule.pattern.flags}g`
    return { rule, priority, scan: new RegExp(rule.pattern.source, flags) }
  })

/** 收集一行内所有规则的命中，按「起点 + 规则优先级」排序并丢弃重叠段 */
const collectHits = (text: string, rules: CompiledRule[]): TextHit[] => {
  const hits: TextHit[] = []
  for (const rule of rules) {
    rule.scan.lastIndex = 0
    let m: RegExpExecArray | null
    while ((m = rule.scan.exec(text)) !== null) {
      // 零长度匹配会死循环
      if (!m[0]) {
        rule.scan.lastIndex++
        continue
      }
      hits.push({ start: m.index, end: m.index + m[0].length, rule })
      // 防止病态正则在一行内产生海量匹配拖垮 UI
      if (hits.length >= MAX_HITS_PER_LINE * 4) break
    }
  }
  if (hits.length === 0) return hits

  // 整行规则优先生效且独占：一旦命中就只返回这一条，
  // 否则后注册的词级高亮会与整行底色叠在一起，观感混乱
  const wholeLineHit = hits
    .filter((hit) => hit.rule.rule.wholeLine === true)
    .sort((a, b) => a.rule.priority - b.rule.priority)[0]
  if (wholeLineHit) return [wholeLineHit]

  // 规则在数组中的顺序即优先级：同一起点保留高优先级，重叠的丢弃
  hits.sort((a, b) => a.start - b.start || a.rule.priority - b.rule.priority)
  const accepted: TextHit[] = []
  let lastEnd = 0
  for (const hit of hits) {
    if (hit.start < lastEnd) continue
    accepted.push(hit)
    lastEnd = hit.end
    if (accepted.length >= MAX_HITS_PER_LINE) break
  }
  return accepted
}

/**
 * 建立「字符串下标 → cell 列号」映射，修正 CJK / emoji 宽字符造成的横向偏移。
 * 必须与 translateToString 的遍历方式一致：宽字符占 2 列，其右半格（width = 0）不产生字符。
 */
const buildColumnMap = (line: IBufferLine, textLength: number): number[] => {
  const columns = new Array<number>(textLength + 1)
  let col = 0
  let offset = 0

  while (col < line.length && offset < textLength) {
    const cell = line.getCell(col)
    if (!cell) break

    const width = cell.getWidth()
    if (width === 0) {
      // 宽字符右半格，不承载字符
      col++
      continue
    }

    // 空白单元格 getChars() 返回 ''，但 translateToString 里算作一个空格
    const chars = cell.getChars() || ' '
    for (let k = 0; k < chars.length && offset + k < textLength; k++) {
      columns[offset + k] = col
    }
    offset += chars.length
    col += width
  }

  // 末端列 = 最后一个字符之后的列，作为命中词的右边界
  columns[textLength] = col
  return columns
}

/** 命中区间内是否有单元格带显式前景色（palette / RGB） */
const hasExplicitForeground = (line: IBufferLine, from: number, to: number): boolean => {
  const end = Math.max(from + 1, to)
  for (let x = from; x < end; x++) {
    const cell = line.getCell(x)
    // width = 0 是宽字符右半格，前景色与左半格一致，跳过
    if (!cell || cell.getWidth() === 0) continue
    if (!cell.isFgDefault()) return true
  }
  return false
}

export function useTerminalHighlight(term: Terminal, options: UseTerminalHighlightOptions = {}): TerminalHighlight {
  const compiledRules = compileRules(options.rules ?? DEFAULT_HIGHLIGHT_RULES)
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

  /** 为一行上的命中项挂 decoration；x / width 均为修正后的 cell 列号 */
  const addHitsForLine = (line: IBufferLine, lineIndex: number, cursorLine: number, hits: TextHit[], text: string) => {
    // 整行规则不需要列号映射，避免纯错误行多做一次分配
    const columns = hits.some((hit) => hit.rule.rule.wholeLine !== true) ? buildColumnMap(line, text.length) : null
    for (const hit of hits) {
      const wholeLine = hit.rule.rule.wholeLine === true
      const from = wholeLine ? 0 : (columns?.[hit.start] ?? 0)
      const to = wholeLine ? term.cols : (columns?.[hit.end] ?? from + 1)
      const width = Math.max(1, to - from)

      const marker = term.registerMarker(lineIndex - cursorLine)
      if (!marker) continue

      const background = hit.rule.rule.backgroundColor
      const foreground = hit.rule.rule.foregroundColor
      // 有背景时背景已足够醒目，不再动前景色，保住程序原有 ANSI 着色；
      // 没有背景时前景色是唯一可见手段，只能直接上色。
      const canPaintForeground =
        foreground !== undefined && (background === undefined || !hasExplicitForeground(line, from, from + width))

      const dec = canPaintForeground
        ? term.registerDecoration({ marker, x: from, width, backgroundColor: background, foregroundColor: foreground, layer: 'top' })
        : term.registerDecoration({ marker, x: from, width, backgroundColor: background, layer: 'top' })

      if (!dec) {
        marker.dispose()
        continue
      }

      decorations.push(dec)
      marker.onDispose(() => {
        dec.dispose()
        const idx = decorations.indexOf(dec)
        if (idx >= 0) decorations.splice(idx, 1)
      })
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
        if (line && text) {
          const hits = collectHits(text, compiledRules)
          if (hits.length > 0) {
            addHitsForLine(line, i, cursorLine, hits, text)
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
      if (!line || !text) {
        callback(undefined)
        return
      }

      // 与关键字高亮共用列号映射，中文 / emoji 行里链接范围不会偏移
      const columns = buildColumnMap(line, text.length)
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
              start: { x: columns[start] ?? start, y: lineIndex },
              end: { x: columns[start + value.length] ?? start + value.length, y: lineIndex }
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
