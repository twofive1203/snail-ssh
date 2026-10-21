import type { Terminal, IDecoration, IDisposable, IBufferLine, ILink, ILinkProvider, IMarker } from '@xterm/xterm'

/**
 * 终端语法高亮（Syntax Highlighting）渲染引擎。
 *
 * 实现约束与设计要点：
 * - 绝不改写 PTY 字节流，只在 xterm 解析落盘后通过 marker + decoration 做后置着色，
 *   从根源杜绝 ANSI 序列或多字节 UTF-8 在分片边界被切断导致的乱码。
 * - 「增量水位线 + 视口按需补扫」双驱动：
 *   1. 新输出到达时增量扫描至 cursorLine（含当前行原地更新）；
 *   2. 用户滚屏翻阅历史（onScroll）时自动补扫当前视口及上下缓冲区，解决翻看数千行历史无高亮的问题。
 * - 同行多规则共存：整行铺底（wholeLine）渲染在 bottom 层，词级高亮（IP、时间、状态词、数值）
 *   叠加在 top 层，不再因命中整行规则而吞掉同行内其它语法元素。
 * - 尊重远端原生 ANSI 颜色：结构化规则（IP/时间/数字/路径/布尔）仅在底层单元格为默认前景色时着色，
 *   不破坏 `ls --color`、`git diff` 或彩色 Shell 提示符；告警级规则（error/warn/success/custom）优先生效。
 * - 列号严格按终端单元格宽度（CJK / emoji 占 2 列）换算，避免中文行内高亮横向偏移。
 */

export interface TerminalHighlightRule {
  key: string
  /** 语义级别，用于错误行导航、概览条与 ANSI 避让策略 */
  level?: string
  pattern: RegExp
  foregroundColor?: string
  backgroundColor?: string
  /** 整行铺底。开启时底色铺满整行，同行内词级高亮仍可在上层叠加 */
  wholeLine?: boolean
}

/** 内置兜底规则集 */
export const DEFAULT_HIGHLIGHT_RULES: TerminalHighlightRule[] = [
  {
    key: 'error',
    level: 'error',
    pattern: /\b(ERROR|FATAL|FAIL(?:ED|URE)?|PANIC|CRITICAL|Exception|Traceback|DENIED|REFUSED|TIMEOUT)\b/i,
    foregroundColor: '#ff6b6b',
    backgroundColor: '#451218'
  },
  {
    key: 'warn',
    level: 'warn',
    pattern: /\b(WARN(?:ING)?|DEPRECAT(?:ED|ION)|RETRY(?:ING)?|PENDING|WAITING|SLOW)\b/i,
    foregroundColor: '#fbbf24'
  },
  {
    key: 'success',
    level: 'success',
    pattern: /\b(SUCCESS(?:FUL)?|PASSED|OK|DONE|ACTIVE|RUNNING|ENABLED|CONNECTED|READY|ONLINE)\b/i,
    foregroundColor: '#4ade80'
  }
]

/** IP / file:line 可点击（点击复制） */
const LINK_PATTERNS: RegExp[] = [
  /\b\d{1,3}(?:\.\d{1,3}){3}(?::\d{1,5})?\b/g,
  /[A-Za-z0-9_./~-]+\.[A-Za-z0-9]+:\d+(?::\d+)?/g
]

export const HIGHLIGHT_STORAGE_KEY = 'snail_ssh_highlight_enabled'

const MAX_SCAN_LINES_PER_TICK = 240
const VIEWPORT_PADDING_LINES = 36
const MAX_LINE_RECORDS = 1500
const MAX_HITS_PER_LINE = 48

/** 即使单元格已有 ANSI 颜色也优先着色的强语义级别 */
const FORCE_FOREGROUND_LEVELS = new Set(['error', 'warn', 'success', 'custom'])

export interface UseTerminalHighlightOptions {
  onToast?: (msg: string, type?: 'info' | 'success' | 'warning' | 'error') => void
  rules?: TerminalHighlightRule[]
}

export interface TerminalHighlight {
  setEnabled(enabled: boolean): void
  isEnabled(): boolean
  /** 热更新规则：清空旧装饰后重扫当前视口与近期输出 */
  setRules(rules: TerminalHighlightRule[]): void
  /** 当前缓冲区中 error 级别命中的绝对行号（升序），用于上一条/下一条错误导航 */
  getProblemLines(): number[]
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
  /** 是否作为整行铺底背景层渲染 */
  wholeLine: boolean
}

interface LineRecord {
  marker: IMarker
  text: string
  decorations: IDecoration[]
  hasError: boolean
}

const compileRules = (rules: TerminalHighlightRule[]): CompiledRule[] =>
  rules.map((rule, priority) => {
    const flags = rule.pattern.flags.includes('g') ? rule.pattern.flags : `${rule.pattern.flags}g`
    return { rule, priority, scan: new RegExp(rule.pattern.source, flags) }
  })

/**
 * 收集一行内所有规则的命中：
 * - 若存在 wholeLine 规则命中，取优先级最高的一条作为底色层（wholeLine: true），
 *   同时保留该行内的词级命中（wholeLine: false）叠加在上层，实现整行底色与同行 IP/时间/关键字共存。
 * - 词级命中按「起点升序 + 规则优先级」排序并剔除重叠区间。
 */
const collectHits = (text: string, rules: CompiledRule[]): TextHit[] => {
  const rawHits: Array<{ start: number; end: number; rule: CompiledRule }> = []
  for (const rule of rules) {
    rule.scan.lastIndex = 0
    let m: RegExpExecArray | null
    while ((m = rule.scan.exec(text)) !== null) {
      if (!m[0]) {
        rule.scan.lastIndex++
        continue
      }
      rawHits.push({ start: m.index, end: m.index + m[0].length, rule })
      if (rawHits.length >= MAX_HITS_PER_LINE * 4) break
    }
  }
  if (rawHits.length === 0) return []

  const result: TextHit[] = []

  // 1. 选出最高优先级的整行规则（若存在），作为整行背景铺底层
  const bestWholeLine = rawHits
    .filter((hit) => hit.rule.rule.wholeLine === true)
    .sort((a, b) => a.rule.priority - b.rule.priority)[0]

  if (bestWholeLine) {
    result.push({
      start: 0,
      end: text.length,
      rule: bestWholeLine.rule,
      wholeLine: true
    })
  }

  // 2. 收集词级命中（含非整行规则，以及整行规则命中词本身以便突出关键词字色）
  const wordCandidates = rawHits
    .filter((hit) => hit.rule.rule.wholeLine !== true || hit.rule.rule.foregroundColor !== undefined)
    .sort((a, b) => a.start - b.start || a.rule.priority - b.rule.priority)

  let lastEnd = 0
  let wordCount = 0
  for (const hit of wordCandidates) {
    if (hit.start < lastEnd) continue
    result.push({
      start: hit.start,
      end: hit.end,
      rule: hit.rule,
      wholeLine: false
    })
    lastEnd = hit.end
    wordCount++
    if (wordCount >= MAX_HITS_PER_LINE) break
  }

  return result
}

/** 供设置面板实时预览复用：返回一行内的整行层与词级命中段 */
export function findHighlightHits(
  text: string,
  rules: TerminalHighlightRule[]
): Array<{ start: number; end: number; rule: TerminalHighlightRule; wholeLine: boolean }> {
  return collectHits(text, compileRules(rules)).map((hit) => ({
    start: hit.start,
    end: hit.end,
    rule: hit.rule.rule,
    wholeLine: hit.wholeLine
  }))
}

/**
 * 建立「字符串下标 → cell 列号」映射，修正 CJK / emoji 宽字符造成的横向偏移。
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
      col++
      continue
    }

    const chars = cell.getChars() || ' '
    for (let k = 0; k < chars.length && offset + k < textLength; k++) {
      columns[offset + k] = col
    }
    offset += chars.length
    col += width
  }

  columns[textLength] = col
  return columns
}

/** 命中区间内是否有单元格带显式前景色（palette / RGB） */
const hasExplicitForeground = (line: IBufferLine, from: number, to: number): boolean => {
  const end = Math.max(from + 1, to)
  for (let x = from; x < end; x++) {
    const cell = line.getCell(x)
    if (!cell || cell.getWidth() === 0) continue
    if (!cell.isFgDefault()) return true
  }
  return false
}

/** 只有 error 级别在右侧概览条打点；颜色必须是合法 #RRGGBB */
const rulerOptions = (rule: TerminalHighlightRule): { color: string; position: 'full' } | undefined => {
  if (rule.level !== 'error') return undefined
  const color = rule.foregroundColor ?? rule.backgroundColor
  if (!color || !/^#[0-9a-fA-F]{6}$/.test(color)) return undefined
  return { color, position: 'full' }
}

export function useTerminalHighlight(term: Terminal, options: UseTerminalHighlightOptions = {}): TerminalHighlight {
  let compiledRules = compileRules(options.rules ?? DEFAULT_HIGHLIGHT_RULES)
  const onToast = options.onToast

  let enabled = false
  let disposed = false
  let lastScannedAbsoluteLine = 0
  let needsViewportScan = false
  let rafId: number | null = null

  /** 按行维护装饰记录：一行共用一个 IMarker，方便原地重绘更新与滚动淘汰 */
  const lineRecords = new Set<LineRecord>()
  /** 缓存已确认无命中的行对象与对应文本，避免滚屏时对空白/普通行重复跑正则 */
  let noHitLineCache = new WeakMap<IBufferLine, string>()

  let linkProviderDisposable: IDisposable | null = null
  let writeParsedDisposable: IDisposable | null = null
  let resizeDisposable: IDisposable | null = null
  let scrollDisposable: IDisposable | null = null

  const currentAbsoluteLine = () => {
    const buffer = term.buffer.active
    return buffer.baseY + buffer.cursorY
  }

  const disposeRecord = (record: LineRecord) => {
    lineRecords.delete(record)
    try {
      record.marker.dispose()
    } catch {
      // ignore
    }
    for (const dec of record.decorations) {
      try {
        dec.dispose()
      } catch {
        // ignore
      }
    }
    record.decorations.length = 0
  }

  const clearDecorations = () => {
    for (const record of Array.from(lineRecords)) {
      disposeRecord(record)
    }
    lineRecords.clear()
    noHitLineCache = new WeakMap<IBufferLine, string>()
  }

  /** 尺寸变化后行列重排，清空后重扫当前视口与近期行 */
  const resetForResize = () => {
    if (!enabled) return
    clearDecorations()
    lastScannedAbsoluteLine = Math.max(0, currentAbsoluteLine() - MAX_SCAN_LINES_PER_TICK)
    needsViewportScan = true
    scheduleScan()
  }

  /** 为一行上的全部命中挂 decoration，共用单个行级 marker */
  const addHitsForLine = (
    line: IBufferLine,
    lineIndex: number,
    cursorLine: number,
    hits: TextHit[],
    text: string
  ): LineRecord | null => {
    const marker = term.registerMarker(lineIndex - cursorLine)
    if (!marker) return null

    const columns = hits.some((hit) => !hit.wholeLine) ? buildColumnMap(line, text.length) : null
    const decs: IDecoration[] = []
    let hasError = false
    let addedOverviewRuler = false

    for (const hit of hits) {
      const rule = hit.rule.rule
      if (rule.level === 'error') hasError = true

      if (hit.wholeLine) {
        const background = rule.backgroundColor
        if (!background) continue
        const ruler = !addedOverviewRuler ? rulerOptions(rule) : undefined
        if (ruler) addedOverviewRuler = true

        const dec = term.registerDecoration({
          marker,
          x: 0,
          width: term.cols,
          backgroundColor: background,
          layer: 'bottom',
          overviewRulerOptions: ruler
        })
        if (dec) decs.push(dec)
        continue
      }

      const from = columns?.[hit.start] ?? 0
      const to = columns?.[hit.end] ?? from + 1
      const width = Math.max(1, to - from)
      const background = rule.backgroundColor
      const foreground = rule.foregroundColor

      // 有显式背景色时，词级标签直接涂前景色保证对比度；
      // 纯前景色模式下，error/warn/success/custom 始终着色，结构化规则仅在默认前景色单元格上着色以保护 ls --color 等
      const forceFg = rule.level ? FORCE_FOREGROUND_LEVELS.has(rule.level) : false
      const canPaintForeground =
        foreground !== undefined &&
        (background !== undefined || forceFg || !hasExplicitForeground(line, from, from + width))

      if (!canPaintForeground && background === undefined) continue

      const ruler = !addedOverviewRuler ? rulerOptions(rule) : undefined
      if (ruler) addedOverviewRuler = true

      const dec = term.registerDecoration({
        marker,
        x: from,
        width,
        backgroundColor: background,
        foregroundColor: canPaintForeground ? foreground : undefined,
        layer: 'top',
        overviewRulerOptions: ruler
      })
      if (dec) decs.push(dec)
    }

    if (decs.length === 0) {
      marker.dispose()
      return null
    }

    const record: LineRecord = {
      marker,
      text,
      decorations: decs,
      hasError
    }

    lineRecords.add(record)
    marker.onDispose(() => {
      lineRecords.delete(record)
      for (const dec of decs) {
        try {
          dec.dispose()
        } catch {
          // ignore
        }
      }
      decs.length = 0
    })

    return record
  }

  /** 扫描单行：支持跳过未变化行、原地替换已更新行 */
  const scanSingleLine = (
    buffer: Terminal['buffer']['active'],
    lineIndex: number,
    cursorLine: number,
    activeByLine: Map<number, LineRecord>
  ) => {
    const line = buffer.getLine(lineIndex)
    if (!line) return
    const text = line.translateToString(true)
    const existing = activeByLine.get(lineIndex)

    if (!text) {
      if (existing) {
        disposeRecord(existing)
        activeByLine.delete(lineIndex)
      }
      return
    }

    if (existing) {
      // 内容未变化，直接复用已有高亮
      if (existing.text === text) return
      // 行内容发生原地刷新（如光标输入、\r 进度更新），清理旧装饰后重新匹配
      disposeRecord(existing)
      activeByLine.delete(lineIndex)
    } else if (noHitLineCache.get(line) === text) {
      return
    }

    const hits = collectHits(text, compiledRules)
    if (hits.length > 0) {
      const record = addHitsForLine(line, lineIndex, cursorLine, hits, text)
      if (record) activeByLine.set(lineIndex, record)
      noHitLineCache.delete(line)
    } else {
      noHitLineCache.set(line, text)
    }
  }

  const scan = () => {
    rafId = null
    if (disposed || !enabled) return
    const buffer = term.buffer.active
    // alt buffer（vim/less/top/htop）不做高亮，避免干扰全屏交互程序
    if (buffer.type !== 'normal') {
      lastScannedAbsoluteLine = currentAbsoluteLine()
      needsViewportScan = false
      return
    }

    const cursorLine = buffer.baseY + buffer.cursorY
    // 若发生了 clear 或屏幕回退，修正扫描水位线
    if (lastScannedAbsoluteLine > cursorLine) {
      lastScannedAbsoluteLine = Math.max(0, buffer.baseY)
    }

    // 构建当前有效行号索引
    const activeByLine = new Map<number, LineRecord>()
    for (const record of Array.from(lineRecords)) {
      if (record.marker.isDisposed || record.marker.line < 0) {
        disposeRecord(record)
        continue
      }
      activeByLine.set(record.marker.line, record)
    }

    // 1. 视口补扫：当用户向上滚动查看历史或刚开启/切换规则时，优先保证当前可见区域及周边行有高亮
    if (needsViewportScan) {
      needsViewportScan = false
      const vpStart = Math.max(0, buffer.viewportY - VIEWPORT_PADDING_LINES)
      const vpEnd = Math.min(cursorLine, buffer.viewportY + term.rows + VIEWPORT_PADDING_LINES)
      for (let y = vpStart; y <= vpEnd; y++) {
        scanSingleLine(buffer, y, cursorLine, activeByLine)
      }
    }

    // 2. 增量扫描：往回多看 1 行以覆盖刚完成换行的上一行及当前光标行
    let i = Math.max(0, lastScannedAbsoluteLine - 1)
    let scanned = 0

    while (i <= cursorLine && scanned < MAX_SCAN_LINES_PER_TICK) {
      scanSingleLine(buffer, i, cursorLine, activeByLine)
      i++
      scanned++
    }

    lastScannedAbsoluteLine = Math.max(0, i - 1)

    // 3. 容量淘汰：超出上限时优先回收距离当前视口最远的行记录，保证当前屏幕不闪烁
    if (lineRecords.size > MAX_LINE_RECORDS) {
      const viewportCenter = buffer.viewportY + Math.floor(term.rows / 2)
      const sorted = Array.from(lineRecords).sort(
        (a, b) => Math.abs(b.marker.line - viewportCenter) - Math.abs(a.marker.line - viewportCenter)
      )
      const overflow = lineRecords.size - MAX_LINE_RECORDS
      for (let k = 0; k < overflow; k++) {
        disposeRecord(sorted[k])
      }
    }

    // 本帧未扫完增量行，下一帧继续
    if (i <= cursorLine) scheduleScan()
  }

  const scheduleScan = () => {
    if (disposed || !enabled || rafId !== null) return
    rafId = requestAnimationFrame(scan)
  }

  /** 规则热更新：清空旧装饰后立即重扫视口与近期行 */
  const setRules = (next: TerminalHighlightRule[]) => {
    if (disposed) return
    compiledRules = compileRules(next)
    clearDecorations()
    if (!enabled) return
    lastScannedAbsoluteLine = Math.max(0, currentAbsoluteLine() - MAX_SCAN_LINES_PER_TICK)
    needsViewportScan = true
    scheduleScan()
  }

  const getProblemLines = (): number[] => {
    const lines = new Set<number>()
    for (const record of lineRecords) {
      if (record.hasError && !record.marker.isDisposed && record.marker.line >= 0) {
        lines.add(record.marker.line)
      }
    }
    return [...lines].sort((a, b) => a - b)
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

  // 监听视口滚动：翻看历史缓冲区时按需为当前视口补齐高亮
  if (typeof term.onScroll === 'function') {
    scrollDisposable = term.onScroll(() => {
      if (!enabled || disposed) return
      needsViewportScan = true
      scheduleScan()
    })
  }

  const setEnabled = (next: boolean) => {
    if (disposed) return
    if (next === enabled) return
    enabled = next

    if (enabled) {
      const cursorLine = currentAbsoluteLine()
      lastScannedAbsoluteLine = Math.max(0, cursorLine - MAX_SCAN_LINES_PER_TICK)
      needsViewportScan = true
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
    setRules,
    getProblemLines,
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
      scrollDisposable?.dispose()
      scrollDisposable = null
      clearDecorations()
    }
  }
}
