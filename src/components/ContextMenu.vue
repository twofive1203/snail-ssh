<template>
  <Teleport to="body">
    <Transition name="ctx-menu">
      <div
        v-if="open && items.length > 0"
        class="fixed inset-0 z-[70]"
        @mousedown="emit('close')"
        @contextmenu.prevent="emit('close')"
        @wheel="emit('close')"
      >
        <div
          ref="menuRef"
          class="absolute w-56 max-h-[70vh] overflow-y-auto custom-scrollbar bg-surface-sidebar/95 backdrop-blur-xl border border-surface-border rounded-xl shadow-2xl py-1.5"
          :style="{ left: pos.left + 'px', top: pos.top + 'px' }"
          @mousedown.stop
          @contextmenu.stop.prevent
        >
          <template v-for="(item, i) in items" :key="item.key || 'divider-' + i">
            <div v-if="item.divider" class="my-1 h-px bg-surface-border/70"></div>
            <button
              v-else
              type="button"
              :disabled="item.disabled"
              @click="handleSelect(item)"
              class="w-full flex items-center gap-2.5 px-3 py-1.5 text-xs text-left transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
              :class="item.danger
                ? 'text-rose-400 hover:bg-rose-500/10 hover:text-rose-300 disabled:hover:bg-transparent'
                : 'text-slate-200 hover:bg-surface-elevated hover:text-white disabled:hover:bg-transparent'"
            >
              <component :is="item.icon" v-if="item.icon" class="w-3.5 h-3.5 shrink-0 opacity-80" />
              <span class="truncate">{{ item.label }}</span>
              <span v-if="item.hint" class="ml-auto pl-3 text-[10px] font-mono text-slate-500 truncate max-w-[40%]">{{ item.hint }}</span>
            </button>
          </template>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onBeforeUnmount } from 'vue'
import type { ContextMenuItem } from '@/types'

const props = defineProps<{
  open: boolean
  x: number
  y: number
  items: ContextMenuItem[]
}>()

const emit = defineEmits<{
  (e: 'select', item: ContextMenuItem): void
  (e: 'close'): void
}>()

const menuRef = ref<HTMLElement | null>(null)
// 先按鼠标位置渲染，测出菜单真实尺寸后再收敛到窗口内，避免贴边被裁切
const pos = ref({ left: 0, top: 0 })

const updatePosition = async () => {
  pos.value = { left: props.x, top: props.y }
  await nextTick()
  const el = menuRef.value
  if (!el) return
  const { width, height } = el.getBoundingClientRect()
  const margin = 8
  const maxLeft = window.innerWidth - width - margin
  const maxTop = window.innerHeight - height - margin
  pos.value = {
    left: Math.max(margin, Math.min(props.x, maxLeft)),
    top: Math.max(margin, Math.min(props.y, maxTop))
  }
}

const handleSelect = (item: ContextMenuItem) => {
  if (item.disabled) return
  emit('select', item)
  emit('close')
}

const handleKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') emit('close')
}

watch(() => props.open, (open) => {
  if (open) {
    updatePosition()
    window.addEventListener('keydown', handleKeydown)
  } else {
    window.removeEventListener('keydown', handleKeydown)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})
</script>

<style scoped>
.ctx-menu-enter-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}
.ctx-menu-leave-active {
  transition: opacity 0.08s ease;
}
.ctx-menu-enter-from {
  opacity: 0;
  transform: scale(0.97);
}
.ctx-menu-leave-to {
  opacity: 0;
}
</style>
