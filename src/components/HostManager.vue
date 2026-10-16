<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden">
    <!-- 顶部操作条 -->
    <div class="h-14 px-6 border-b border-surface-border flex items-center justify-between shrink-0 bg-surface-card/20">
      <div class="flex items-center gap-4">
        <div>
          <h1 class="text-sm font-bold text-white flex items-center gap-2">
            <span>服务器列表</span>
            <span class="text-xs px-2 py-0.5 rounded-full bg-surface-card border border-surface-border text-surface-subtle font-mono font-normal">
              共 {{ filteredHosts.length }} 台主机
            </span>
          </h1>
          <p class="text-[11px] text-surface-subtle">统一管理集群资产、认证凭证及直连入口</p>
        </div>
      </div>

      <!-- 搜索、视图切换、操作按键 -->
      <div class="flex items-center gap-3">
        <!-- 搜索框 -->
        <div class="relative w-64">
          <Search class="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="搜索主机名、IP、标签..."
            class="w-full bg-surface-card border border-surface-border rounded-lg pl-9 pr-3 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-brand-500/60 focus:ring-1 focus:ring-brand-500/40 transition-all font-mono"
          />
        </div>

        <!-- 卡片/表格切换 -->
        <div class="flex items-center bg-surface-card border border-surface-border rounded-lg p-0.5">
          <button
            @click="layout = 'grid'"
            :class="layout === 'grid' ? 'bg-surface-elevated text-white' : 'text-surface-subtle hover:text-white'"
            class="p-1 rounded transition-colors"
            title="网格卡片视图"
          >
            <LayoutGrid class="w-3.5 h-3.5" />
          </button>
          <button
            @click="layout = 'list'"
            :class="layout === 'list' ? 'bg-surface-elevated text-white' : 'text-surface-subtle hover:text-white'"
            class="p-1 rounded transition-colors"
            title="紧凑列表视图"
          >
            <List class="w-3.5 h-3.5" />
          </button>
        </div>

        <!-- 批量导入/导出 -->
        <button
          @click="$emit('import-export')"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-300 hover:text-white transition-all"
        >
          <FileUp class="w-3.5 h-3.5" />
          <span>导入/导出</span>
        </button>

        <!-- 新建服务器 -->
        <button
          @click="$emit('open-new-host')"
          class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald"
        >
          <Plus class="w-4 h-4 stroke-[2.5]" />
          <span>新建服务器</span>
        </button>
      </div>
    </div>

    <!-- 服务器卡片列表区 -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <!-- 空状态 -->
      <div v-if="filteredHosts.length === 0" class="flex flex-col items-center justify-center p-16 text-center">
        <div class="w-14 h-14 rounded-2xl bg-surface-card border border-surface-border flex items-center justify-center text-slate-500 mb-3">
          <ServerOff class="w-7 h-7" />
        </div>
        <div class="text-sm font-semibold text-slate-300">
          {{ hosts.length === 0 ? '还没有任何服务器' : '未找到匹配的服务器' }}
        </div>
        <div class="text-xs text-slate-500 mt-1">
          {{ hosts.length === 0
            ? '点击「新建服务器」添加第一台主机，密码与私钥会存入系统钥匙串'
            : '请尝试更换搜索关键字或清除筛选条件' }}
        </div>
        <button
          v-if="hosts.length > 0"
          @click="searchQuery = ''"
          class="mt-4 px-3.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-brand-400"
        >
          重置搜索
        </button>
        <button
          v-else
          @click="$emit('open-new-host')"
          class="mt-4 px-3.5 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs"
        >
          新建服务器
        </button>
      </div>

      <!-- 网格视图 (Grid) -->
      <div v-else-if="layout === 'grid'" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div
          v-for="host in filteredHosts"
          :key="host.id"
          class="host-card rounded-xl border border-surface-border p-4 flex flex-col justify-between group shadow-glow-card"
        >
          <!-- 头部信息 -->
          <div>
            <div class="flex items-start justify-between mb-2.5">
              <div class="flex items-center gap-2.5">
                <div class="w-8 h-8 rounded-lg bg-surface-card border border-surface-border/80 flex items-center justify-center text-brand-400 group-hover:border-brand-500/30 transition-colors">
                  <component :is="getOsIcon(host.os)" class="w-4 h-4 text-brand-400" />
                </div>
                <div>
                  <div class="text-xs font-bold text-white flex items-center gap-1.5 font-mono">
                    <span>{{ host.name }}</span>
                    <span class="w-2 h-2 rounded-full" :class="statusDotClass(host)" :title="statusTitle(host)"></span>
                  </div>
                  <div class="text-[11px] font-mono text-surface-subtle flex items-center gap-1 mt-0.5">
                    <span>{{ host.username }}@{{ host.host }}:{{ host.port }}</span>
                  </div>
                </div>
              </div>

              <!-- 右侧更多操作 -->
              <div class="flex items-center gap-1">
                <button
                  @click.stop="$emit('edit-host', host)"
                  class="p-1 rounded text-surface-subtle hover:text-white hover:bg-surface-highlight transition-colors"
                  title="编辑配置"
                >
                  <Edit3 class="w-3.5 h-3.5" />
                </button>
                <button
                  @click.stop="$emit('delete-host', host)"
                  class="p-1 rounded text-surface-subtle hover:text-rose-400 hover:bg-surface-highlight transition-colors"
                  title="删除主机"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <!-- 系统与硬件微型指标 -->
            <div class="grid grid-cols-3 gap-2 my-3 p-2 rounded-lg bg-surface-base/60 border border-surface-border/50 text-[11px] font-mono">
              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">CPU</span>
                <div class="flex items-center gap-1.5 mt-0.5">
                  <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
                    <div class="bg-brand-500 h-full rounded-full" :style="{ width: `${host.cpu ?? 0}%` }"></div>
                  </div>
                  <span class="text-slate-300 font-semibold text-[10px]">{{ host.cpu != null ? host.cpu + '%' : '—' }}</span>
                </div>
              </div>

              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">MEM</span>
                <div class="flex items-center gap-1.5 mt-0.5">
                  <div class="w-full bg-surface-highlight h-1.5 rounded-full overflow-hidden">
                    <div class="bg-cyan-500 h-full rounded-full" :style="{ width: `${host.mem ?? 0}%` }"></div>
                  </div>
                  <span class="text-slate-300 font-semibold text-[10px]">{{ host.mem != null ? host.mem + '%' : '—' }}</span>
                </div>
              </div>

              <div class="flex flex-col">
                <span class="text-slate-500 text-[10px]">延迟 (Ping)</span>
                <div class="flex items-center gap-1 mt-0.5">
                  <span class="text-emerald-400 font-semibold text-[11px]">{{ host.ping || '—' }}</span>
                </div>
              </div>
            </div>

            <!-- 标签列表 -->
            <div class="flex flex-wrap gap-1.5 mb-4">
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-surface-highlight text-slate-300 border border-surface-border/60">
                {{ host.osName || '未知系统' }}
              </span>
              <span
                v-for="tag in host.tags"
                :key="tag"
                class="text-[10px] font-mono px-2 py-0.5 rounded bg-brand-500/10 text-brand-300 border border-brand-500/20"
              >
                #{{ tag }}
              </span>
            </div>
          </div>

          <!-- 卡片底部直连动作栏 -->
          <div class="pt-3 border-t border-surface-border/60 flex items-center justify-between text-xs">
            <span class="text-[10px] font-mono text-slate-500 uppercase">{{ host.authType }} 认证</span>
            <div class="flex items-center gap-2">
              <button
                @click="$emit('connect-sftp', host)"
                class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card hover:bg-surface-elevated border border-surface-border hover:border-cyan-500/40 text-cyan-400 text-xs transition-all"
              >
                <FolderGit2 class="w-3.5 h-3.5" />
                <span>SFTP</span>
              </button>
              <button
                @click="$emit('connect-ssh', host)"
                class="flex items-center gap-1.5 px-3 py-1 rounded-md bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald"
              >
                <Terminal class="w-3.5 h-3.5" />
                <span>连接 SSH</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 列表视图 (List) -->
      <div v-else class="bg-surface-card/60 rounded-xl border border-surface-border overflow-hidden">
        <table class="w-full text-left text-xs border-collapse font-mono">
          <thead class="bg-surface-sidebar border-b border-surface-border text-[11px] text-slate-400 uppercase">
            <tr>
              <th class="py-2.5 px-4 font-medium">服务器 / 别名</th>
              <th class="py-2.5 px-4 font-medium">系统</th>
              <th class="py-2.5 px-4 font-medium">IP & 端口</th>
              <th class="py-2.5 px-4 font-medium">认证方式</th>
              <th class="py-2.5 px-4 font-medium text-center">延迟</th>
              <th class="py-2.5 px-4 font-medium">标签</th>
              <th class="py-2.5 px-4 font-medium text-right">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-surface-border/40">
            <tr v-for="host in filteredHosts" :key="host.id" class="hover:bg-surface-card/50 transition-colors">
              <td class="py-3 px-4 font-medium text-white flex items-center gap-2">
                <span class="w-2 h-2 rounded-full" :class="statusDotClass(host)" :title="statusTitle(host)"></span>
                <span class="font-bold">{{ host.name }}</span>
              </td>
              <td class="py-3 px-4 text-slate-300 text-[11px]">{{ host.osName || '未知系统' }}</td>
              <td class="py-3 px-4 text-slate-400">{{ host.username }}@{{ host.host }}:{{ host.port }}</td>
              <td class="py-3 px-4 text-slate-400 uppercase">{{ host.authType }}</td>
              <td class="py-3 px-4 text-center text-emerald-400 font-semibold">{{ host.ping || '—' }}</td>
              <td class="py-3 px-4">
                <div class="flex gap-1 flex-wrap">
                  <span
                    v-for="t in host.tags"
                    :key="t"
                    class="text-[9px] px-1.5 py-0.2 rounded bg-brand-500/10 text-brand-300 border border-brand-500/20"
                  >
                    {{ t }}
                  </span>
                </div>
              </td>
              <td class="py-3 px-4 text-right">
                <div class="flex items-center justify-end gap-2">
                  <button
                    @click="$emit('connect-sftp', host)"
                    class="px-2 py-1 rounded bg-surface-card hover:bg-surface-elevated border border-surface-border text-cyan-400 text-xs"
                  >
                    SFTP
                  </button>
                  <button
                    @click="$emit('connect-ssh', host)"
                    class="px-2.5 py-1 rounded bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs"
                  >
                    连接
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { HostItem } from '@/types'
import {
  Search,
  LayoutGrid,
  List,
  FileUp,
  Plus,
  ServerOff,
  Edit3,
  Trash2,
  FolderGit2,
  Terminal,
  Cpu,
  Box,
  HardDrive,
  Command,
  Server
} from 'lucide-vue-next'

const props = defineProps<{
  hosts: HostItem[]
  selectedGroup: string
}>()

defineEmits<{
  (e: 'connect-ssh', host: HostItem): void
  (e: 'connect-sftp', host: HostItem): void
  (e: 'edit-host', host: HostItem): void
  (e: 'delete-host', host: HostItem): void
  (e: 'open-new-host'): void
  (e: 'import-export'): void
}>()

const layout = ref<'grid' | 'list'>('grid')
const searchQuery = ref('')

const filteredHosts = computed(() => {
  return props.hosts.filter(h => {
    const matchGroup = props.selectedGroup === 'all' || h.group === props.selectedGroup
    const q = searchQuery.value.trim().toLowerCase()
    const matchQuery = !q ||
      h.name.toLowerCase().includes(q) ||
      h.host.toLowerCase().includes(q) ||
      h.tags.some(t => t.toLowerCase().includes(q))
    return matchGroup && matchQuery
  })
})

const getOsIcon = (os?: string) => {
  switch (os) {
    case 'ubuntu':
    case 'debian':
      return Cpu
    case 'centos':
      return Box
    case 'alpine':
      return HardDrive
    case 'apple':
    case 'darwin':
      return Command
    default:
      return Server
  }
}

// 状态点只反映真实状态：连接成功后会标记 online，未探测过则为未知
const statusDotClass = (host: HostItem) => {
  if (host.status === 'online') return 'bg-emerald-400'
  if (host.status === 'offline') return 'bg-rose-500'
  return 'bg-slate-500'
}

const statusTitle = (host: HostItem) => {
  if (host.status === 'online') return `在线${host.ping ? ` (${host.ping})` : ''}`
  if (host.status === 'offline') return '离线'
  return '尚未探测连接状态'
}
</script>
