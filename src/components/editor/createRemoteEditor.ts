import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { bracketMatching, foldGutter, foldKeymap, forceParsing, indentOnInput } from '@codemirror/language'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import { EditorState, type Extension } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from '@codemirror/view'
import { structuralFolding } from './fold'
import { detectLineEnding, resolveEditorLanguage } from './language'

export interface EditorCursor {
  line: number
  col: number
}

export interface RemoteEditorHandle {
  languageLabel: string
  lineEndingLabel: string
  getValue: () => string
  getCursor: () => EditorCursor
  focus: () => void
  destroy: () => void
}

const editorPhrases: Record<string, string> = {
  Find: '查找',
  Replace: '替换',
  next: '下一个',
  previous: '上一个',
  all: '全部',
  'match case': '区分大小写',
  regexp: '正则',
  'by word': '全词',
  replace: '替换',
  'replace all': '全部替换',
  close: '关闭',
  'Go to line': '跳转到行',
  go: '跳转',
  'Fold line': '折叠',
  'Unfold line': '展开',
  'folded code': '已折叠',
  unfold: '展开',
}

const snailTheme = EditorView.theme({
  '&': {
    height: '100%',
    backgroundColor: '#0a0d14',
    fontSize: '12px',
  },
  '.cm-scroller': {
    fontFamily: '"JetBrains Mono", "Fira Code", Consolas, monospace',
    lineHeight: '1.55',
    overflow: 'auto',
  },
  '.cm-content': {
    padding: '8px 0',
    caretColor: '#10b981',
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: '#10b981',
  },
  '.cm-gutters': {
    backgroundColor: '#0c0e14',
    color: '#64748b',
    borderRight: '1px solid #262d40',
  },
  '.cm-activeLineGutter': {
    backgroundColor: '#161a26',
    color: '#cbd5e1',
  },
  '.cm-activeLine': {
    backgroundColor: 'rgba(47, 56, 80, 0.45)',
  },
  '.cm-foldGutter .cm-gutterElement': {
    color: '#8892b0',
    padding: '0 2px',
  },
  '.cm-lineNumbers .cm-gutterElement': {
    padding: '0 8px 0 4px',
  },
  '.cm-foldPlaceholder': {
    backgroundColor: '#1d2333',
    border: 'none',
    color: '#7dddb3',
    padding: '0 4px',
  },
  '.cm-panels': {
    backgroundColor: '#11141d',
    color: '#e2e8f0',
  },
  '.cm-panels.cm-panels-top': {
    borderBottom: '1px solid #262d40',
  },
  '.cm-panel.cm-search': {
    padding: '6px 8px',
    fontSize: '12px',
  },
  '.cm-textfield': {
    backgroundColor: '#0c0e14',
    color: '#e2e8f0',
    border: '1px solid #262d40',
    borderRadius: '4px',
    padding: '2px 6px',
  },
  '.cm-button': {
    backgroundColor: '#1d2333',
    color: '#e2e8f0',
    border: '1px solid #262d40',
    borderRadius: '4px',
    padding: '1px 8px',
    cursor: 'pointer',
  },
  '.cm-selectionMatch': {
    backgroundColor: 'rgba(16, 185, 129, 0.18)',
  },
}, { dark: true })

function cursorAt(state: EditorState): EditorCursor {
  const pos = state.selection.main.head
  const line = state.doc.lineAt(pos)
  return { line: line.number, col: pos - line.from + 1 }
}

/**
 * 远程配置编辑器。
 * 用 CodeMirror 6 而不是 Monaco（VS Code 编辑器内核）：官方语言包已覆盖
 * json/yaml/xml/nginx/toml/shell，折叠不依赖 Worker，体积也更适合当前安装包。
 */
export function createRemoteEditor(options: {
  parent: HTMLElement
  path: string
  text: string
  onChange: (value: string, cursor: EditorCursor) => void
}): RemoteEditorHandle {
  const language = resolveEditorLanguage(options.path, options.text)
  const ending = detectLineEnding(options.text)
  const extensions: Extension[] = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    foldGutter({ openText: '▾', closedText: '▸' }),
    drawSelection(),
    EditorState.allowMultipleSelections.of(true),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    keymap.of([
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      ...foldKeymap,
      indentWithTab,
    ]),
    oneDark,
    snailTheme,
    EditorState.tabSize.of(2),
    EditorState.phrases.of(editorPhrases),
    EditorView.contentAttributes.of({ spellcheck: 'false' }),
    language.extension,
    EditorView.updateListener.of((update) => {
      if (update.docChanged || update.selectionSet) {
        options.onChange(update.state.sliceDoc(), cursorAt(update.state))
      }
    }),
  ]
  if (language.structuralFold) extensions.push(structuralFolding())
  if (ending.separator === '\r\n') extensions.push(EditorState.lineSeparator.of('\r\n'))

  const view = new EditorView({
    parent: options.parent,
    state: EditorState.create({
      doc: options.text,
      extensions,
    }),
  })
  // 配置文件通常不大。先解析完，打开时标签/代码块折叠标记就能出现，不必等空闲解析
  if (options.text.length < 512_000) forceParsing(view, view.state.doc.length, 100)
  view.focus()

  return {
    languageLabel: language.label,
    lineEndingLabel: ending.label,
    getValue: () => view.state.sliceDoc(),
    getCursor: () => cursorAt(view.state),
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  }
}
