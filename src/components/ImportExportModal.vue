<template>
  <div v-if="open" class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="$emit('close')">
    <div class="w-full max-w-3xl bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden max-h-[82vh] flex flex-col">
      <div class="px-5 py-3.5 border-b border-surface-border flex items-center justify-between">
        <div>
          <h2 class="text-sm font-bold text-white">导入 / 导出主机</h2>
          <p class="text-[11px] text-surface-subtle">读取 OpenSSH config。ProxyCommand 不会执行；密码不会写进导出文件。</p>
        </div>
        <button class="text-xs text-slate-400 hover:text-white" @click="$emit('close')">关闭</button>
      </div>
      <div class="px-5 py-3 flex gap-2 border-b border-surface-border">
        <button type="button" class="px-3 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-slate-200" @click="loadDefault">读取 ~/.ssh/config</button>
        <button type="button" class="px-3 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-slate-200" @click="loadPicked">选择文件</button>
        <button type="button" class="px-3 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-slate-200" @click="exportConfig">导出为 ssh config</button>
      </div>
      <div class="p-5 overflow-y-auto custom-scrollbar space-y-2">
        <p v-if="source" class="text-[11px] font-mono text-slate-500">来源：{{ source }}</p>
        <p v-for="warning in warnings" :key="warning" class="text-[11px] text-amber-300">{{ warning }}</p>
        <label v-for="host in preview" :key="host.id" class="flex gap-3 border border-surface-border rounded-lg p-3 cursor-pointer">
          <input type="checkbox" :checked="selected.has(host.id)" @change="toggle(host.id)" />
          <div class="min-w-0">
            <div class="text-xs text-white">{{ host.name }} <span class="text-slate-500 font-mono">{{ host.username }}@{{ host.host }}:{{ host.port }}</span></div>
            <div class="text-[10px] text-slate-500 font-mono">
              {{ host.authType }}
              <span v-if="host.jumpHostIds?.length"> · 跳板 {{ host.jumpHostIds.length }} 跳</span>
              <span v-if="host.forwards?.length"> · 转发 {{ host.forwards.length }} 条</span>
              <span v-if="exists(host.id)"> · 将覆盖同 ID 主机</span>
            </div>
          </div>
        </label>
        <p v-if="preview.length === 0" class="text-xs text-slate-500">还没有预览。先读取 ~/.ssh/config 或选择一个配置文件。</p>
      </div>
      <div class="px-5 py-3 border-t border-surface-border flex justify-end">
        <button type="button" class="px-4 py-1.5 rounded-lg bg-brand-500 text-xs font-semibold text-slate-950 disabled:opacity-40" :disabled="selected.size === 0" @click="commit">
          导入选中的 {{ selected.size }} 台
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import type { HostItem } from '@/types'
import { exportHostsSshConfigApi, previewSshConfigApi, saveHostApi } from '@/api/tauri'

const props = defineProps<{ open: boolean; hosts: HostItem[] }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'imported'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const preview = ref<HostItem[]>([])
const warnings = ref<string[]>([])
const source = ref('')
const selected = ref(new Set<string>())

const exists = (id: string) => props.hosts.some(host => host.id === id)
const toggle = (id: string) => {
  const next = new Set(selected.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  selected.value = next
}
const applyPreview = (result: { hosts: HostItem[]; warnings: string[]; sourcePath: string }) => {
  preview.value = result.hosts
  warnings.value = result.warnings
  source.value = result.sourcePath
  selected.value = new Set(result.hosts.map(host => host.id))
}
const loadDefault = async () => {
  try { applyPreview(await previewSshConfigApi()) } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const loadPicked = async () => {
  const picked = await openDialog({ multiple: false, filters: [{ name: 'SSH config', extensions: ['config', 'txt', '*'] }] })
  if (typeof picked !== 'string') return
  try { applyPreview(await previewSshConfigApi(picked)) } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const commit = async () => {
  const chosen = preview.value.filter(host => selected.value.has(host.id))
  try {
    for (const host of chosen) await saveHostApi(host)
    emit('toast', `已导入 ${chosen.length} 台主机`, 'success')
    emit('imported')
    emit('close')
  } catch (e: any) {
    emit('toast', String(e?.message || e), 'error')
  }
}
const exportConfig = async () => {
  const path = await saveDialog({ defaultPath: 'snail-ssh-config', filters: [{ name: 'SSH config', extensions: ['config'] }] })
  if (typeof path !== 'string') return
  try {
    await exportHostsSshConfigApi(path)
    emit('toast', `已导出到 ${path}`, 'success')
  } catch (e: any) {
    emit('toast', String(e?.message || e), 'error')
  }
}
</script>
