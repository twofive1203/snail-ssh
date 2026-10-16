<template>
  <div
    class="fixed bottom-4 right-4 w-96 max-h-[460px] bg-surface-sidebar/95 backdrop-blur-xl border border-surface-border rounded-xl shadow-2xl flex flex-col z-40 transition-all duration-300"
    :class="open ? 'translate-y-0 opacity-100 pointer-events-auto' : 'translate-y-4 opacity-0 pointer-events-none'"
  >
    <!-- 头部 -->
    <div class="p-3 border-b border-surface-border flex items-center justify-between shrink-0">
      <div class="flex items-center gap-2">
        <div
          class="w-2 h-2 rounded-full"
          :class="runningCount + pausedCount > 0 ? 'bg-brand-500 animate-pulse' : 'bg-slate-600'"
        ></div>
        <h3 class="text-xs font-bold text-white font-mono uppercase tracking-wide">传输任务队列</h3>
        <span class="text-[10px] font-mono text-slate-500">{{ headerText }}</span>
      </div>
      <div class="flex items-center gap-2">
        <button
          @click="$emit('clear-completed')"
          class="text-[11px] text-surface-subtle hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!hasFinished"
          title="移除已完成 / 失败 / 已取消的记录"
        >
          清除已结束
        </button>
        <button
          @click="$emit('close')"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white"
          title="收起传输面板（Ctrl+T）"
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
        :class="{ 'opacity-65': task.status !== 'running' && task.status !== 'paused' }"
      >
        <div class="flex items-center justify-between text-xs gap-2">
          <div class="flex items-center gap-2 truncate">
            <component
              :is="task.type === 'upload' ? ArrowUpCircle : ArrowDownCircle"
              class="w-4 h-4 shrink-0"
              :class="task.type === 'upload' ? 'text-brand-400' : 'text-cyan-400'"
            />
            <span class="font-medium text-slate-200 truncate" :title="task.name">{{ task.name }}</span>
          </div>
          <div class="flex items-center gap-1.5 shrink-0">
            <span class="font-mono text-[11px] font-semibold" :class="statusClass(task)">{{ statusLabel(task) }}</span>
            <div class="flex items-center gap-0.5">
              <button
                v-if="task.status === 'running'"
                @click="$emit('pause-task', task)"
                class="p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-amber-300 transition-colors"
                title="暂停"
              >
                <Pause class="w-3 h-3" />
              </button>
              <button
                v-if="task.status === 'paused'"
                @click="$emit('resume-task', task)"
                class="p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-brand-300 transition-colors"
                title="继续"
              >
                <Play class="w-3 h-3" />
              </button>
              <button
                v-if="task.status === 'running' || task.status === 'paused'"
                @click="$emit('cancel-task', task)"
                class="p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-rose-300 transition-colors"
                title="取消"
              >
                <Square class="w-3 h-3" />
              </button>
              <button
                v-if="task.status === 'failed' || task.status === 'cancelled'"
                @click="$emit('retry-task', task)"
                class="p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-brand-300 transition-colors"
                title="从已传位置重试"
              >
                <RotateCcw class="w-3 h-3" />
              </button>
              <button
                v-if="task.status !== 'running' && task.status !== 'paused'"
                @click="$emit('remove-task', task.id)"
                class="p-0.5 rounded hover:bg-surface-elevated text-slate-500 hover:text-white transition-colors"
                title="从列表中移除"
              >
                <X class="w-3 h-3" />
              </button>
            </div>
          </div>
        </div>

        <!-- 进度条 -->
        <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
          <div
            class="h-full rounded-full transition-all duration-300"
            :class="barClass(task)"
            :style="{ width: `${task.progress}%` }"
          ></div>
        </div>

        <div class="flex items-center justify-between text-[10px] font-mono text-slate-400 gap-2">
          <span class="truncate" :title="task.size">{{ task.size }}</span>
          <span class="truncate text-right" :title="task.speed">
            {{ task.status === 'completed' ? '传输完成' : task.speed }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { ArrowUpCircle, ArrowDownCircle, X, Pause, Play, Square, RotateCcw } from 'lucide-vue-next'
import type { TransferTask } from '@/types'

const props = defineProps<{
  open: boolean
  tasks: TransferTask[]
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'clear-completed'): void
  (e: 'remove-task', taskId: string): void
  (e: 'pause-task', task: TransferTask): void
  (e: 'resume-task', task: TransferTask): void
  (e: 'cancel-task', task: TransferTask): void
  (e: 'retry-task', task: TransferTask): void
}>()

const runningCount = computed(() => props.tasks.filter(t => t.status === 'running').length)
const pausedCount = computed(() => props.tasks.filter(t => t.status === 'paused').length)
const hasFinished = computed(() => props.tasks.some(t => t.status !== 'running' && t.status !== 'paused'))

const headerText = computed(() => {
  const parts: string[] = []
  if (runningCount.value > 0) parts.push(`${runningCount.value} 进行中`)
  if (pausedCount.value > 0) parts.push(`${pausedCount.value} 已暂停`)
  if (parts.length > 0) return parts.join(' · ')
  return props.tasks.length > 0 ? '已全部结束' : ''
})

const statusLabel = (task: TransferTask): string => {
  switch (task.status) {
    case 'completed': return '完成'
    case 'failed': return '失败'
    case 'cancelled': return '已取消'
    case 'paused': return '已暂停'
    default: return `${task.progress}%`
  }
}

const statusClass = (task: TransferTask): string => {
  switch (task.status) {
    case 'completed': return 'text-emerald-400'
    case 'failed': return 'text-rose-400'
    case 'cancelled': return 'text-slate-400'
    case 'paused': return 'text-amber-400'
    default: return 'text-brand-400'
  }
}

const barClass = (task: TransferTask): string => {
  if (task.status === 'failed') return 'bg-rose-500'
  if (task.status === 'cancelled') return 'bg-slate-600'
  if (task.status === 'paused') return 'bg-amber-500'
  return task.type === 'upload' ? 'bg-gradient-to-r from-brand-500 to-cyan-400' : 'bg-cyan-500'
}
</script>
