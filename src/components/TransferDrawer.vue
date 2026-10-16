<template>
  <div
    class="fixed bottom-4 right-4 w-96 max-h-[460px] bg-surface-sidebar/95 backdrop-blur-xl border border-surface-border rounded-xl shadow-2xl flex flex-col z-40 transition-all duration-300"
    :class="open ? 'translate-y-0 opacity-100 pointer-events-auto' : 'translate-y-4 opacity-0 pointer-events-none'"
  >
    <!-- 头部 -->
    <div class="p-3 border-b border-surface-border flex items-center justify-between shrink-0">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-brand-500 animate-pulse"></div>
        <h3 class="text-xs font-bold text-white font-mono uppercase tracking-wide">传输任务队列 (Transfers)</h3>
      </div>
      <div class="flex items-center gap-2">
        <button
          @click="$emit('clear-completed')"
          class="text-[11px] text-surface-subtle hover:text-white transition-colors"
          title="清除已完成"
        >
          清除已完成
        </button>
        <button
          @click="$emit('close')"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- 任务列表 -->
    <div class="flex-1 overflow-y-auto p-3 space-y-2.5 custom-scrollbar">
      <div v-if="tasks.length === 0" class="p-8 text-center text-xs text-slate-500 font-mono">
        暂无活跃传输任务
      </div>

      <div
        v-for="task in tasks"
        :key="task.id"
        class="p-2.5 rounded-lg bg-surface-card border border-surface-border/80 space-y-2"
        :class="{ 'opacity-65': task.status === 'completed' }"
      >
        <div class="flex items-center justify-between text-xs">
          <div class="flex items-center gap-2 truncate pr-2">
            <component
              :is="task.type === 'upload' ? ArrowUpCircle : ArrowDownCircle"
              class="w-4 h-4 shrink-0"
              :class="task.type === 'upload' ? 'text-brand-400' : 'text-cyan-400'"
            />
            <span class="font-medium text-slate-200 truncate">{{ task.name }}</span>
          </div>
          <span
            class="font-mono text-[11px] font-semibold shrink-0"
            :class="task.status === 'completed' ? 'text-emerald-400' : task.status === 'failed' ? 'text-rose-400' : 'text-brand-400'"
          >
            {{ task.status === 'completed' ? '完成' : task.status === 'failed' ? '失败' : `${task.progress}%` }}
          </span>
        </div>

        <!-- 进度条 -->
        <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="task.type === 'upload' ? 'bg-gradient-to-r from-brand-500 to-cyan-400' : 'bg-cyan-500'"
            :style="{ width: `${task.progress}%` }"
          ></div>
        </div>

        <div class="flex items-center justify-between text-[10px] font-mono text-slate-400">
          <span>{{ task.size }}</span>
          <span>{{ task.status === 'completed' ? '传输完成' : `${task.speed}` }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ArrowUpCircle, ArrowDownCircle, X } from 'lucide-vue-next'
import type { TransferTask } from '@/types'

defineProps<{
  open: boolean
  tasks: TransferTask[]
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'clear-completed'): void
}>()
</script>
