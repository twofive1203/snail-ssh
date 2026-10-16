<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden bg-[#0c0e14]">
    <!-- 终端标签页管理条 (Tab Bar) -->
    <div class="h-10 bg-surface-sidebar border-b border-surface-border flex items-center justify-between px-2 shrink-0 select-none">
      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar flex-1 mr-2">
        <div
          v-for="(tab, idx) in tabs"
          :key="tab.id"
          @click="$emit('switch-tab', idx)"
          @contextmenu.prevent="openTabMenu($event, idx)"
          :class="idx === activeTabIndex ? 'bg-[#0c0e14] border-t-2 border-brand-500 text-slate-200 border-x border-surface-border/50' : 'bg-surface-card/40 hover:bg-surface-card text-surface-subtle hover:text-slate-300 border-t-2 border-transparent'"
          class="group flex items-center gap-2 px-3 py-1.5 rounded-t-lg text-xs cursor-pointer transition-all shrink-0"
        >
          <span
            class="w-2 h-2 rounded-full"
            :class="tab.connected ? 'bg-emerald-400 animate-pulse' : 'bg-rose-500'"
          ></span>
          <span class="font-mono font-medium">{{ tab.title }}</span>
          <span class="text-[10px] font-mono text-slate-500 hidden sm:inline">{{ tab.sub }}</span>
          <button
            @click.stop="$emit('close-tab', idx)"
            class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-all ml-1"
          >
            <X class="w-3 h-3" />
          </button>
        </div>

        <!-- 添加新终端 -->
        <button
          @click="$emit('add-tab')"
          class="p-1 rounded-md text-surface-subtle hover:text-white hover:bg-surface-card transition-colors ml-1"
          title="连接新的服务器终端"
        >
          <Plus class="w-4 h-4" />
        </button>
      </div>

      <!-- 终端操作按钮组：展开文件树、查找、清屏 -->
      <div class="flex items-center gap-1 text-xs">
        <button
          v-if="sidebarCollapsed"
          @click="$emit('toggle-sidebar')"
          class="flex items-center gap-1.5 px-2 py-1 rounded bg-brand-500/10 text-emerald-400 border border-brand-500/30 hover:bg-brand-500/20 text-xs font-medium transition-all"
          title="展开左侧文件工作区"
        >
          <FolderGit2 class="w-3.5 h-3.5" />
          <span class="hidden sm:inline">文件工作区</span>
        </button>

        <span class="w-px h-4 bg-surface-border shrink-0"></span>

        <button
          @click="toggleHighlight"
          :title="highlightEnabled ? '关闭关键字高亮' : '开启关键字高亮（ERROR/WARN/成功）'"
          class="p-1.5 rounded hover:bg-surface-elevated transition-colors"
          :class="highlightEnabled ? 'text-amber-300 bg-amber-500/10' : 'text-surface-subtle hover:text-white'"
        >
          <Highlighter class="w-3.5 h-3.5" />
        </button>

        <button
          @click="toggleSearch"
          :disabled="!currentTab"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :class="searchOpen ? 'text-brand-300 bg-brand-500/10' : ''"
          title="在终端中查找 (Ctrl+F)"
        >
          <Search class="w-3.5 h-3.5" />
        </button>

        <button
          @click="clearCurrentTerminal"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="清除屏幕 (Ctrl+L)"
        >
          <Eraser class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- 标签页右键菜单：复制会话 / 复制 IP / 标签管理 -->
    <ContextMenu
      :open="tabMenu.open"
      :x="tabMenu.x"
      :y="tabMenu.y"
      :items="tabMenuItems"
      @select="handleTabMenuSelect"
      @close="tabMenu.open = false"
    />

    <!-- 终端内右键菜单：复制 / 粘贴 / 全选 / 查找 / 清除屏幕 -->
    <ContextMenu
      :open="termMenu.open"
      :x="termMenu.x"
      :y="termMenu.y"
      :items="termMenuItems"
      @select="handleTermMenuSelect"
      @close="termMenu.open = false"
    />

    <!-- 常用指令代码片段工具栏 -->
    <div class="h-8 bg-surface-card/40 border-b border-surface-border/60 px-3 flex items-center justify-between text-xs shrink-0 select-none">
      <div class="flex items-center gap-2 overflow-x-auto no-scrollbar py-1">
        <span class="text-[10px] font-mono text-slate-500 uppercase flex items-center gap-1">
          <Zap class="w-3 h-3 text-amber-400" />
          <span>常用指令:</span>
        </span>
        <button
          v-for="s in snippets"
          :key="s.label"
          @click="sendSnippetCommand(s.cmd)"
          :disabled="!currentTab"
          class="px-2 py-0.5 rounded bg-surface-card hover:bg-surface-elevated text-[11px] font-mono text-slate-300 hover:text-brand-300 border border-surface-border transition-colors shrink-0 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {{ s.label }}
        </button>
      </div>
      <div class="flex items-center gap-3 text-[11px] font-mono text-slate-500">
        <span>UTF-8</span>
        <span>xterm-256color</span>
        <span v-if="termSize">{{ termSize.cols }}x{{ termSize.rows }}</span>
      </div>
    </div>

    <!-- 真实 xterm.js 容器：每个标签页一个独立实例，切换不丢回显 -->
    <div class="flex-1 w-full h-full relative overflow-hidden bg-[#0c0e14] p-2">
      <!-- 终端内搜索条 (Ctrl+F)：贴在容器右上角，不抢占终端焦点 -->
      <div
        v-if="searchOpen"
        class="absolute top-3 right-4 z-30 flex items-center gap-1 bg-surface-sidebar/95 backdrop-blur-xl border border-surface-border rounded-lg shadow-2xl pl-2 pr-1 py-1"
      >
        <Search class="w-3.5 h-3.5 text-slate-500 shrink-0" />
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          spellcheck="false"
          placeholder="在终端中查找"
          class="w-40 bg-transparent text-xs font-mono text-slate-200 placeholder:text-slate-600 outline-none"
          @keydown.enter.prevent="onSearchEnter"
          @keydown.esc.prevent.stop="closeSearch"
        />
        <span class="text-[10px] font-mono text-slate-500 min-w-[42px] text-center shrink-0">{{ searchResultText }}</span>
        <button
          @click="runSearch('prev')"
          :disabled="!searchQuery"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
          title="上一个 (Shift+Enter)"
        >
          <ChevronUp class="w-3.5 h-3.5" />
        </button>
        <button
          @click="runSearch('next')"
          :disabled="!searchQuery"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
          title="下一个 (Enter)"
        >
          <ChevronDown class="w-3.5 h-3.5" />
        </button>
        <span class="w-px h-4 bg-surface-border shrink-0"></span>
        <button
          @click="searchCaseSensitive = !searchCaseSensitive"
          class="p-1 rounded transition-colors"
          :class="searchCaseSensitive ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="区分大小写"
        >
          <CaseSensitive class="w-3.5 h-3.5" />
        </button>
        <button
          @click="searchWholeWord = !searchWholeWord"
          class="p-1 rounded transition-colors"
          :class="searchWholeWord ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="全字匹配"
        >
          <WholeWord class="w-3.5 h-3.5" />
        </button>
        <button
          @click="searchRegex = !searchRegex"
          class="p-1 rounded transition-colors"
          :class="searchRegex ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="正则表达式"
        >
          <Regex class="w-3.5 h-3.5" />
        </button>
        <span class="w-px h-4 bg-surface-border shrink-0"></span>
        <button
          @click="closeSearch"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors"
          title="关闭 (Esc)"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>

      <div
        v-if="tabs.length === 0"
        class="w-full h-full flex flex-col items-center justify-center gap-3 text-center"
      >
        <TerminalIcon class="w-10 h-10 text-slate-600" />
        <p class="text-sm text-slate-400">当前没有活动的终端会话</p>
        <p class="text-xs text-slate-500 font-mono">请在「服务器资产」中选择主机并点击「连接 SSH」，或用 Ctrl+K 快速直连</p>
      </div>

      <div
        v-for="tab in tabs"
        :key="tab.id"
        :ref="el => setContainer(tab.id, el)"
        class="w-full h-full"
        :style="{ display: tab.id === currentTabId ? 'block' : 'none' }"
        @contextmenu.prevent="openTermMenu($event)"
      ></div>
    </div>

    <!-- 终端状态栏 -->
    <div class="h-6 bg-surface-sidebar border-t border-surface-border px-3 flex items-center justify-between text-[11px] font-mono text-surface-subtle shrink-0">
      <div class="flex items-center gap-3">
        <span class="flex items-center gap-1.5" :class="currentTab?.connected ? 'text-emerald-400' : 'text-rose-400'">
          <span class="w-1.5 h-1.5 rounded-full" :class="currentTab?.connected ? 'bg-emerald-400' : 'bg-rose-400'"></span>
          <span>{{ currentTab?.connected ? 'SSH 协议就绪 (OpenSSH 2.0 / Pty)' : '会话未连接' }}</span>
        </span>
        <span>编码: UTF-8</span>
      </div>
      <div>
        <span>快捷键: Ctrl+C 中断 | Ctrl+L 清屏</span>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { Terminal as Xterm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { SearchAddon } from '@xterm/addon-search'
import { Unicode11Addon } from '@xterm/addon-unicode11'
import {
  X, Plus, FolderGit2, Eraser, Zap, Terminal as TerminalIcon,
  CopyPlus, Globe, User, Terminal, PenLine, RefreshCw, XCircle, ChevronsRight,
  Search, ChevronUp, ChevronDown, CaseSensitive, WholeWord, Regex,
  Copy, ClipboardPaste, TextSelect, Highlighter
} from 'lucide-vue-next'
import ContextMenu from '@/components/ContextMenu.vue'
import { dialogState } from '@/composables/useDialog'
import { useTerminalHighlight, HIGHLIGHT_STORAGE_KEY, type TerminalHighlight } from '@/composables/useTerminalHighlight'
import type { ContextMenuItem, TerminalTab } from '@/types'
import {
  writeSshSessionApi,
  resizeSshSessionApi,
  getSshSessionCwdApi,
  setMockTerminalCwd,
  onSshStdout,
  onSshClosed,
  isTauri
} from '@/api/tauri'

const props = withDefaults(
  defineProps<{
    tabs: TerminalTab[]
    activeTabIndex: number
    sidebarCollapsed?: boolean
    /** 该终端所属的会话工作区是否为当前活动工作区（用于全局快捷键只在一个实例上生效） */
    active?: boolean
  }>(),
  {
    sidebarCollapsed: false,
    active: false
  }
)

const emit = defineEmits<{
  (e: 'switch-tab', index: number): void
  (e: 'close-tab', index: number): void
  (e: 'add-tab'): void
  (e: 'jump-to-sftp'): void
  (e: 'toggle-sidebar'): void
  (e: 'duplicate-tab', index: number): void
  (e: 'reconnect-tab', index: number): void
  (e: 'rename-tab', index: number): void
  (e: 'copy-tab-info', index: number, kind: 'ip' | 'login' | 'ssh-command'): void
  (e: 'open-tab-sftp', index: number): void
  (e: 'close-other-tabs', index: number): void
  (e: 'close-tabs-after', index: number): void
  (e: 'session-closed', tabId: string, reason: string): void
  (e: 'terminal-cwd-change', tabId: string, cwd: string): void
  (e: 'terminal-enter', tabId: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

interface TerminalSession {
  term: Xterm
  fit: FitAddon
  search: SearchAddon
  highlight: TerminalHighlight
  container: HTMLDivElement
  inputBuffer: string
  unlisten: Array<() => void>
  writeFailed: boolean
  /** 右键按下瞬间的选区快照（右键可能清空选区，需提前捕获） */
  rightClickSelection: string
}

const snippets = [
  { label: 'docker ps', cmd: 'docker ps\n' },
  { label: 'free -h', cmd: 'free -h\n' },
  { label: 'df -h', cmd: 'df -h\n' },
  { label: 'systemctl status nginx', cmd: 'systemctl status nginx\n' },
  { label: 'netstat -tulpn', cmd: 'netstat -tulpn\n' },
  { label: 'htop', cmd: 'htop\n' },
]

const terminalTheme = {
  background: '#0c0e14',
  foreground: '#e2e8f0',
  cursor: '#10b981',
  selectionBackground: 'rgba(16, 185, 129, 0.3)',
  black: '#1e293b',
  red: '#ef4444',
  green: '#10b981',
  yellow: '#f59e0b',
  blue: '#06b6d4',
  magenta: '#d946ef',
  cyan: '#38bdf8',
  white: '#f8fafc',
}

const containerMap = new Map<string, HTMLDivElement>()
const sessions = new Map<string, TerminalSession>()
const termSize = ref<{ cols: number; rows: number }>({ cols: 80, rows: 24 })

// 关键字高亮开关（默认开，存 localStorage）
const readHighlightEnabled = (): boolean => {
  try {
    return localStorage.getItem(HIGHLIGHT_STORAGE_KEY) !== '0'
  } catch {
    return true
  }
}
const highlightEnabled = ref(readHighlightEnabled())

const currentTabId = computed(() => props.tabs[props.activeTabIndex]?.id ?? '')
const currentTab = computed(() => props.tabs[props.activeTabIndex])
const currentSession = computed(() => sessions.get(currentTabId.value))

const setContainer = (tabId: string, el: any) => {
  if (el) {
    containerMap.set(tabId, el as HTMLDivElement)
  } else {
    containerMap.delete(tabId)
  }
}

const createSession = (tab: TerminalTab, container: HTMLDivElement): TerminalSession => {
  const term = new Xterm({
    fontFamily: '"JetBrains Mono", "Fira Code", monospace',
    fontSize: 13,
    lineHeight: 1.3,
    cursorBlink: true,
    cursorStyle: 'block',
    theme: terminalTheme,
    allowProposedApi: true,
  })

  const fit = new FitAddon()
  const search = new SearchAddon()
  term.loadAddon(fit)
  term.loadAddon(search)
  // 中日韩宽字符宽度修正（中文环境收益明显）
  term.loadAddon(new Unicode11Addon())
  term.unicode.activeVersion = '11'
  term.loadAddon(new WebLinksAddon())

  const session: TerminalSession = {
    term, fit, search,
    highlight: useTerminalHighlight(term, { onToast: (msg, type) => emit('toast', msg, type ?? 'info') }),
    container, inputBuffer: '', unlisten: [], writeFailed: false, rightClickSelection: ''
  }

  term.open(container)
  session.highlight.setEnabled(highlightEnabled.value)

  // 搜索结果计数（仅在开启 search decorations 时触发）
  search.onDidChangeResults((result) => {
    if (tab.id === currentTabId.value) searchResult.value = { resultIndex: result.resultIndex, resultCount: result.resultCount }
  })

  // 右键会先触发 mousedown，此处用捕获阶段提前保存选区
  container.addEventListener('mousedown', (e) => {
    if (e.button === 2) session.rightClickSelection = term.getSelection()
  }, true)

  // 监听 OSC 7 (标准终端当前目录转义序列)
  term.parser.registerOscHandler(7, (data) => {
    const p = parseOsc7Path(data)
    if (p) {
      tab.cwd = p
      emit('terminal-cwd-change', tab.id, p)
    }
    return true
  })

  // 监听 OSC 1337 (iTerm2 / Shell Integration 扩展目录格式)
  term.parser.registerOscHandler(1337, (data) => {
    const p = parseOsc1337Path(data)
    if (p) {
      tab.cwd = p
      emit('terminal-cwd-change', tab.id, p)
    }
    return true
  })

  // 绑定用户输入
  term.onData((data) => {
    if (data.includes('\r')) {
      emit('terminal-enter', tab.id)
    }
    writeToSession(tab.id, data)
  })

  // 接管终端内快捷键：剪贴板与查找（返回 false 阻止 xterm 继续处理）
  term.attachCustomKeyEventHandler((e) => {
    if (e.type !== 'keydown') return true
    const mod = e.ctrlKey || e.metaKey
    const key = e.key

    // 复制：Ctrl+Shift+C / Ctrl+Insert
    if (mod && ((e.shiftKey && key.toLowerCase() === 'c') || key === 'Insert')) {
      e.preventDefault()
      copySelection(tab.id)
      return false
    }
    // 粘贴：Ctrl+Shift+V / Shift+Insert
    if ((mod && e.shiftKey && key.toLowerCase() === 'v') || (e.shiftKey && key === 'Insert')) {
      e.preventDefault()
      pasteToTerminal(tab.id)
      return false
    }
    // 终端内查找：Ctrl+F
    if (mod && !e.shiftKey && key.toLowerCase() === 'f') {
      e.preventDefault()
      if (tab.id === currentTabId.value) openSearch()
      return false
    }
    return true
  })

  // 窗口尺寸变化时同步给远端 PTY
  term.onResize(({ cols, rows }) => {
    if (tab.id !== currentTabId.value) return
    termSize.value = { cols, rows }
    if (isTauri()) {
      resizeSshSessionApi(tab.id, cols, rows).catch(() => {})
    }
  })

  // 真实会话不打印假欢迎信息，避免误导连接状态；仅浏览器预览时输出演示横幅
  if (!isTauri()) {
    printMockBanner(term)
  }

  return session
}

const bindSessionEvents = async (tab: TerminalTab, session: TerminalSession) => {
  if (!isTauri()) return
  const unlistenStdout = await onSshStdout(tab.id, (chunk) => {
    session.term.write(chunk)
  })
  const unlistenClosed = await onSshClosed(tab.id, (reason) => {
    session.term.write(`\r\n\x1b[33m[会话结束] ${reason}\x1b[0m\r\n`)
    emit('session-closed', tab.id, reason)
  })
  session.unlisten.push(unlistenStdout, unlistenClosed)
}

/** 按标签页同步 xterm 实例：新增则创建，移除则销毁 */
const syncSessions = async () => {
  await nextTick()

  for (const [tabId, session] of sessions) {
    if (props.tabs.some(t => t.id === tabId)) continue
    session.unlisten.forEach(fn => fn())
    session.highlight.dispose()
    session.term.dispose()
    sessions.delete(tabId)
  }

  for (const tab of props.tabs) {
    if (sessions.has(tab.id)) continue
    const container = containerMap.get(tab.id)
    if (!container) continue
    const session = createSession(tab, container)
    sessions.set(tab.id, session)
    // 可见元素才能量出真实行列数，隐藏的标签页等切换时再 fit
    if (container.offsetParent) {
      session.fit.fit()
      termSize.value = { cols: session.term.cols, rows: session.term.rows }
    }
    await bindSessionEvents(tab, session)
  }

  fitActive()
}

const fitActive = () => {
  const session = sessions.get(currentTabId.value)
  if (!session || !session.container.offsetParent) return
  session.fit.fit()
  termSize.value = { cols: session.term.cols, rows: session.term.rows }
}

const focusActive = () => {
  const session = sessions.get(currentTabId.value)
  if (!session || !session.container.offsetParent) return
  session.term.focus()
}

const clearCurrentTerminal = () => clearTerminal(currentTabId.value)

/** 按标签页清屏（右键菜单可作用于非活动标签） */
const clearTerminal = (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  if (isTauri()) {
    // 交给远端 Shell 执行清屏，保留当前提示符
    writeSshSessionApi(tabId, '\x0c').catch(() => {})
  } else {
    session.term.clear()
  }
}

// ==========================================
// 终端内搜索 / 剪贴板 / 右键菜单
// ==========================================
const searchOpen = ref(false)
const searchQuery = ref('')
const searchCaseSensitive = ref(false)
const searchWholeWord = ref(false)
const searchRegex = ref(false)
const searchResult = ref<{ resultIndex: number; resultCount: number }>({ resultIndex: -1, resultCount: 0 })
const searchInputRef = ref<HTMLInputElement | null>(null)

const emitToast = (msg: string, type: 'info' | 'success' | 'warning' | 'error' = 'info') => emit('toast', msg, type)

const searchResultText = computed(() => {
  if (!searchQuery.value) return ''
  const { resultIndex, resultCount } = searchResult.value
  if (resultCount <= 0) return '无结果'
  return `${resultIndex + 1}/${resultCount}`
})

const buildSearchOptions = () => ({
  regex: searchRegex.value,
  wholeWord: searchWholeWord.value,
  caseSensitive: searchCaseSensitive.value,
  incremental: true,
  decorations: {
    matchBackground: '#14532d',
    matchOverviewRuler: '#22c55e',
    activeMatchBackground: '#b45309',
    activeMatchColorOverviewRuler: '#fbbf24'
  }
})

const runSearch = (direction: 'next' | 'prev') => {
  const session = currentSession.value
  if (!session) return
  const q = searchQuery.value
  if (!q) {
    session.search.clearDecorations()
    searchResult.value = { resultIndex: -1, resultCount: 0 }
    return
  }
  const options = buildSearchOptions()
  if (direction === 'next') session.search.findNext(q, options)
  else session.search.findPrevious(q, options)
}

const onSearchEnter = (e: KeyboardEvent) => {
  runSearch(e.shiftKey ? 'prev' : 'next')
}

const openSearch = () => {
  const session = currentSession.value
  if (!session) return
  searchOpen.value = true
  nextTick(() => {
    // 搜索输入框独立于 xterm，不抢终端按键
    session.term.blur()
    searchInputRef.value?.focus()
    searchInputRef.value?.select()
    if (searchQuery.value) runSearch('next')
  })
}

const closeSearch = () => {
  if (!searchOpen.value) return
  searchOpen.value = false
  currentSession.value?.search.clearDecorations()
  searchResult.value = { resultIndex: -1, resultCount: 0 }
  nextTick(() => focusActive())
}

const toggleSearch = () => (searchOpen.value ? closeSearch() : openSearch())

/** 关键字高亮开关：作用于所有已创建的终端实例 */
const toggleHighlight = () => {
  highlightEnabled.value = !highlightEnabled.value
  try {
    localStorage.setItem(HIGHLIGHT_STORAGE_KEY, highlightEnabled.value ? '1' : '0')
  } catch {
    // localStorage 不可用时仅本次会话生效
  }
  for (const session of sessions.values()) session.highlight.setEnabled(highlightEnabled.value)
  emitToast(highlightEnabled.value ? '关键字高亮已开启' : '关键字高亮已关闭', 'info')
}

watch([searchQuery, searchCaseSensitive, searchWholeWord, searchRegex], () => {
  if (searchOpen.value) runSearch('next')
})

/** 统一的会话写入入口（真实 SSH 与预览降级共用） */
const writeToSession = (tabId: string, data: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  if (isTauri()) {
    writeSshSessionApi(tabId, data).catch((err: any) => {
      if (session.writeFailed) return
      session.writeFailed = true
      session.term.write(`\r\n\x1b[31m[写入失败] ${err?.message || err}\x1b[0m\r\n`)
    })
  } else {
    const tab = props.tabs.find(t => t.id === tabId)
    if (tab) handleMockTerminalInput(tab, session, data)
  }
}

const copySelection = async (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  const text = session.term.getSelection()
  if (!text) {
    emitToast('当前没有选中的文本', 'warning')
    return
  }
  try {
    await navigator.clipboard.writeText(text)
    emitToast('已复制选中内容', 'success')
  } catch (e: any) {
    emitToast(`复制失败: ${e?.message || e}`, 'error')
  }
}

const pasteToTerminal = async (tabId: string) => {
  try {
    const text = await navigator.clipboard.readText()
    if (!text) {
      emitToast('剪贴板为空', 'warning')
      return
    }
    writeToSession(tabId, text)
  } catch {
    emitToast('无法读取剪贴板，请使用 Ctrl+V 粘贴', 'warning')
  }
}

const selectAllInTerminal = (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  session.term.selectAll()
  emitToast('已全选终端内容', 'info')
}

const termMenu = ref({ open: false, x: 0, y: 0, tabId: '', hasSelection: false })

const openTermMenu = (event: MouseEvent) => {
  const session = currentSession.value
  if (!session) return
  const selection = session.term.getSelection() || session.rightClickSelection
  termMenu.value = { open: true, x: event.clientX, y: event.clientY, tabId: currentTabId.value, hasSelection: !!selection }
}

const termMenuItems = computed<ContextMenuItem[]>(() => [
  { key: 'copy', label: '复制', icon: Copy, hint: 'Ctrl+Shift+C', disabled: !termMenu.value.hasSelection },
  { key: 'paste', label: '粘贴', icon: ClipboardPaste, hint: 'Ctrl+Shift+V' },
  { key: 'select-all', label: '全选', icon: TextSelect },
  { divider: true },
  { key: 'find', label: '查找', icon: Search, hint: 'Ctrl+F' },
  { key: 'clear', label: '清除屏幕', icon: Eraser }
])

const handleTermMenuSelect = (item: ContextMenuItem) => {
  const tabId = termMenu.value.tabId
  if (!tabId) return
  switch (item.key) {
    case 'copy': {
      const session = sessions.get(tabId)
      const text = session?.term.getSelection() || session?.rightClickSelection || ''
      if (!text) {
        emitToast('当前没有选中的文本', 'warning')
        break
      }
      navigator.clipboard.writeText(text)
        .then(() => emitToast('已复制选中内容', 'success'))
        .catch((e: any) => emitToast(`复制失败: ${e?.message || e}`, 'error'))
      break
    }
    case 'paste': pasteToTerminal(tabId); break
    case 'select-all': selectAllInTerminal(tabId); break
    case 'find':
      if (tabId === currentTabId.value) openSearch()
      else emitToast('请先切换到该终端再查找', 'info')
      break
    case 'clear': clearTerminal(tabId); break
  }
}

// ==========================================
// 标签页右键菜单
// ==========================================
const tabMenu = ref({ open: false, x: 0, y: 0, index: -1 })

const openTabMenu = (event: MouseEvent, index: number) => {
  tabMenu.value = { open: true, x: event.clientX, y: event.clientY, index }
}

const tabMenuItems = computed<ContextMenuItem[]>(() => {
  const index = tabMenu.value.index
  const tab = props.tabs[index]
  if (!tab) return []
  const isLast = index === props.tabs.length - 1
  return [
    { key: 'duplicate', label: '复制会话', icon: CopyPlus, hint: '新标签页' },
    { divider: true },
    { key: 'copy-ip', label: '复制 IP 地址', icon: Globe },
    { key: 'copy-login', label: '复制登录信息', icon: User, hint: 'user@host' },
    { key: 'copy-ssh', label: '复制 SSH 命令', icon: Terminal, hint: 'ssh -p' },
    { divider: true },
    { key: 'rename', label: '重命名标签', icon: PenLine },
    { key: 'reconnect', label: '重新连接', icon: RefreshCw, disabled: tab.connected },
    { key: 'sftp', label: '打开 SFTP', icon: FolderGit2 },
    { key: 'clear', label: '清除屏幕', icon: Eraser },
    { divider: true },
    { key: 'close', label: '关闭标签', icon: X, danger: true },
    { key: 'close-others', label: '关闭其他标签', icon: XCircle, disabled: props.tabs.length < 2 },
    { key: 'close-after', label: '关闭右侧标签', icon: ChevronsRight, disabled: isLast }
  ]
})

const handleTabMenuSelect = (item: ContextMenuItem) => {
  const index = tabMenu.value.index
  const tab = props.tabs[index]
  if (!tab) return
  switch (item.key) {
    case 'duplicate': emit('duplicate-tab', index); break
    case 'copy-ip': emit('copy-tab-info', index, 'ip'); break
    case 'copy-login': emit('copy-tab-info', index, 'login'); break
    case 'copy-ssh': emit('copy-tab-info', index, 'ssh-command'); break
    case 'rename': emit('rename-tab', index); break
    case 'reconnect': emit('reconnect-tab', index); break
    case 'sftp': emit('open-tab-sftp', index); break
    case 'clear': clearTerminal(tab.id); break
    case 'close': emit('close-tab', index); break
    case 'close-others': emit('close-other-tabs', index); break
    case 'close-after': emit('close-tabs-after', index); break
  }
}

const sendSnippetCommand = (cmd: string) => {
  const session = currentSession.value
  const tab = currentTab.value
  if (!session || !tab) return
  if (isTauri()) {
    writeSshSessionApi(tab.id, cmd).catch((err: any) => {
      session.term.write(`\r\n\x1b[31m[写入失败] ${err?.message || err}\x1b[0m\r\n`)
    })
  } else {
    session.term.write(cmd)
    handleMockTerminalInput(tab, session, '\r')
  }
}

const parseOsc7Path = (data: string): string | null => {
  try {
    if (data.startsWith('file://')) {
      const url = new URL(data)
      return decodeURIComponent(url.pathname)
    }
    if (data.startsWith('/')) {
      return data.trim()
    }
  } catch {
    const stripped = data.replace(/^file:\/\/[^/]*/, '')
    if (stripped.startsWith('/')) {
      return decodeURIComponent(stripped)
    }
  }
  return null
}

const parseOsc1337Path = (data: string): string | null => {
  if (data.startsWith('CurrentDir=')) {
    const p = data.substring('CurrentDir='.length).trim()
    if (p.startsWith('/')) return p
  }
  return null
}

const getCurrentCwd = async (): Promise<string> => {
  const tab = currentTab.value
  if (!tab) return '/'
  if (isTauri()) {
    try {
      const remoteCwd = await getSshSessionCwdApi(tab.id)
      if (remoteCwd) {
        tab.cwd = remoteCwd
        emit('terminal-cwd-change', tab.id, remoteCwd)
        return remoteCwd
      }
    } catch (e) {
      console.warn('获取终端目录失败:', e)
    }
  }
  return tab.cwd || '/'
}

// ==========================================
// 浏览器预览 (非 Tauri) 降级：模拟终端打字响应
// ==========================================
const printMockBanner = (term: Xterm) => {
  term.write('\r\n\x1b[1;36m SnailSSH 浏览器预览模式（未连接真实主机）\x1b[0m\r\n')
  term.write('\x1b[90m在桌面客户端中运行可建立真实 SSH 会话。\x1b[0m\r\n\r\n')
  term.write('\x1b[1;32mpreview@snail-ssh\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m$ \x1b[0m')
}

const writeMockPrompt = (term: Xterm) => {
  term.write('\x1b[1;32mpreview@snail-ssh\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m$ \x1b[0m')
}

const handleMockTerminalInput = (tab: TerminalTab, session: TerminalSession, data: string) => {
  const { term } = session
  if (data === '\r') {
    term.write('\r\n')
    const cmd = session.inputBuffer.trim()
    session.inputBuffer = ''
    if (cmd === 'clear') {
      term.clear()
    } else if (cmd.startsWith('cd ')) {
      let target = cmd.slice(3).trim()
      if (target === '~' || target === '') target = '/root'
      else if (!target.startsWith('/')) target = '/root/' + target
      tab.cwd = target
      setMockTerminalCwd(tab.id, target)
      emit('terminal-cwd-change', tab.id, target)
      term.write(`\x1b[32m已切换到目录: ${target}\x1b[0m\r\n`)
    } else if (cmd === 'docker ps') {
      term.write('\x1b[90mCONTAINER ID   IMAGE          COMMAND       STATUS       PORTS                   NAMES\x1b[0m\r\n')
      term.write('\x1b[32ma9c812d4ef01\x1b[0m   nginx:alpine   "/docker..."  Up 2 hours   0.0.0.0:80->80/tcp      web-gateway\r\n')
      term.write('\x1b[32mf4b008e1a7b2\x1b[0m   redis:7.2      "docker..."   Up 3 days    0.0.0.0:6379->6379/tcp  redis-prod\r\n')
    } else if (cmd === 'free -h') {
      term.write('               total        used        free      shared  buff/cache   available\r\n')
      term.write('Mem:            15Gi       9.6Gi       1.2Gi       180Mi       4.2Gi       5.2Gi\r\n')
      term.write('Swap:          4.0Gi       320Mi       3.7Gi\r\n')
    } else if (cmd.length > 0) {
      term.write(`\x1b[33mCommand executed: ${cmd} (exit: 0)\x1b[0m\r\n`)
    }
    writeMockPrompt(term)
  } else if (data === '\u007F') {
    if (session.inputBuffer.length > 0) {
      session.inputBuffer = session.inputBuffer.slice(0, -1)
      term.write('\b \b')
    }
  } else if (data === '\x0c') {
    term.clear()
    writeMockPrompt(term)
  } else {
    session.inputBuffer += data
    term.write(data)
  }
}

const handleWindowResize = () => fitActive()

/** 全局 Ctrl+F：仅在当前活动工作区生效，终端聚焦时交给 xterm 自定义处理器 */
const handleGlobalKeydown = (e: KeyboardEvent) => {
  if (!props.active || dialogState.open) return
  const mod = e.ctrlKey || e.metaKey
  if (!mod || e.shiftKey || e.key.toLowerCase() !== 'f') return
  const el = e.target as HTMLElement | null
  if (el && typeof el.closest === 'function' && el.closest('.xterm')) return
  if (!currentSession.value) return
  e.preventDefault()
  openSearch()
}

// 标签页增删 -> 创建/销毁终端实例
watch(() => props.tabs.map(t => t.id).join('|'), () => { syncSessions() })
// 切换标签页 -> 重新计算尺寸（隐藏时无法 fit），并关闭搜索条避免作用域错乱
watch(() => props.activeTabIndex, async () => {
  if (searchOpen.value) closeSearch()
  await nextTick()
  fitActive()
})

onMounted(async () => {
  window.addEventListener('resize', handleWindowResize)
  window.addEventListener('keydown', handleGlobalKeydown)
  await syncSessions()
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleWindowResize)
  window.removeEventListener('keydown', handleGlobalKeydown)
  for (const session of sessions.values()) {
    session.unlisten.forEach(fn => fn())
    session.highlight.dispose()
    session.term.dispose()
  }
  sessions.clear()
})

defineExpose({
  sendCommand: sendSnippetCommand,
  clearTerminal: clearCurrentTerminal,
  getCurrentCwd,
  fit: fitActive,
  focus: focusActive
})
</script>
