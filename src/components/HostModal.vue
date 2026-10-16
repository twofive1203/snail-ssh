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
            <p class="text-[11px] text-surface-subtle">填写连接凭证、认证方式及跳板机参数</p>
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
          <button type="button" @click="$emit('toast', '跳板机/代理功能已就绪', 'info')" class="px-4 py-2 border-b-2 border-transparent text-slate-400 hover:text-slate-200">跳板机 & 代理</button>
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
            <label class="text-xs font-medium text-slate-300">私钥文本 (Private Key PEM)</label>
            <textarea
              v-model="form.privateKey"
              rows="3"
              placeholder="-----BEGIN OPENSSH PRIVATE KEY-----..."
              class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
            ></textarea>
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
            class="px-4 py-2 rounded-lg bg-surface-elevated border border-surface-border hover:border-brand-500/50 text-xs text-brand-400 transition-colors"
          >
            测试连通性
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
import { ref, watch } from 'vue'
import { Server, X } from 'lucide-vue-next'
import type { HostItem, AuthType } from '@/types'

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

const form = ref<{
  id: string
  name: string
  host: string
  port: number
  username: string
  authType: AuthType
  password?: string
  privateKey?: string
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
        group: 'production'
      }
      tagInput.value = 'Ubuntu, prod'
    }
  }
})

const testConnect = () => {
  emit('toast', `正在测试连接 ${form.value.host}:${form.value.port}...`, 'info')
  setTimeout(() => {
    emit('toast', '连接测试成功！网络延迟 22ms', 'success')
  }, 500)
}

const handleSubmit = () => {
  const tags = tagInput.value.split(',').map(s => s.trim()).filter(Boolean)
  const item: HostItem = {
    ...form.value,
    tags,
    os: 'ubuntu',
    osName: 'Ubuntu 22.04 LTS',
    status: 'online',
    ping: '22ms',
    cpu: Math.floor(Math.random() * 30) + 10,
    mem: Math.floor(Math.random() * 40) + 20,
    disk: 35,
    updatedAt: '刚刚'
  }
  emit('save', item)
}
</script>
