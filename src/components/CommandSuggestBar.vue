<template>
  <div
    v-if="visible && candidates.length > 0"
    class="absolute bottom-1.5 left-3 right-3 z-20 flex items-center gap-2 px-2 py-1 rounded-lg border border-surface-border bg-surface-sidebar/95 backdrop-blur-xl shadow-2xl overflow-x-auto no-scrollbar"
  >
    <span class="text-[10px] font-mono text-slate-500 uppercase shrink-0 select-none">历史补全</span>
    <button
      v-for="(candidate, index) in candidates"
      :key="candidate"
      @mousedown.prevent="$emit('accept', candidate)"
      @mouseenter="$emit('hover', index)"
      class="px-2 py-0.5 rounded text-[11px] font-mono shrink-0 transition-colors border"
      :class="index === activeIndex
        ? 'bg-brand-500/20 text-brand-200 border-brand-500/40'
        : 'text-slate-300 border-transparent hover:bg-surface-elevated'"
    >
      {{ candidate }}
    </button>
    <span class="ml-auto text-[10px] font-mono text-slate-500 shrink-0 hidden sm:inline select-none">
      Tab 接受 · ↑↓ 切换 · Esc 关闭
    </span>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    /** 候选命令列表（已按前缀/频次排序，最多 8 条） */
    candidates: string[]
    /** 当前高亮的候选下标 */
    activeIndex: number
    /** 是否展示候选条 */
    visible: boolean
  }>(),
  {
    candidates: () => [],
    activeIndex: 0,
    visible: false
  }
)

defineEmits<{
  (e: 'accept', cmd: string): void
  (e: 'hover', index: number): void
}>()
</script>
