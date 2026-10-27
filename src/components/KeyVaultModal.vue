<template>
  <div v-if="open" class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="$emit('close')">
    <div class="w-full max-w-2xl bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[80vh]">
      <div class="px-5 py-3.5 border-b border-surface-border flex items-center justify-between">
        <div>
          <h2 class="text-sm font-bold text-white">SSH 密钥保险箱</h2>
          <p class="text-[11px] text-surface-subtle">生成或导入的私钥保存在 ~/.snail-ssh/keys，主机配置只引用路径</p>
        </div>
        <button class="text-slate-400 hover:text-white" @click="$emit('close')">关闭</button>
      </div>
      <div class="px-5 pt-3 flex gap-2 text-xs border-b border-surface-border">
        <button type="button" class="px-3 py-2 border-b-2" :class="tab === 'keys' ? 'border-brand-500 text-brand-400' : 'border-transparent text-slate-500'" @click="tab = 'keys'">密钥</button>
        <button type="button" class="px-3 py-2 border-b-2" :class="tab === 'known' ? 'border-brand-500 text-brand-400' : 'border-transparent text-slate-500'" @click="tab = 'known'; loadKnown()">known_hosts</button>
      </div>
      <div class="p-5 overflow-y-auto custom-scrollbar space-y-3">
        <template v-if="tab === 'keys'">
          <div class="grid grid-cols-4 gap-2">
            <input v-model="name" placeholder="密钥名称" class="col-span-2 bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
            <select v-model="algorithm" class="bg-surface-card border border-surface-border rounded-lg px-2 py-2 text-xs text-white">
              <option value="ed25519">ed25519</option>
              <option value="rsa">rsa</option>
            </select>
            <button type="button" class="rounded-lg bg-brand-500 text-slate-950 text-xs font-semibold" @click="generate">生成</button>
          </div>
          <textarea v-model="importMaterial" rows="3" placeholder="导入：粘贴私钥，或填写本地路径" class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono"></textarea>
          <div class="flex gap-2">
            <input v-model="passphrase" type="password" placeholder="口令（可选）" class="flex-1 bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
            <button type="button" class="px-3 rounded-lg border border-surface-border text-xs text-slate-200" @click="importKey">导入</button>
          </div>
          <div v-if="freshPublic" class="text-[11px] font-mono text-emerald-300 break-all bg-surface-card border border-surface-border rounded-lg p-2">
            公钥（请自行写入远端 authorized_keys）：{{ freshPublic }}
          </div>
          <div v-for="key in keys" :key="key.id" class="border border-surface-border rounded-lg p-3 bg-surface-card/40">
            <div class="flex items-center justify-between gap-2">
              <div class="min-w-0">
                <div class="text-xs text-white">{{ key.name }} <span class="text-slate-500 font-mono">{{ key.algorithm }}</span></div>
                <div class="text-[10px] font-mono text-slate-500 truncate">{{ key.fingerprint }}</div>
                <div class="text-[10px] font-mono text-slate-600 truncate">{{ key.path }}</div>
              </div>
              <div class="flex gap-1 shrink-0">
                <button type="button" class="px-2 py-1 text-[11px] rounded border border-surface-border text-slate-300" @click="copy(key.publicKey)">复制公钥</button>
                <button type="button" class="px-2 py-1 text-[11px] rounded border border-surface-border text-slate-300" @click="$emit('use-key', key.path)">用于主机</button>
                <button type="button" class="px-2 py-1 text-[11px] rounded border border-rose-500/40 text-rose-300" @click="remove(key.id)">删除</button>
              </div>
            </div>
          </div>
          <p v-if="keys.length === 0" class="text-xs text-slate-500">还没有保存在保险箱里的密钥。已有的 ~/.ssh/id_ed25519 可以直接在主机里填路径，不必导入。</p>
        </template>
        <template v-else>
          <div v-for="entry in known" :key="entry.hosts + entry.fingerprint" class="border border-surface-border rounded-lg p-3 flex items-center justify-between gap-3">
            <div class="min-w-0">
              <div class="text-xs text-white font-mono truncate">{{ entry.hosts }}</div>
              <div class="text-[10px] text-slate-500 font-mono truncate">{{ entry.keyType }} {{ entry.fingerprint }}</div>
            </div>
            <button type="button" class="text-[11px] text-rose-300" @click="removeKnown(entry)">删除</button>
          </div>
          <p v-if="known.length === 0" class="text-xs text-slate-500">~/.ssh/known_hosts 里没有可显示的记录。</p>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import type { KnownHostEntry, SshKeyInfo } from '@/types'
import { deleteKnownHostApi, deleteSshKeyApi, generateSshKeyApi, importSshKeyApi, listKnownHostsApi, listSshKeysApi } from '@/api/tauri'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
  (e: 'use-key', path: string): void
}>()

const tab = ref<'keys' | 'known'>('keys')
const keys = ref<SshKeyInfo[]>([])
const known = ref<KnownHostEntry[]>([])
const name = ref('新密钥')
const algorithm = ref('ed25519')
const passphrase = ref('')
const importMaterial = ref('')
const freshPublic = ref('')

watch(() => props.open, (open) => { if (open) loadKeys() })

const loadKeys = async () => {
  try { keys.value = await listSshKeysApi() } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const loadKnown = async () => {
  try { known.value = await listKnownHostsApi() } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const generate = async () => {
  try {
    const created = await generateSshKeyApi(name.value, algorithm.value, passphrase.value)
    freshPublic.value = created.publicKey
    emit('toast', `已生成 ${created.name}`, 'success')
    await loadKeys()
  } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const importKey = async () => {
  try {
    await importSshKeyApi(name.value, importMaterial.value, passphrase.value)
    importMaterial.value = ''
    emit('toast', '密钥已导入保险箱', 'success')
    await loadKeys()
  } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const remove = async (id: string) => {
  try { await deleteSshKeyApi(id); await loadKeys() } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const removeKnown = async (entry: KnownHostEntry) => {
  try { await deleteKnownHostApi(entry.hosts, entry.fingerprint); await loadKnown() } catch (e: any) { emit('toast', String(e?.message || e), 'error') }
}
const copy = async (text: string) => {
  await navigator.clipboard.writeText(text)
  emit('toast', '公钥已复制', 'success')
}
</script>
