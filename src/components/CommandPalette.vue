<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-start justify-center pt-24 p-4 transition-opacity"
    @click="$emit('close')"
  >
    <div
      class="w-full max-w-xl bg-surface-sidebar border border-surface-border rounded-xl shadow-2xl overflow-hidden flex flex-col"
      @click.stop
    >
      <div class="p-3 border-b border-surface-border flex items-center gap-3 bg-surface-card/40">
        <Search class="w-4 h-4 text-brand-400" />
        <input
          ref="inputRef"
          v-model="query"
          type="text"
          placeholder="输入指令、直连主机、快捷动作..."
          class="w-full bg-transparent border-none text-xs text-white placeholder-slate-500 focus:outline-none font-mono"
        />
        <kbd class="text-[10px] bg-surface-highlight text-slate-400 px-1.5 py-0.5 rounded border border-surface-border">ESC</kbd>
      </div>

      <div class="p-2 max-h-80 overflow-y-auto space-y-1 custom-scrollbar">
        <!-- 主机直连 -->
        <div class="text-[10px] font-mono text-slate-500 uppercase px-2 py-1">快速直连服务器</div>
        <div
          v-for="h in filteredHosts"
          :key="h.id"
          @click="$emit('select-host', h); $emit('close')"
          class="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-brand-500/10 text-xs text-slate-200 hover:text-brand-300 cursor-pointer"
        >
          <span class="flex items-center gap-2">
            <Terminal class="w-3.5 h-3.5 text-emerald-400" />
            <span>直连 {{ h.name }} ({{ h.host }})</span>
          </span>
          <span class="text-[10px] font-mono text-slate-500">SSH 终端</span>
        </div>

        <!-- 系统快捷命令 -->
        <div class="text-[10px] font-mono text-slate-500 uppercase px-2 py-1 mt-2">系统动作</div>
        <div
          @click="$emit('action', 'new-host'); $emit('close')"
          class="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-surface-elevated text-xs text-slate-200 cursor-pointer"
        >
          <span class="flex items-center gap-2">
            <PlusCircle class="w-3.5 h-3.5 text-brand-400" />
            <span>新建 SSH 服务器配置</span>
          </span>
          <kbd class="text-[10px] font-mono text-slate-500">Ctrl + N</kbd>
        </div>

        <div
          @click="$emit('action', 'toggle-transfers'); $emit('close')"
          class="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-surface-elevated text-xs text-slate-200 cursor-pointer"
        >
          <span class="flex items-center gap-2">
            <ArrowUpDown class="w-3.5 h-3.5 text-cyan-400" />
            <span>查看传输队列任务</span>
          </span>
          <kbd class="text-[10px] font-mono text-slate-500">Ctrl + T</kbd>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { Search, Terminal, PlusCircle, ArrowUpDown } from 'lucide-vue-next'
import type { HostItem } from '@/types'

const props = defineProps<{
  open: boolean
  hosts: HostItem[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'select-host', host: HostItem): void
  (e: 'action', act: string): void
}>()

const query = ref('')
const inputRef = ref<HTMLInputElement | null>(null)

const filteredHosts = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return props.hosts
  return props.hosts.filter(h => h.name.toLowerCase().includes(q) || h.host.toLowerCase().includes(q))
})

watch(() => props.open, (val) => {
  if (val) {
    query.value = ''
    nextTick(() => {
      inputRef.value?.focus()
    })
  }
})

// 面板上标注了 ESC，这里必须真的能用
const onKeydown = (e: KeyboardEvent) => {
  if (!props.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('close')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>
