import { foldService } from '@codemirror/language'
import type { EditorState, Extension } from '@codemirror/state'

const OPENERS = '{[('
const CLOSERS = '}])'
const MAX_SCAN_LINES = 1500

/**
 * 流式语法和纯文本没有语法树折叠。这里补两种：
 * 未闭合的括号（nginx 的 server { }），以及缩进更深的后续行（shell / 无后缀配置）。
 * 有 Lezer 折叠的语言不要挂这个扩展，否则会盖住 XML/HTML 标签折叠。
 */
export function structuralFolding(): Extension {
  return foldService.of((state, lineStart) => bracketFold(state, lineStart) ?? indentFold(state, lineStart))
}

function leadingWhitespace(text: string): number {
  let i = 0
  while (i < text.length && (text[i] === ' ' || text[i] === '\t')) i++
  return i
}

function indentFold(state: EditorState, lineStart: number): { from: number; to: number } | null {
  const line = state.doc.lineAt(lineStart)
  if (!line.text.trim()) return null
  const indent = leadingWhitespace(line.text)
  let end = -1
  const last = Math.min(state.doc.lines, line.number + MAX_SCAN_LINES)
  for (let n = line.number + 1; n <= last; n++) {
    const next = state.doc.line(n)
    if (!next.text.trim()) continue
    if (leadingWhitespace(next.text) > indent) end = next.to
    else break
  }
  if (end <= line.to) return null
  return { from: line.to, to: end }
}

function bracketFold(state: EditorState, lineStart: number): { from: number; to: number } | null {
  const line = state.doc.lineAt(lineStart)
  const opener = firstUnclosed(line.text)
  if (!opener) return null

  const closer = CLOSERS[OPENERS.indexOf(opener.ch)]
  let depth = 1
  let inStr: string | null = null
  const lastLine = Math.min(state.doc.lines, line.number + MAX_SCAN_LINES)

  for (let n = line.number; n <= lastLine; n++) {
    const current = state.doc.line(n)
    const from = n === line.number ? opener.index + 1 : 0
    const text = current.text
    for (let i = from; i < text.length; i++) {
      const ch = text[i]
      if (inStr) {
        if (ch === '\\') {
          i++
          continue
        }
        if (ch === inStr) inStr = null
        continue
      }
      if (ch === '"' || ch === "'" || ch === '`') {
        inStr = ch
        continue
      }
      if (ch === '#' || (ch === '/' && text[i + 1] === '/')) break
      if (ch === opener.ch) depth++
      else if (ch === closer) {
        depth--
        if (depth === 0) {
          if (current.number === line.number) return null
          // 收起中间内容，保留闭合括号所在行，方便对上标签/代码块结尾
          if (current.from <= line.to) return null
          return { from: line.to, to: current.from }
        }
      }
    }
  }
  return null
}

function firstUnclosed(text: string): { ch: string; index: number } | null {
  const stack: { ch: string; index: number }[] = []
  let inStr: string | null = null
  for (let i = 0; i < text.length; i++) {
    const ch = text[i]
    if (inStr) {
      if (ch === '\\') {
        i++
        continue
      }
      if (ch === inStr) inStr = null
      continue
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      inStr = ch
      continue
    }
    if (ch === '#' || (ch === '/' && text[i + 1] === '/')) break
    const openAt = OPENERS.indexOf(ch)
    if (openAt >= 0) {
      stack.push({ ch, index: i })
      continue
    }
    const closeAt = CLOSERS.indexOf(ch)
    if (closeAt >= 0 && stack.length && stack[stack.length - 1].ch === OPENERS[closeAt]) {
      stack.pop()
    }
  }
  return stack[0] ?? null
}
