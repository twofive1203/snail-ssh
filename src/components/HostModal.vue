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
            <h2 class="text-sm font-bold text-white">{{ isEditing ? '编辑 SSH 服务器' : '配置新建服务器' }}</h2>
            <p class="text-[11px] text-surface-subtle">填写连接凭证与认证方式，密码/私钥会存入系统钥匙串</p>
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
          <button type="button" class="px-4 py-2 border-b-2 border-brand-500 text-brand-400 font-medium">常规设置</button>
          <button type="button" @click="$emit('toast', '跳板机 & 代理尚未实现，将在后续版本提供', 'warning')" class="px-4 py-2 border-b-2 border-transparent text-slate-500 hover:text-slate-300 flex items-center gap-1.5">
            <span>跳板机 & 代理</span>
            <span class="text-[9px] px-1 py-0.2 rounded bg-surface-highlight text-slate-400 border border-surface-border font-mono">即将推出</span>
          </button>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5 col-span-2 sm:col-span-1">
            <label class="text-xs font-medium text-slate-300">服务器别名 (Name) <span class="text-rose-400">*</span></label>
            <input
              v-model="form.name"
              type="text"
              required
              placeholder="如: prod-k8s-master-01"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            />
          </div>
          <div class="space-y-1.5 col-span-2 sm:col-span-1">
            <label class="text-xs font-medium text-slate-300">所属分组 (Group)</label>
            <select
              v-model="form.group"
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            >
              <option value="production">生产集群 (Production)</option>
              <option value="staging">测试环境 (Staging)</option>
              <option value="cloud">个人 VPS & 实验机</option>
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
          <div class="grid grid-cols-3 gap-2">
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

        <!-- 标签设置 -->
        <div class="space-y-1.5">
          <label class="text-xs font-medium text-slate-300">分类标签 (英文逗号分隔)</label>
          <input
            v-model="tagInput"
            type="text"
            placeholder="如: Ubuntu, Web, Aliyun"
            class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
          />
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
            保存配置
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { Server, X } from 'lucide-vue-next'
import type { HostItem, AuthType } from '@/types'
import { testSshConnectionApi, isTauri } from '@/api/tauri'
import { confirmDialog, dialogState } from '@/composables/useDialog'

const props = defineProps<{
  open: boolean
  hostData?: HostItem | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'save', host: HostItem): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const isEditing = ref(false)
const tagInput = ref('')
const testing = ref(false)

const form = ref<{
  id: string
  name: string
  host: string
  port: number
  username: string
  authType: AuthType
  password?: string
  privateKey?: string
  passphrase?: string
  group: string
}>({
  id: '',
  name: '',
  host: '',
  port: 22,
  username: 'root',
  authType: 'password',
  password: '',
  privateKey: '',
  passphrase: '',
  group: 'production'
})

watch(() => props.open, (val) => {
  if (val) {
    if (props.hostData) {
      isEditing.value = true
      form.value = { ...props.hostData }
      tagInput.value = props.hostData.tags.join(', ')
    } else {
      isEditing.value = false
      form.value = {
        id: 'host-' + Date.now(),
        name: '',
        host: '',
        port: 22,
        username: 'root',
        authType: 'password',
        password: '',
        privateKey: '',
        passphrase: '',
        group: 'production'
      }
      tagInput.value = 'Ubuntu, prod'
    }
  }
})

// 构造后端测试所需的连接配置
const buildConfig = (): HostItem => ({
  ...form.value,
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
  try {
    const res = await testSshConnectionApi(buildConfig(), false)
    emit(
      'toast',
      `连接成功：TCP+握手+认证共 ${res.latencyMs} ms，主机密钥 ${res.hostKeyFingerprint}`,
      'success'
    )
  } catch (e) {
    const msg = errorText(e)
    if (msg.startsWith('HOST_KEY_MISMATCH|')) {
      const parts = msg.split('|')
      const newFp = parts[1] || '未知'
      const oldFps = (parts[2] || '').split(';').filter(Boolean)
      const proceed = await confirmDialog({
        title: '主机密钥不一致',
        variant: 'danger',
        message: `${form.value.host}:${form.value.port} 的主机密钥与 ~/.ssh/known_hosts 记录不一致。`,
        detail: `本次指纹：\n${newFp}\n\n已知记录：\n${oldFps.join('\n') || '（无法读取）'}`,
        confirmText: '确认更换并重试',
        cancelText: '取消'
      })
      if (proceed) {
        try {
          const res = await testSshConnectionApi(buildConfig(), true)
          emit('toast', `已更新 known_hosts，连接成功（${res.latencyMs} ms）`, 'success')
        } catch (e2) {
          emit('toast', `连接失败: ${errorText(e2)}`, 'error')
        }
      }
      return
    }
    const unknown = msg.match(/HOST_KEY_UNKNOWN\|(.+)/)
    if (unknown) {
      const trust = await confirmDialog({
        title: '首次连接该主机',
        variant: 'warning',
        message: '该主机密钥未记录在 ~/.ssh/known_hosts，是否信任并写入后再测试？',
        detail: `主机密钥指纹：\n${unknown[1]}`,
        confirmText: '信任并测试',
        cancelText: '取消'
      })
      if (trust) {
        try {
          const res = await testSshConnectionApi(buildConfig(), true)
          emit('toast', `已信任主机密钥，连接成功（${res.latencyMs} ms）`, 'success')
        } catch (e2) {
          emit('toast', `连接失败: ${errorText(e2)}`, 'error')
        }
      }
    } else {
      emit('toast', `连接失败: ${msg}`, 'error')
    }
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
    ...metrics,
    tags
  }
  emit('save', item)
}
</script>
