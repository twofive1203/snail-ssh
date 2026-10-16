<template>
  <div class="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
    <transition-group name="toast">
      <div
        v-for="item in messages"
        :key="item.id"
        class="flex items-center gap-2.5 px-3.5 py-2.5 rounded-xl border shadow-xl text-xs backdrop-blur-md font-medium pointer-events-auto"
        :class="getToastClasses(item.type)"
      >
        <component :is="getIcon(item.type)" class="w-4 h-4 shrink-0" />
        <span class="flex-1 font-sans">{{ item.text }}</span>
      </div>
    </transition-group>
  </div>
</template>

<script setup lang="ts">
import type { ToastMessage } from '@/types'
import { CheckCircle2, Info, AlertTriangle, AlertOctagon } from 'lucide-vue-next'

defineProps<{
  messages: ToastMessage[]
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
