<template>
  <header
    class="h-11 bg-surface-sidebar border-b border-surface-border flex items-center justify-between px-3 shrink-0 z-30 select-none"
    data-tauri-drag-region
  >
    <!-- 左侧：应用 LOGO & 仿 macOS 装饰圆点 -->
    <div class="flex items-center gap-3">
      <div class="flex items-center gap-2 mr-1">
        <span
          @click="$emit('close-window')"
          class="w-3 h-3 rounded-full bg-[#ff5f56] inline-block shadow-sm cursor-pointer hover:opacity-80 transition-opacity"
          title="关闭窗口"
        ></span>
        <span
          @click="minimizeWindow"
          class="w-3 h-3 rounded-full bg-[#ffbd2e] inline-block shadow-sm cursor-pointer hover:opacity-80 transition-opacity"
          title="最小化"
        ></span>
        <span
          @click="toggleMaximize"
          class="w-3 h-3 rounded-full bg-[#27c93f] inline-block shadow-sm cursor-pointer hover:opacity-80 transition-opacity"
          title="全屏缩放"
        ></span>
      </div>

      <div class="h-4 w-[1px] bg-surface-border mx-1"></div>

      <!-- App Brand -->
      <div class="flex items-center gap-2">
        <img
          src="/logo.svg"
          alt="SnailSSH"
          class="w-6 h-6 rounded-md shadow-glow-emerald object-contain select-none"
        />
        <span class="font-bold text-sm tracking-wide bg-gradient-to-r from-white via-slate-200 to-brand-300 bg-clip-text text-transparent">
          SnailSSH
        </span>
        <span class="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-brand-500/10 text-brand-400 border border-brand-500/20 font-semibold tracking-wider">
          v1.0.0
        </span>
      </div>
    </div>

    <!-- 中间：全局搜索与快捷命令入口 (Ctrl+K 呼出) -->
    <div class="flex items-center">
      <button
        @click="$emit('open-palette')"
        class="flex items-center gap-2.5 px-3 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-surface-subtle hover:text-white hover:border-brand-500/40 hover:bg-surface-elevated transition-all duration-200 w-80 justify-between group shadow-inner"
      >
        <span class="flex items-center gap-2">
          <Search class="w-3.5 h-3.5 text-slate-400 group-hover:text-brand-400 transition-colors" />
          <span>快速搜索主机、命令、快捷操作...</span>
        </span>
        <kbd class="font-mono text-[10px] bg-surface-highlight text-slate-300 px-1.5 py-0.5 rounded border border-surface-border/80">Ctrl + K</kbd>
      </button>
    </div>

    <!-- 右侧：环境状态、传输总览按钮、新建服务器、设置 -->
    <div class="flex items-center gap-2 text-xs">
      <!-- 传输任务指示悬浮胶囊 -->
      <button
        @click="$emit('toggle-transfers')"
        class="flex items-center gap-2 px-2.5 py-1 rounded-md bg-surface-card border border-surface-border hover:border-brand-500/40 text-slate-300 hover:text-white transition-all text-xs"
        :title="activeTransfersCount > 0 ? '查看进行中的传输任务' : '当前没有进行中的传输'"
      >
        <div class="relative flex items-center justify-center">
          <ArrowUpDown class="w-3.5 h-3.5 text-brand-400" />
          <span v-if="activeTransfersCount > 0" class="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-brand-500 animate-pulse"></span>
        </div>
        <template v-if="activeTransfersCount > 0">
          <span class="hidden md:inline font-mono text-[11px]">{{ activeTransfersCount }} 任务传输中</span>
          <span class="text-brand-400 font-mono font-semibold text-[11px]">{{ transferSpeed }}</span>
        </template>
        <span v-else class="hidden md:inline font-mono text-[11px] text-surface-subtle">传输空闲</span>
      </button>

      <!-- 快速新建主机按钮 -->
      <button
        @click="$emit('open-new-host')"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold transition-all shadow-glow-emerald active:scale-95 text-xs"
      >
        <Plus class="w-3.5 h-3.5 stroke-[2.5]" />
        <span>新建主机</span>
      </button>

      <div class="h-4 w-[1px] bg-surface-border mx-1"></div>

      <!-- 系统设置 -->
      <button
        @click="$emit('open-settings')"
        class="p-1.5 rounded-md hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
        title="系统设置"
      >
        <Settings class="w-4 h-4" />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { Search, ArrowUpDown, Plus, Settings } from 'lucide-vue-next'
import { isTauri } from '@/api/tauri'

defineProps<{
  activeTransfersCount: number
  transferSpeed: string
}>()

defineEmits<{
  (e: 'open-palette'): void
  (e: 'toggle-transfers'): void
  (e: 'open-new-host'): void
  (e: 'open-settings'): void
  (e: 'close-window'): void
}>()

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
      await getCurrentWindow().toggleMaximize()
    } catch (e) {
      console.warn('Tauri window maximize error:', e)
    }
  }
}
</script>
