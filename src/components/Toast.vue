<template>
  <!-- 定位在标题栏下方，避免遮挡右上角的传输状态与窗口按钮 -->
  <div class="fixed top-14 right-4 z-50 flex flex-col gap-2 pointer-events-none max-w-sm">
    <transition-group name="toast">
      <div
        v-for="item in messages"
        :key="item.id"
        class="flex items-start gap-2.5 px-3.5 py-2.5 rounded-xl border shadow-xl text-xs backdrop-blur-md font-medium pointer-events-auto"
        :class="getToastClasses(item.type)"
      >
        <component :is="getIcon(item.type)" class="w-4 h-4 shrink-0 mt-0.5" />
        <span class="flex-1 font-sans break-words">{{ item.text }}</span>
        <button
          @click="$emit('dismiss', item.id)"
          class="p-0.5 rounded hover:bg-white/10 opacity-60 hover:opacity-100 transition-opacity shrink-0"
          title="关闭提示"
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    </transition-group>
  </div>
</template>

<script setup lang="ts">
import type { ToastMessage } from '@/types'
import { CheckCircle2, Info, AlertTriangle, AlertOctagon, X } from 'lucide-vue-next'

defineProps<{
  messages: ToastMessage[]
}>()

defineEmits<{
  (e: 'dismiss', id: string): void
}>()

const getToastClasses = (type: string) => {
  switch (type) {
    case 'success':
      return 'border-emerald-500/40 text-emerald-300 bg-[#11241f]/95'
    case 'warning':
      return 'border-amber-500/40 text-amber-300 bg-[#2b2214]/95'
    case 'error':
      return 'border-rose-500/40 text-rose-300 bg-[#2b161a]/95'
    default:
      return 'border-blue-500/40 text-blue-300 bg-[#161d2d]/95'
  }
}

const getIcon = (type: string) => {
  switch (type) {
    case 'success':
      return CheckCircle2
    case 'warning':
      return AlertTriangle
    case 'error':
      return AlertOctagon
    default:
      return Info
  }
}
</script>

<style scoped>
.toast-enter-active,
.toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(-8px) scale(0.95);
}
.toast-leave-to {
  opacity: 0;
  transform: translateY(-8px) scale(0.95);
}
</style>
