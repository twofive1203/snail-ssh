<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden bg-[#0c0e14]">
    <TerminalView
      transport="local"
      :tabs="session.terminalTabs"
      :active-tab-index="session.activeTerminalIndex"
      :active="isActive"
      @switch-tab="idx => emit('switch-terminal-tab', idx)"
      @close-tab="idx => emit('close-terminal-tab', idx)"
      @add-tab="emit('add-terminal-tab')"
      @duplicate-tab="() => emit('add-terminal-tab')"
      @reconnect-tab="idx => emit('reconnect-terminal-tab', idx)"
      @rename-tab="idx => emit('rename-terminal-tab', idx)"
      @close-other-tabs="idx => emit('close-other-terminal-tabs', idx)"
      @close-tabs-after="idx => emit('close-tabs-after', idx)"
      @session-closed="(tabId, reason) => emit('session-closed', tabId, reason)"
      @toast="(msg, type) => emit('toast', msg, type)"
    />
  </section>
</template>

<script setup lang="ts">
import TerminalView from '@/components/TerminalView.vue'
import type { LocalWorkspace } from '@/types'

defineProps<{
  session: LocalWorkspace
  isActive?: boolean
}>()

const emit = defineEmits<{
  (e: 'switch-terminal-tab', index: number): void
  (e: 'close-terminal-tab', index: number): void
  (e: 'add-terminal-tab'): void
  (e: 'reconnect-terminal-tab', index: number): void
  (e: 'rename-terminal-tab', index: number): void
  (e: 'close-other-terminal-tabs', index: number): void
  (e: 'close-tabs-after', index: number): void
  (e: 'session-closed', tabId: string, reason: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()
</script>
