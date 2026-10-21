import { css } from '@codemirror/lang-css'
import { html } from '@codemirror/lang-html'
import { javascript } from '@codemirror/lang-javascript'
import { json } from '@codemirror/lang-json'
import { python } from '@codemirror/lang-python'
import { xml } from '@codemirror/lang-xml'
import { yaml } from '@codemirror/lang-yaml'
import { StreamLanguage } from '@codemirror/language'
import type { Extension } from '@codemirror/state'
import { dockerFile } from '@codemirror/legacy-modes/mode/dockerfile'
import { nginx } from '@codemirror/legacy-modes/mode/nginx'
import { properties } from '@codemirror/legacy-modes/mode/properties'
import { shell } from '@codemirror/legacy-modes/mode/shell'
import { toml } from '@codemirror/legacy-modes/mode/toml'

export interface EditorLanguage {
  label: string
  /**
   * 官方 Lezer 语言包自带语法折叠（JSON 对象、YAML 块、XML/HTML 标签）。
   * 流式语法（nginx/shell/toml 等）没有折叠信息，需要缩进/括号折叠补上。
   */
  structuralFold: boolean
  extension: Extension
}

function fileName(path: string): string {
  const parts = path.split(/[/\\]/)
  return (parts[parts.length - 1] || '').toLowerCase()
}

function looksLikeNginx(content: string): boolean {
  return /(^|\n)\s*(http|server|events|location|upstream|stream|mail)\s*\{/.test(content)
}

function pathHintsNginx(path: string, name: string): boolean {
  const lower = path.toLowerCase()
  return name === 'nginx.conf'
    || name.endsWith('.nginx')
    || (name.endsWith('.conf') && (name.includes('nginx') || lower.includes('/nginx/') || lower.includes('\\nginx\\')))
}

function stream(parser: Parameters<typeof StreamLanguage.define>[0], label: string): EditorLanguage {
  return {
    label,
    structuralFold: true,
    extension: StreamLanguage.define(parser),
  }
}

/** 按路径和内容选高亮。内容只用于 .conf 这种后缀无法区分 nginx / ini 的情况。 */
export function resolveEditorLanguage(path: string, content: string): EditorLanguage {
  const name = fileName(path)
  if (name === 'dockerfile' || name.startsWith('dockerfile.') || name.endsWith('.dockerfile')) {
    return stream(dockerFile, 'Dockerfile')
  }
  if (name === '.env' || name.startsWith('.env.') || name.endsWith('.env')) {
    return stream(properties, 'ENV')
  }
  if (pathHintsNginx(path, name) || (name.endsWith('.conf') && looksLikeNginx(content))) {
    return stream(nginx, 'Nginx')
  }

  const ext = name.includes('.') ? name.slice(name.lastIndexOf('.') + 1) : ''
  switch (ext) {
    case 'json':
    case 'jsonc':
      return { label: 'JSON', structuralFold: false, extension: json() }
    case 'yaml':
    case 'yml':
      return { label: 'YAML', structuralFold: false, extension: yaml() }
    case 'xml':
    case 'svg':
    case 'plist':
      return { label: 'XML', structuralFold: false, extension: xml() }
    case 'html':
    case 'htm':
      return { label: 'HTML', structuralFold: false, extension: html() }
    case 'css':
      return { label: 'CSS', structuralFold: false, extension: css() }
    case 'py':
      return { label: 'Python', structuralFold: false, extension: python() }
    case 'js':
    case 'mjs':
    case 'cjs':
    case 'jsx':
      return { label: 'JavaScript', structuralFold: false, extension: javascript({ jsx: ext === 'jsx' }) }
    case 'ts':
    case 'tsx':
      return { label: 'TypeScript', structuralFold: false, extension: javascript({ typescript: true, jsx: ext === 'tsx' }) }
    case 'sh':
    case 'bash':
    case 'zsh':
      return stream(shell, 'Shell')
    case 'toml':
      return stream(toml, 'TOML')
    case 'ini':
    case 'properties':
    case 'cfg':
    case 'cnf':
    case 'service':
    case 'conf':
      return stream(properties, 'Properties')
    default:
      return { label: '纯文本', structuralFold: true, extension: [] }
  }
}

/** 统一换行才能原样写回。混合换行无法逐行保留，保存时会规范成 LF。 */
export function detectLineEnding(text: string): { separator: '\n' | '\r\n'; label: string } {
  const crlf = text.split('\r\n').length - 1
  const bareLf = text.replace(/\r\n/g, '').split('\n').length - 1
  const bareCr = text.replace(/\r\n/g, '').includes('\r')
  if (crlf > 0 && bareLf === 0 && !bareCr) return { separator: '\r\n', label: 'CRLF' }
  if (crlf > 0 || bareCr) return { separator: '\n', label: '混合→LF' }
  return { separator: '\n', label: 'LF' }
}
