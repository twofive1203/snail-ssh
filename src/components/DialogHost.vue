<template>
  <div
    v-if="dialogState.open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm z-[60] flex items-center justify-center p-6"
    @click.self="resolveDialog(false)"
  >
    <div
      class="w-full max-w-md bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden animate-dialog-in"
      @click.stop
    >
      <!-- 标题 -->
      <div class="px-5 py-3.5 border-b border-surface-border flex items-center gap-2.5 bg-surface-card/30">
        <component :is="iconComponent" class="w-4 h-4 shrink-0" :class="iconClass" />
        <h2 class="text-sm font-bold text-white">{{ dialogState.title }}</h2>
      </div>

      <!-- 正文 -->
      <div class="px-5 py-4 space-y-3">
        <p class="text-xs text-slate-200 leading-relaxed whitespace-pre-wrap break-words">{{ dialogState.message }}</p>

        <pre
          v-if="dialogState.detail"
          class="text-[11px] font-mono text-surface-subtle bg-surface-base border border-surface-border rounded-lg p-2.5 max-h-40 overflow-y-auto custom-scrollbar whitespace-pre-wrap break-all"
        >{{ dialogState.detail }}</pre>

        <div v-if="dialogState.mode === 'prompt' && dialogState.input" class="space-y-1.5">
          <label v-if="dialogState.input.label" class="text-[11px] font-medium text-slate-300">
            {{ dialogState.input.label }}
          </label>
          <input
            ref="inputRef"
            v-model="dialogState.value"
            type="text"
            :placeholder="dialogState.input.placeholder"
            class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            @keydown.enter.prevent="handleConfirm"
          />
        </div>
      </div>

      <!-- 按键 -->
      <div class="px-5 py-3 border-t border-surface-border flex items-center justify-end gap-2 bg-surface-card/20">
        <button
          v-if="!dialogState.hideCancel"
          type="button"
          @click="resolveDialog(false)"
          class="px-3.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-300 hover:text-white transition-colors"
        >
          {{ dialogState.cancelText }}
        </button>
        <button
          type="button"
          :disabled="!canConfirm"
          @click="handleConfirm"
          class="px-4 py-1.5 rounded-lg text-xs font-semibold transition-all disabled:opacity-40 disabled:cursor-not-allowed"
          :class="dialogState.variant === 'danger'
            ? 'bg-rose-500 hover:bg-rose-600 text-white'
            : dialogState.variant === 'warning'
              ? 'bg-amber-500 hover:bg-amber-600 text-slate-950'
              : 'bg-brand-500 hover:bg-brand-600 text-slate-950 shadow-glow-emerald'"
        >
          {{ dialogState.confirmText }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, HelpCircle, ShieldAlert } from 'lucide-vue-next'
import { dialogState, resolveDialog } from '@/composables/useDialog'

const inputRef = ref<HTMLInputElement | null>(null)

const canConfirm = computed(() => {
  if (dialogState.mode !== 'prompt' || !dialogState.input?.required) return true
  return dialogState.value.trim().length > 0
})

const iconComponent = computed(() => {
  if (dialogState.variant === 'danger') return ShieldAlert
  if (dialogState.variant === 'warning') return AlertTriangle
  return HelpCircle
})

const iconClass = computed(() => {
  if (dialogState.variant === 'danger') return 'text-rose-400'
  if (dialogState.variant === 'warning') return 'text-amber-400'
  return 'text-brand-400'
})

const handleConfirm = () => {
  if (!canConfirm.value) return
  resolveDialog(true)
}

// 打开输入框时自动聚焦并选中已有内容，减少一次手动点击
watch(
  () => dialogState.open,
  async (open) => {
    if (!open) return
    if (dialogState.mode === 'prompt' && dialogState.input) {
      await nextTick()
      inputRef.value?.focus()
      inputRef.value?.select()
    }
  }
)

const onKeydown = (e: KeyboardEvent) => {
  if (!dialogState.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    resolveDialog(false)
  } else if (e.key === 'Enter' && dialogState.mode === 'prompt' && document.activeElement !== inputRef.value) {
    e.preventDefault()
    handleConfirm()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
@keyframes dialog-in {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
.animate-dialog-in {
  animation: dialog-in 0.16s cubic-bezier(0.16, 1, 0.3, 1);
}
</style>
