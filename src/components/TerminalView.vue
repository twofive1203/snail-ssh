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

      <!-- 终端操作按钮组：展开文件树、清屏 -->
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
import {
  X, Plus, FolderGit2, Eraser, Zap, Terminal as TerminalIcon,
  CopyPlus, Globe, User, Terminal, PenLine, RefreshCw, XCircle, ChevronsRight
} from 'lucide-vue-next'
import ContextMenu from '@/components/ContextMenu.vue'
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
  }>(),
  {
    sidebarCollapsed: false
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
}>()

interface TerminalSession {
  term: Xterm
  fit: FitAddon
  container: HTMLDivElement
  inputBuffer: string
  unlisten: Array<() => void>
  writeFailed: boolean
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
  term.loadAddon(fit)
  term.loadAddon(new WebLinksAddon())

  const session: TerminalSession = { term, fit, container, inputBuffer: '', unlisten: [], writeFailed: false }

  term.open(container)

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
    if (isTauri()) {
      writeSshSessionApi(tab.id, data).catch((err: any) => {
        if (session.writeFailed) return
        session.writeFailed = true
        term.write(`\r\n\x1b[31m[写入失败] ${err?.message || err}\x1b[0m\r\n`)
      })
    } else {
      handleMockTerminalInput(tab, session, data)
    }
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

// 标签页增删 -> 创建/销毁终端实例
watch(() => props.tabs.map(t => t.id).join('|'), () => { syncSessions() })
// 切换标签页 -> 重新计算尺寸（隐藏时无法 fit）
watch(() => props.activeTabIndex, async () => {
  await nextTick()
  fitActive()
})

onMounted(async () => {
  window.addEventListener('resize', handleWindowResize)
  await syncSessions()
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleWindowResize)
  for (const session of sessions.values()) {
    session.unlisten.forEach(fn => fn())
    session.term.dispose()
  }
  sessions.clear()
})

defineExpose({
  sendCommand: sendSnippetCommand,
  clearTerminal: clearCurrentTerminal,
  getCurrentCwd,
  fit: fitActive
})
</script>
