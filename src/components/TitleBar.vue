<template>
  <header
    class="h-10 bg-[#0e1117] border-b border-[#1f2430] flex items-center justify-between pl-3 pr-0 shrink-0 z-40 select-none"
    data-tauri-drag-region
    @dblclick="handleHeaderDblClick"
  >
    <!-- 1. 左侧：品牌 LOGO -->
    <div class="flex items-center gap-2.5 shrink-0 pr-3 pointer-events-none">
      <img
        src="/logo.svg"
        alt="SnailSSH"
        class="w-5 h-5 rounded-md shadow-glow-emerald object-contain select-none"
      />
      <span class="font-bold text-xs tracking-wider bg-gradient-to-r from-white via-slate-200 to-brand-300 bg-clip-text text-transparent">
        SnailSSH
      </span>
    </div>

    <div class="h-4 w-[1px] bg-slate-700/60 shrink-0"></div>

    <!-- 2. 中间：主机会话多标签栏 (Session Tabs) -->
    <div class="flex items-center gap-1.5 overflow-x-auto no-scrollbar flex-1 h-full px-2" data-tauri-drag-region>
      <!-- 固定项：主机资产库 -->
      <button
        @click="$emit('select-session', 'hosts')"
        :class="activeSessionId === 'hosts' ? 'bg-[#1a202c] text-emerald-400 font-semibold border border-brand-500/30 shadow-sm' : 'text-slate-400 hover:text-white hover:bg-surface-card/60 border border-transparent'"
        class="flex items-center gap-1.5 px-3 py-1 rounded-md text-xs transition-all shrink-0 font-medium"
      >
        <Server class="w-3.5 h-3.5 text-brand-400" />
        <span>主机资产</span>
      </button>

      <!-- 动态主机会话标签 -->
      <div
        v-for="s in sessions"
        :key="s.id"
        @click="$emit('select-session', s.id)"
        :class="activeSessionId === s.id ? 'bg-[#1a202c] text-emerald-400 font-semibold border border-brand-500/30 shadow-sm' : 'text-slate-400 hover:text-white hover:bg-surface-card/60 border border-transparent'"
        class="group flex items-center gap-2 px-2.5 py-1 rounded-md text-xs font-mono cursor-pointer transition-all shrink-0 max-w-[190px]"
      >
        <span class="w-2 h-2 rounded-full bg-emerald-400 shrink-0" :class="{ 'animate-pulse': activeSessionId === s.id }"></span>
        <span class="truncate">{{ s.title }}</span>
        <button
          @click.stop="$emit('close-session', s.id)"
          class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated hover:text-white text-slate-400 transition-opacity ml-0.5"
          title="关闭该会话"
        >
          <X class="w-3 h-3" />
        </button>
      </div>

      <!-- 快速新建/连接会话加号 -->
      <button
        @click="$emit('open-palette')"
        class="p-1 rounded-md text-slate-400 hover:text-white hover:bg-surface-card transition-colors shrink-0 ml-0.5"
        title="连接服务器 / 搜索 (Ctrl+K)"
      >
        <Plus class="w-3.5 h-3.5" />
      </button>
    </div>

    <!-- 3. 右侧：操作区与 Windows 风格窗口控制按钮 -->
    <div class="flex items-center h-full text-xs shrink-0">
      <!-- 搜索触发按键 -->
      <button
        @click="$emit('open-palette')"
        class="hidden lg:flex items-center gap-2 px-2.5 py-1 rounded-md bg-surface-card/70 border border-surface-border text-surface-subtle hover:text-white hover:border-brand-500/40 hover:bg-surface-card transition-all text-xs mr-2"
        title="全局快速搜索 (Ctrl+K)"
      >
        <Search class="w-3.5 h-3.5 text-slate-400" />
        <span class="text-[11px]">搜索</span>
        <kbd class="font-mono text-[9px] bg-surface-highlight text-slate-300 px-1 py-0.2 rounded border border-surface-border/80">Ctrl+K</kbd>
      </button>

      <!-- 传输任务指示悬浮胶囊 -->
      <button
        @click="$emit('toggle-transfers')"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card/70 border border-surface-border hover:border-brand-500/40 text-slate-300 hover:text-white transition-all text-xs mr-2"
        :title="activeTransfersCount > 0 ? '查看进行中的传输任务' : '当前没有进行中的传输'"
      >
        <div class="relative flex items-center justify-center">
          <ArrowUpDown class="w-3.5 h-3.5 text-brand-400" />
          <span v-if="activeTransfersCount > 0" class="absolute -top-1 -right-1 w-1.5 h-1.5 rounded-full bg-brand-500 animate-pulse"></span>
        </div>
        <template v-if="activeTransfersCount > 0">
          <span class="hidden md:inline font-mono text-[11px]">{{ activeTransfersCount }} 任务</span>
          <span class="text-brand-400 font-mono font-semibold text-[11px]">{{ transferSpeed }}</span>
        </template>
        <span v-else class="hidden md:inline font-mono text-[11px] text-surface-subtle">传输</span>
      </button>

      <!-- 快速新建主机按钮 -->
      <button
        @click="$emit('open-new-host')"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold transition-all shadow-glow-emerald active:scale-95 text-xs mr-1"
      >
        <Plus class="w-3.5 h-3.5 stroke-[2.5]" />
        <span class="hidden sm:inline">新建</span>
      </button>

      <!-- 系统设置 -->
      <button
        @click="$emit('open-settings')"
        class="p-1.5 rounded-md hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors mr-2"
        title="系统设置"
      >
        <Settings class="w-3.5 h-3.5" />
      </button>

      <div class="h-4 w-[1px] bg-slate-700/60 shrink-0 mr-1"></div>

      <!-- Windows 风格自定义窗口控制按钮组 (Window Controls) -->
      <div class="flex items-center h-full">
        <!-- 最小化 -->
        <button
          @click="minimizeWindow"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 active:bg-white/15 transition-colors"
          title="最小化"
        >
          <Minus class="w-3.5 h-3.5" />
        </button>

        <!-- 最大化 / 还原 -->
        <button
          @click="toggleMaximize"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 active:bg-white/15 transition-colors"
          :title="isMaximized ? '向下还原' : '最大化'"
        >
          <!-- 还原状态双层框图标 -->
          <svg v-if="isMaximized" class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="4.5" y="1.5" width="10" height="10" rx="0.5" />
            <path d="M1.5 5.5v9h9" />
          </svg>
          <!-- 最大化状态单方框图标 -->
          <Square v-else class="w-3 h-3 stroke-[1.2]" />
        </button>

        <!-- 关闭窗口 -->
        <button
          @click="closeWindow"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-[#e81123] active:bg-[#c4101e] transition-colors"
          title="关闭窗口"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import {
  Search,
  ArrowUpDown,
  Plus,
  Settings,
  Server,
  X,
  Minus,
  Square
} from 'lucide-vue-next'
import { isTauri } from '@/api/tauri'
import type { HostSession } from '@/types'

withDefaults(
  defineProps<{
    activeTransfersCount: number
    transferSpeed: string
    sessions?: HostSession[]
    activeSessionId?: string
  }>(),
  {
    sessions: () => [],
    activeSessionId: 'hosts'
  }
)

const emit = defineEmits<{
  (e: 'open-palette'): void
  (e: 'toggle-transfers'): void
  (e: 'open-new-host'): void
  (e: 'open-settings'): void
  (e: 'select-session', sessionId: string): void
  (e: 'close-session', sessionId: string): void
  (e: 'close-window'): void
}>()

const isMaximized = ref(false)

const checkMaximized = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      isMaximized.value = await getCurrentWindow().isMaximized()
    } catch {}
  }
}

const minimizeWindow = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().minimize()
    } catch (e) {
      console.warn('Tauri window minimize error:', e)
    }
  }
}

const toggleMaximize = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const win = getCurrentWindow()
      await win.toggleMaximize()
      isMaximized.value = await win.isMaximized()
    } catch (e) {
      console.warn('Tauri window toggleMaximize error:', e)
    }
  }
}

const closeWindow = () => {
  emit('close-window')
}

const handleHeaderDblClick = (e: MouseEvent) => {
  // 排除按钮点击引起的双击
  const target = e.target as HTMLElement
  if (target && target.closest('button')) return
  toggleMaximize()
}

onMounted(async () => {
  if (isTauri()) {
    await checkMaximized()
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const win = getCurrentWindow()
      await win.onResized(async () => {
        isMaximized.value = await win.isMaximized()
      })
    } catch {}
  }
})
</script>
