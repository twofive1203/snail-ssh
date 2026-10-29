<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4 transition-opacity"
  >
    <div
      class="w-full max-w-xl bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden flex flex-col"
      @click.stop
    >
      <!-- 弹窗顶栏 -->
      <div class="px-6 py-4 border-b border-surface-border flex items-center justify-between bg-surface-card/30">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-lg bg-brand-500/10 border border-brand-500/20 flex items-center justify-center text-brand-400">
            <Server class="w-4 h-4" />
          </div>
          <div>
            <h2 class="text-sm font-bold text-white">{{ isQuick ? '快速连接' : isEditing ? '编辑 SSH 服务器' : '配置新建服务器' }}</h2>
            <p class="text-[11px] text-surface-subtle">
              {{ isQuick ? '临时会话不会保存到资产列表，密码 / 私钥仅保存在内存中' : '填写连接凭证与认证方式，密码/私钥会存入系统钥匙串' }}
            </p>
          </div>
        </div>
        <button @click="$emit('close')" class="p-1.5 rounded-lg hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors">
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- 表单内容 -->
      <form @submit.prevent="handleSubmit" class="p-6 space-y-4 max-h-[75vh] overflow-y-auto custom-scrollbar">
        <!-- 标签页 -->
        <div class="flex border-b border-surface-border text-xs mb-3">
          <button type="button" class="px-4 py-2 border-b-2" :class="tab === 'general' ? 'border-brand-500 text-brand-400 font-medium' : 'border-transparent text-slate-500'" @click="tab = 'general'">常规设置</button>
          <button type="button" class="px-4 py-2 border-b-2" :class="tab === 'jump' ? 'border-brand-500 text-brand-400 font-medium' : 'border-transparent text-slate-500'" @click="tab = 'jump'">跳板机 & 代理</button>
          <button type="button" class="px-4 py-2 border-b-2" :class="tab === 'forward' ? 'border-brand-500 text-brand-400 font-medium' : 'border-transparent text-slate-500'" @click="tab = 'forward'">端口转发</button>
        </div>

        <div v-show="tab === 'general'" class="space-y-4">
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5" :class="isQuick ? 'col-span-2' : 'col-span-2 sm:col-span-1'">
            <label class="text-xs font-medium text-slate-300">服务器别名 (Name) <span class="text-rose-400">*</span></label>
            <input
              v-model="form.name"
              type="text"
              required
              placeholder="如: prod-k8s-master-01"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>
          <div v-if="!isQuick" class="space-y-1.5 col-span-2 sm:col-span-1">
            <label class="text-xs font-medium text-slate-300">所属分组 (Group)</label>
            <select
              v-model="form.group"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            >
              <option v-for="g in settings.groups" :key="g.id" :value="g.id">{{ g.label }}</option>
              <!-- 旧配置里的分组已不在设置中心时，保留原值避免保存时被静默改写 -->
              <option v-if="legacyGroup" :value="form.group">{{ form.group }}（未登记）</option>
            </select>
          </div>
        </div>

        <div class="grid grid-cols-3 gap-4">
          <div class="col-span-2 space-y-1.5">
            <label class="text-xs font-medium text-slate-300">主机地址 / IP (Host) <span class="text-rose-400">*</span></label>
            <input
              v-model="form.host"
              type="text"
              required
              placeholder="192.168.1.1 或 domain.com"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>
          <div class="col-span-1 space-y-1.5">
            <label class="text-xs font-medium text-slate-300">SSH 端口</label>
            <input
              v-model.number="form.port"
              type="number"
              required
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>
        </div>

        <!-- 认证方式 -->
        <div class="space-y-3 pt-2">
          <label class="text-xs font-medium text-slate-300 block">身份认证方式 (Authentication)</label>
          <div class="grid grid-cols-2 gap-2">
            <label
              class="flex items-center gap-2 p-2.5 rounded-lg bg-surface-card border hover:border-brand-500/50 cursor-pointer"
              :class="form.authType === 'password' ? 'border-brand-500 text-brand-400' : 'border-surface-border text-slate-200'"
            >
              <input type="radio" v-model="form.authType" value="password" class="text-brand-500 focus:ring-0" />
              <span class="text-xs">密码认证</span>
            </label>
            <label
              class="flex items-center gap-2 p-2.5 rounded-lg bg-surface-card border hover:border-brand-500/50 cursor-pointer"
              :class="form.authType === 'key' ? 'border-brand-500 text-brand-400' : 'border-surface-border text-slate-200'"
            >
              <input type="radio" v-model="form.authType" value="key" class="text-brand-500 focus:ring-0" />
              <span class="text-xs">私钥 (Key)</span>
            </label>
            <label
              class="flex items-center gap-2 p-2.5 rounded-lg bg-surface-card border hover:border-brand-500/50 cursor-pointer"
              :class="form.authType === 'agent' ? 'border-brand-500 text-brand-400' : 'border-surface-border text-slate-200'"
            >
              <input type="radio" v-model="form.authType" value="agent" class="text-brand-500 focus:ring-0" />
              <span class="text-xs">SSH Agent</span>
            </label>
            <label
              class="flex items-center gap-2 p-2.5 rounded-lg bg-surface-card border hover:border-brand-500/50 cursor-pointer"
              :class="form.authType === 'keyboard' ? 'border-brand-500 text-brand-400' : 'border-surface-border text-slate-200'"
            >
              <input type="radio" v-model="form.authType" value="keyboard" class="text-brand-500 focus:ring-0" />
              <span class="text-xs">交互式 / OTP</span>
            </label>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">登录用户名 (Username) <span class="text-rose-400">*</span></label>
            <input
              v-model="form.username"
              type="text"
              required
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>

          <div v-if="form.authType === 'password'" class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">登录密码 (Password)</label>
            <input
              v-model="form.password"
              type="password"
              placeholder="••••••••••••"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>

          <div v-if="form.authType === 'key'" class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">私钥内容或本地路径 (Private Key PEM / Path)</label>
            <textarea
              v-model="form.privateKey"
              rows="3"
              placeholder="粘贴 -----BEGIN OPENSSH PRIVATE KEY-----，或直接填写本地私钥文件绝对路径"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            ></textarea>
          </div>

          <div v-if="form.authType === 'key'" class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">私钥口令 (Passphrase，可选)</label>
            <input
              v-model="form.passphrase"
              type="password"
              placeholder="私钥无口令时留空"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>
        </div>

        <!-- 标签设置（快速连接不需要归类） -->
        <div v-if="!isQuick" class="space-y-1.5">
          <label class="text-xs font-medium text-slate-300">分类标签 (英文逗号分隔)</label>
          <input
            v-model="tagInput"
            type="text"
            placeholder="如: Ubuntu, Web, Aliyun"
            class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
          />
        </div>
        </div>

        <div v-show="tab === 'jump'" class="space-y-4">
          <p class="text-[11px] text-slate-500">按顺序经过已保存的主机。每一跳用它自己的凭证，确认指纹时只信任正在询问的那一跳。不执行 ProxyCommand。</p>
          <div class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">添加跳板</label>
            <select class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" @change="addJump(($event.target as HTMLSelectElement).value); ($event.target as HTMLSelectElement).value = ''">
              <option value="">选择一台已保存的主机</option>
              <option v-for="candidate in jumpCandidates" :key="candidate.id" :value="candidate.id">{{ candidate.name }} · {{ candidate.username }}@{{ candidate.host }}</option>
            </select>
          </div>
          <div v-for="(id, index) in form.jumpHostIds" :key="id + index" class="flex items-center justify-between gap-2 border border-surface-border rounded-lg px-3 py-2">
            <span class="text-xs font-mono text-slate-200 truncate">{{ jumpLabel(id) }}</span>
            <span class="flex gap-1 shrink-0">
              <button type="button" class="text-[11px] text-slate-400" @click="moveJump(index, -1)">上移</button>
              <button type="button" class="text-[11px] text-slate-400" @click="moveJump(index, 1)">下移</button>
              <button type="button" class="text-[11px] text-rose-300" @click="form.jumpHostIds.splice(index, 1)">移除</button>
            </span>
          </div>
          <div class="grid grid-cols-4 gap-2">
            <select v-model="form.proxy.kind" class="bg-surface-card border border-surface-border rounded-lg px-2 py-2 text-xs text-white">
              <option value="">无代理</option>
              <option value="socks5">SOCKS5</option>
              <option value="http">HTTP</option>
            </select>
            <input v-model="form.proxy.host" placeholder="代理地址" class="col-span-2 bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
            <input v-model.number="form.proxy.port" type="number" placeholder="端口" class="bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
            <input v-model="form.proxy.username" placeholder="代理用户（可选）" class="col-span-2 bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
            <input v-model="form.proxy.password" type="password" placeholder="代理密码" class="col-span-2 bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white font-mono" />
          </div>
        </div>

        <div v-show="tab === 'forward'" class="space-y-3">
          <p class="text-[11px] text-slate-500">启用的转发会在连接这台主机时自动启动，也可以从侧边栏单独拉起隧道。动态转发只能监听 127.0.0.1、localhost 或 ::1。</p>
          <div v-for="(forward, index) in form.forwards" :key="forward.id" class="grid grid-cols-6 gap-2 items-center">
            <select v-model="forward.kind" class="bg-surface-card border border-surface-border rounded px-1 py-1.5 text-[11px] text-white">
              <option value="local">本地</option>
              <option value="remote">远端</option>
              <option value="dynamic">动态</option>
            </select>
            <input v-model="forward.bindHost" class="bg-surface-card border border-surface-border rounded px-2 py-1.5 text-[11px] text-white font-mono" />
            <input v-model.number="forward.bindPort" type="number" class="bg-surface-card border border-surface-border rounded px-2 py-1.5 text-[11px] text-white font-mono" />
            <input v-if="forward.kind !== 'dynamic'" v-model="forward.targetHost" placeholder="目标主机" class="bg-surface-card border border-surface-border rounded px-2 py-1.5 text-[11px] text-white font-mono" />
            <input v-if="forward.kind !== 'dynamic'" v-model.number="forward.targetPort" type="number" placeholder="目标端口" class="bg-surface-card border border-surface-border rounded px-2 py-1.5 text-[11px] text-white font-mono" />
            <button type="button" class="text-[11px] text-rose-300" @click="form.forwards.splice(index, 1)">删除</button>
            <label class="col-span-6 text-[11px] text-slate-400 flex items-center gap-2">
              <input type="checkbox" v-model="forward.enabled" /> 连接时启动
            </label>
          </div>
          <button type="button" class="text-xs text-brand-400" @click="addForward">添加转发</button>
        </div>

        <!-- 底部按键 -->
        <div class="pt-4 flex items-center justify-end gap-3 border-t border-surface-border">
          <button
            type="button"
            @click="$emit('close')"
            class="px-4 py-2 rounded-lg bg-surface-card hover:bg-surface-elevated text-xs text-slate-300 hover:text-white transition-colors"
          >
            取消
          </button>
          <button
            type="button"
            @click="testConnect"
            :disabled="testing"
            class="px-4 py-2 rounded-lg bg-surface-elevated border border-surface-border hover:border-brand-500/50 text-xs text-brand-400 transition-colors disabled:opacity-50"
          >
            {{ testing ? '测试中...' : '测试连通性' }}
          </button>
          <button
            type="submit"
            class="px-5 py-2 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs shadow-glow-emerald transition-all"
          >
            {{ isQuick ? '连接' : '保存配置' }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { Server, X } from 'lucide-vue-next'
import type { HostItem, AuthType, PortForward, ProxyConfig, TrustedEndpoint } from '@/types'
import { testSshConnectionApi, isTauri } from '@/api/tauri'
import { confirmDialog, dialogState } from '@/composables/useDialog'
import { settings } from '@/composables/useAppSettings'
import { parseHostKeyError } from '@/utils/hostKey'

const props = defineProps<{
  open: boolean
  hostData?: HostItem | null
  hosts?: HostItem[]
  /** edit=编辑已有主机，new=新建入库，quick=快速连接（不落盘） */
  mode?: 'edit' | 'new' | 'quick'
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', host: HostItem): void
  (e: 'connect-quick', host: HostItem): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const isEditing = ref(false)
const isQuick = computed(() => props.mode === 'quick')
const tagInput = ref('')
const testing = ref(false)
const tab = ref<'general' | 'jump' | 'forward'>('general')

const emptyProxy = (): ProxyConfig => ({ kind: '', host: '', port: 1080, username: '', password: '' })
const blankForm = (id = '') => ({
  id,
  name: '',
  host: '',
  port: 22,
  username: 'root',
  authType: 'password' as AuthType,
  password: '',
  privateKey: '',
  passphrase: '',
  group: defaultGroupId(),
  jumpHostIds: [] as string[],
  proxy: emptyProxy(),
  forwards: [] as PortForward[],
})

/** 当前选中的分组不在设置里时，下拉里要额外提供一个原值选项 */

/** 新主机的默认分组：取设置里的第一个分组，不硬编码 production */
const defaultGroupId = () => settings.groups[0]?.id || 'production'

const form = ref(blankForm())
const jumpCandidates = computed(() => (props.hosts || []).filter(host => host.id !== form.value.id))
const jumpLabel = (id: string) => {
  const host = (props.hosts || []).find(item => item.id === id)
  return host ? `${host.name} · ${host.username}@${host.host}:${host.port}` : id
}
const addJump = (id: string) => {
  if (!id || form.value.jumpHostIds.includes(id) || id === form.value.id) return
  form.value.jumpHostIds.push(id)
}
const moveJump = (index: number, delta: number) => {
  const next = index + delta
  if (next < 0 || next >= form.value.jumpHostIds.length) return
  const list = form.value.jumpHostIds
  const [item] = list.splice(index, 1)
  list.splice(next, 0, item)
}
const addForward = () => {
  form.value.forwards.push({
    id: `fwd-${Date.now()}`,
    kind: 'local',
    bindHost: '127.0.0.1',
    bindPort: 0,
    targetHost: '127.0.0.1',
    targetPort: 0,
    enabled: true,
  })
}

const legacyGroup = computed(() =>
  !!form.value.group && !settings.groups.some(g => g.id === form.value.group)
)

const fillForm = (host?: HostItem | null, id = '') => {
  form.value = {
    ...blankForm(id || host?.id || ''),
    ...host,
    id: id || host?.id || blankForm().id,
    jumpHostIds: [...(host?.jumpHostIds || [])],
    proxy: { ...emptyProxy(), ...(host?.proxy || {}) },
    forwards: (host?.forwards || []).map(item => ({ ...item })),
  }
}

watch(() => props.open, (val) => {
  if (!val) return
  tab.value = 'general'
  if (isQuick.value) {
    isEditing.value = false
    fillForm(null, 'quick-' + Date.now())
    tagInput.value = ''
    return
  }
  if (props.hostData) {
    isEditing.value = true
    fillForm(props.hostData)
    tagInput.value = props.hostData.tags.join(', ')
  } else {
    isEditing.value = false
    fillForm(null, 'host-' + Date.now())
    tagInput.value = 'Ubuntu, prod'
  }
})

// 构造后端测试所需的连接配置
const buildConfig = (): HostItem => ({
  ...form.value,
  proxy: form.value.proxy.kind && form.value.proxy.host ? form.value.proxy : null,
  tags: []
})

const errorText = (e: unknown): string => String((e as any)?.message || e)

const testConnect = async () => {
  if (!form.value.host.trim()) {
    emit('toast', '请先填写主机地址', 'warning')
    return
  }
  if (!isTauri()) {
    emit('toast', '浏览器预览模式无法测试真实连接，请使用 Tauri 桌面客户端', 'info')
    return
  }

  testing.value = true
  const trusted: TrustedEndpoint[] = []
  try {
    for (let attempt = 0; attempt < 8; attempt++) {
      try {
        const res = await testSshConnectionApi(buildConfig(), trusted, props.hosts || [])
        emit('toast', `连接成功：TCP+握手+认证共 ${res.latencyMs} ms，主机密钥 ${res.hostKeyFingerprint}`, 'success')
        return
      } catch (e) {
        const challenge = parseHostKeyError(errorText(e))
        if (!challenge) throw e
        const where = challenge.host ? `${challenge.host}:${challenge.port ?? ''}` : `${form.value.host}:${form.value.port}`
        const proceed = await confirmDialog({
          title: challenge.kind === 'mismatch' ? '主机密钥不一致' : '首次连接该主机',
          variant: challenge.kind === 'mismatch' ? 'danger' : 'warning',
          message: challenge.kind === 'mismatch'
            ? `${where} 的主机密钥与 ~/.ssh/known_hosts 不一致。`
            : `${where} 的主机密钥尚未记录，是否信任并写入？`,
          detail: `指纹：\n${challenge.fingerprint}`,
          confirmText: '信任这一跳并继续',
          cancelText: '取消'
        })
        if (!proceed || !challenge.host || challenge.port == null) return
        trusted.push({ host: challenge.host, port: challenge.port })
      }
    }
  } catch (e) {
    emit('toast', `连接失败: ${errorText(e)}`, 'error')
  } finally {
    testing.value = false
  }
}

// ESC 关闭弹窗；若上层还有确认框，先交给确认框处理
const onKeydown = (e: KeyboardEvent) => {
  if (!props.open || dialogState.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('close')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))

const handleSubmit = () => {
  const tags = tagInput.value.split(',').map(s => s.trim()).filter(Boolean)
  // 不再随保存伪造指标：监控数据仅来自真实探测结果，编辑时沿用已有值
  const metrics = props.hostData
    ? {
        os: props.hostData.os,
        osName: props.hostData.osName,
        status: props.hostData.status,
        ping: props.hostData.ping,
        cpu: props.hostData.cpu,
        mem: props.hostData.mem,
        disk: props.hostData.disk,
        updatedAt: props.hostData.updatedAt
      }
    : {}
  const item: HostItem = {
    ...form.value,
    proxy: form.value.proxy.kind && form.value.proxy.host ? form.value.proxy : null,
    ...metrics,
    tags
  }
  if (isQuick.value) {
    // 临时主机必须带唯一 id，否则会被 handleConnectSsh 当成已有会话去重
    emit('connect-quick', { ...item, id: 'quick-' + Date.now() })
    return
  }
  emit('save', item)
}
</script>
