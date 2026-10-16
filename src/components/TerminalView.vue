<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden bg-[#0c0e14]">
    <!-- 终端标签页管理条 (Tab Bar) -->
    <div class="h-10 bg-surface-sidebar border-b border-surface-border flex items-center justify-between px-2 shrink-0 select-none">
      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar flex-1 mr-2">
        <div
          v-for="(tab, idx) in tabs"
          :key="tab.id"
          @click="$emit('switch-tab', idx)"
          :class="idx === activeTabIndex ? 'bg-[#0c0e14] border-t-2 border-brand-500 text-slate-200 border-x border-surface-border/50' : 'bg-surface-card/40 hover:bg-surface-card text-surface-subtle hover:text-slate-300 border-t-2 border-transparent'"
          class="group flex items-center gap-2 px-3 py-1.5 rounded-t-lg text-xs cursor-pointer transition-all shrink-0"
        >
          <span
            class="w-2 h-2 rounded-full"
            :class="tab.connected ? 'bg-emerald-400 animate-pulse' : 'bg-amber-400'"
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
          title="打开新终端"
        >
          <Plus class="w-4 h-4" />
        </button>
      </div>

      <!-- 终端操作按钮组：打开SFTP、分屏、清屏 -->
      <div class="flex items-center gap-1 text-xs">
        <button
          @click="$emit('jump-to-sftp')"
          class="flex items-center gap-1.5 px-2 py-1 rounded bg-cyan-500/10 text-cyan-400 border border-cyan-500/30 hover:bg-cyan-500/20 text-xs font-medium transition-all"
          title="打开此服务器的 SFTP 目录"
        >
          <FolderGit2 class="w-3.5 h-3.5" />
          <span class="hidden sm:inline">打开 SFTP</span>
        </button>

        <button
          @click="$emit('toggle-split')"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="水平/垂直分屏"
        >
          <Columns2 class="w-3.5 h-3.5" />
        </button>

        <button
          @click="clearCurrentTerminal"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="清除屏幕"
        >
          <Eraser class="w-3.5 h-3.5" />
        </button>

        <button
          @click="$emit('broadcast-mode')"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="多会话指令广播 (Broadcast)"
        >
          <Radio class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

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
          class="px-2 py-0.5 rounded bg-surface-card hover:bg-surface-elevated text-[11px] font-mono text-slate-300 hover:text-brand-300 border border-surface-border transition-colors shrink-0"
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

    <!-- 真实 xterm.js 容器 -->
    <div class="flex-1 w-full h-full relative overflow-hidden bg-[#0c0e14] p-2">
      <div ref="terminalContainerRef" class="w-full h-full"></div>
    </div>

    <!-- 终端状态栏 -->
    <div class="h-6 bg-surface-sidebar border-t border-surface-border px-3 flex items-center justify-between text-[11px] font-mono text-surface-subtle shrink-0">
      <div class="flex items-center gap-3">
        <span class="flex items-center gap-1.5 text-emerald-400">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
          <span>SSH 协议就绪 (OpenSSH 2.0 / Pty)</span>
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
import { ref, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { Terminal as Xterm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { X, Plus, FolderGit2, Columns2, Eraser, Radio, Zap } from 'lucide-vue-next'
import type { TerminalTab } from '@/types'
import { writeSshSessionApi, resizeSshSessionApi, onSshStdout, isTauri } from '@/api/tauri'

const props = defineProps<{
  tabs: TerminalTab[]
  activeTabIndex: number
}>()

defineEmits<{
  (e: 'switch-tab', index: number): void
  (e: 'close-tab', index: number): void
  (e: 'add-tab'): void
  (e: 'jump-to-sftp'): void
  (e: 'toggle-split'): void
  (e: 'broadcast-mode'): void
}>()

const terminalContainerRef = ref<HTMLDivElement | null>(null)
let xtermInstance: Xterm | null = null
let fitAddon: FitAddon | null = null
let unlistenStdout: (() => void) | null = null
const termSize = ref<{ cols: number; rows: number }>({ cols: 80, rows: 24 })

const snippets = [
  { label: 'docker ps', cmd: 'docker ps\n' },
  { label: 'free -h', cmd: 'free -h\n' },
  { label: 'df -h', cmd: 'df -h\n' },
  { label: 'systemctl status nginx', cmd: 'systemctl status nginx\n' },
  { label: 'netstat -tulpn', cmd: 'netstat -tulpn\n' },
  { label: 'htop', cmd: 'htop\n' },
]

const initTerminal = () => {
  if (!terminalContainerRef.value) return

  // 销毁旧实例
  if (xtermInstance) {
    xtermInstance.dispose()
    xtermInstance = null
  }

  const term = new Xterm({
    fontFamily: '"JetBrains Mono", "Fira Code", monospace',
    fontSize: 13,
    lineHeight: 1.3,
    cursorBlink: true,
    cursorStyle: 'block',
    theme: {
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
    },
    allowProposedApi: true,
  })

  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.loadAddon(new WebLinksAddon())

  term.open(terminalContainerRef.value)
  fitAddon.fit()

  termSize.value = { cols: term.cols, rows: term.rows }

  // 绑定用户输入
  term.onData((data) => {
    const currentTab = props.tabs[props.activeTabIndex]
    if (!currentTab) return

    if (isTauri()) {
      writeSshSessionApi(currentTab.id, data)
    } else {
      // 浏览器预览 Mock 响应
      handleMockTerminalInput(term, data)
    }
  })

  // 监听窗口缩放
  term.onResize(({ cols, rows }) => {
    termSize.value = { cols, rows }
    const currentTab = props.tabs[props.activeTabIndex]
    if (currentTab && isTauri()) {
      resizeSshSessionApi(currentTab.id, cols, rows)
    }
  })

  xtermInstance = term

  // 打印欢迎信息或连接
  printWelcomeMotd(term)
  bindSessionStdout()
}

// 终端标准 MOTD 输出
const printWelcomeMotd = (term: Xterm) => {
  term.write('\r\n\x1b[1;32m   _____             _ _   _____ _____ _    _ \x1b[0m\r\n')
  term.write('\x1b[1;32m  / ____|           (_) | / ____/ ____| |  | |\x1b[0m\r\n')
  term.write('\x1b[1;32m | (___  _ __   __ _ _| || (___| (___ | |__| |\x1b[0m\r\n')
  term.write('\x1b[1;32m  \\___ \\| \'_ \\ / _` | | | \\___ \\\\___ \\|  __  |\x1b[0m\r\n')
  term.write('\x1b[1;32m  ____) | | | | (_| | | | ____) |___) | |  | |\x1b[0m\r\n')
  term.write('\x1b[1;32m |_____/|_| |_|\\__,_|_|_||_____/_____/|_|  |_|\x1b[0m\r\n\r\n')
  term.write('\x1b[90mConnected via SSH-2.0-OpenSSH_8.9p1 Ubuntu (x86_64)\x1b[0m\r\n')
  term.write('\x1b[36mLast login: Fri Feb 20 18:24:12 2025 from 123.120.45.67\x1b[0m\r\n\r\n')
  term.write('\x1b[1;32mroot@k8s-master\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m# \x1b[0m')
}

// 模拟打字响应逻辑 (浏览器开发降级)
let currentInputBuffer = ''
const handleMockTerminalInput = (term: Xterm, data: string) => {
  if (data === '\r') {
    // 回车
    term.write('\r\n')
    const cmd = currentInputBuffer.trim()
    currentInputBuffer = ''
    if (cmd === 'clear') {
      term.clear()
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
    term.write('\x1b[1;32mroot@k8s-master\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m# \x1b[0m')
  } else if (data === '\u007F') {
    // 退格
    if (currentInputBuffer.length > 0) {
      currentInputBuffer = currentInputBuffer.slice(0, -1)
      term.write('\b \b')
    }
  } else {
    currentInputBuffer += data
    term.write(data)
  }
}

const bindSessionStdout = async () => {
  if (unlistenStdout) {
    unlistenStdout()
    unlistenStdout = null
  }
  const currentTab = props.tabs[props.activeTabIndex]
  if (!currentTab) return

  if (isTauri()) {
    unlistenStdout = await onSshStdout(currentTab.id, (chunk) => {
      if (xtermInstance) {
        xtermInstance.write(chunk)
      }
    })
  }
}

const sendSnippetCommand = (cmd: string) => {
  if (!xtermInstance) return
  const currentTab = props.tabs[props.activeTabIndex]
  if (currentTab && isTauri()) {
    writeSshSessionApi(currentTab.id, cmd)
  } else {
    xtermInstance.write(cmd)
    handleMockTerminalInput(xtermInstance, '\r')
  }
}

const clearCurrentTerminal = () => {
  if (xtermInstance) {
    xtermInstance.clear()
    xtermInstance.write('\x1b[1;32mroot@k8s-master\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m# \x1b[0m')
  }
}

const handleWindowResize = () => {
  if (fitAddon) {
    fitAddon.fit()
  }
}

watch(() => props.activeTabIndex, () => {
  nextTick(() => {
    initTerminal()
  })
})

onMounted(() => {
  nextTick(() => {
    initTerminal()
    window.addEventListener('resize', handleWindowResize)
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleWindowResize)
  if (unlistenStdout) unlistenStdout()
  if (xtermInstance) xtermInstance.dispose()
})
</script>
