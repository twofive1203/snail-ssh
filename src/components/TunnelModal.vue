<template>
  <div v-if="open" class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="$emit('close')">
    <div class="w-full max-w-2xl bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden max-h-[80vh] flex flex-col">
      <div class="px-5 py-3.5 border-b border-surface-border flex items-center justify-between">
        <div>
          <h2 class="text-sm font-bold text-white">端口转发 & 隧道</h2>
          <p class="text-[11px] text-surface-subtle">已启用的转发会在连接该主机时自动启动。这里可以不打开终端，单独把隧道拉起来。</p>
        </div>
        <button class="text-slate-400 hover:text-white text-xs" @click="$emit('close')">关闭</button>
      </div>
      <div class="p-5 overflow-y-auto custom-scrollbar space-y-3">
        <div v-if="active.length" class="space-y-2">
          <div class="text-[11px] uppercase tracking-wider text-slate-500 font-mono">独立隧道</div>
          <div v-for="item in active" :key="item.id" class="border border-emerald-500/30 rounded-lg p-3 bg-emerald-500/5">
            <div class="flex items-center justify-between">
              <div class="text-xs text-white font-mono">{{ item.label }}</div>
              <button type="button" class="text-[11px] text-rose-300" @click="stop(item.id)">停止</button>
            </div>
            <div v-for="note in item.notes" :key="note" class="text-[10px] text-slate-400 font-mono mt-1">{{ note }}</div>
          </div>
        </div>
        <div v-for="host in hostsWithForwards" :key="host.id" class="border border-surface-border rounded-lg p-3">
          <div class="flex items-center justify-between gap-2">
            <div>
              <div class="text-xs text-white">{{ host.name }}</div>
              <div class="text-[10px] font-mono text-slate-500">{{ host.username }}@{{ host.host }}:{{ host.port }}</div>
            </div>
            <button type="button" class="px-2.5 py-1 rounded border border-surface-border text-[11px] text-slate-200" :disabled="starting === host.id" @click="start(host)">
              {{ starting === host.id ? '启动中...' : '启动独立隧道' }}
            </button>
          </div>
          <div v-for="forward in host.forwards || []" :key="forward.id" class="mt-2 text-[11px] font-mono text-slate-400">
            <span :class="forward.enabled ? 'text-emerald-400' : 'text-slate-600'">{{ forward.enabled ? '启用' : '关闭' }}</span>
            {{ labelOf(forward) }}
          </div>
        </div>
        <p v-if="hostsWithForwards.length === 0" class="text-xs text-slate-500">还没有配置转发。在主机编辑里的「端口转发」页添加。</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { HostItem, PortForward, TrustedEndpoint, TunnelStatus } from '@/types'
import { listTunnelsApi, startTunnelApi, stopTunnelApi } from '@/api/tauri'
import { confirmDialog } from '@/composables/useDialog'
import { parseHostKeyError } from '@/utils/hostKey'

const props = defineProps<{ open: boolean; hosts: HostItem[] }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const active = ref<TunnelStatus[]>([])
const starting = ref('')
const hostsWithForwards = computed(() => props.hosts.filter(host => (host.forwards || []).length > 0))

watch(() => props.open, (open) => { if (open) refresh() })

const refresh = async () => {
  try { active.value = await listTunnelsApi() } catch { active.value = [] }
}
const start = async (host: HostItem) => {
  starting.value = host.id
  const trusted: TrustedEndpoint[] = []
  try {
    for (let attempt = 0; attempt < 8; attempt++) {
      try {
        const status = await startTunnelApi(host, props.hosts, trusted)
        const failed = status.notes.some(note => note.includes('失败'))
        emit('toast', status.notes.join('；') || '隧道已启动', failed ? 'warning' : 'success')
        await refresh()
        return
      } catch (e: any) {
        const challenge = parseHostKeyError(String(e?.message || e))
        if (!challenge || !challenge.host || challenge.port == null) throw e
        const proceed = await confirmDialog({
          title: '确认跳板或目标主机密钥',
          variant: 'warning',
          message: `是否信任 ${challenge.host}:${challenge.port} 的主机密钥？`,
          detail: challenge.fingerprint,
          confirmText: '信任并继续',
          cancelText: '取消'
        })
        if (!proceed) return
        trusted.push({ host: challenge.host, port: challenge.port })
      }
    }
  } catch (e: any) {
    emit('toast', String(e?.message || e), 'error')
  } finally {
    starting.value = ''
  }
}
const stop = async (id: string) => {
  try { await stopTunnelApi(id); await refresh() } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const labelOf = (forward: PortForward) => {
  if (forward.kind === 'dynamic') return `SOCKS5 ${forward.bindHost}:${forward.bindPort}`
  const arrow = forward.kind === 'remote' ? '远端监听' : '本地监听'
  return `${arrow} ${forward.bindHost}:${forward.bindPort} → ${forward.targetHost}:${forward.targetPort}`
}
</script>
