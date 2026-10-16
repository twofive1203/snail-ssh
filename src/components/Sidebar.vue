<template>
  <aside class="w-60 bg-surface-sidebar border-r border-surface-border flex flex-col shrink-0 select-none">
    <!-- 顶部视图切换菜单 -->
    <div class="p-3 border-b border-surface-border space-y-1">
      <div class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase px-2 mb-1.5 font-mono">视图导航</div>
      
      <button
        @click="$emit('switch-view', 'hosts')"
        :class="activeView === 'hosts' ? 'bg-brand-500/15 text-emerald-400 border-l-2 border-brand-500' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg text-xs font-medium transition-all group"
      >
        <div class="flex items-center gap-2.5">
          <Server class="w-4 h-4 text-brand-400" />
          <span>服务器管理</span>
        </div>
        <span class="text-[11px] font-mono px-1.5 py-0.2 rounded-full bg-surface-highlight text-slate-300 group-hover:text-white">{{ totalHosts }}</span>
      </button>

      <button
        @click="$emit('switch-view', 'terminal')"
        :class="activeView === 'terminal' ? 'bg-brand-500/15 text-emerald-400 border-l-2 border-brand-500' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg text-xs font-medium transition-all group"
      >
        <div class="flex items-center gap-2.5">
          <SquareTerminal class="w-4 h-4 text-emerald-400" />
          <span>SSH 终端</span>
        </div>
        <div class="flex items-center gap-1.5">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping"></span>
          <span class="text-[10px] font-mono text-emerald-400">{{ activeSessionsCount }} 在线</span>
        </div>
      </button>

      <button
        @click="$emit('switch-view', 'sftp')"
        :class="activeView === 'sftp' ? 'bg-brand-500/15 text-emerald-400 border-l-2 border-brand-500' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg text-xs font-medium transition-all group"
      >
        <div class="flex items-center gap-2.5">
          <FolderGit2 class="w-4 h-4 text-cyan-400" />
          <span>SFTP 文件传输</span>
        </div>
        <span class="text-[10px] font-mono px-1.5 py-0.2 rounded bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">双栏浏览</span>
      </button>
    </div>

    <!-- 分组过滤器 (Host Groups) -->
    <div class="flex-1 overflow-y-auto px-3 py-3 space-y-4 custom-scrollbar">
      <div>
        <div class="flex items-center justify-between px-2 mb-2">
          <span class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase font-mono">主机分组</span>
          <button @click="$emit('add-group')" class="text-slate-500 hover:text-brand-400 transition-colors p-0.5 rounded" title="添加分组">
            <FolderPlus class="w-3.5 h-3.5" />
          </button>
        </div>
        <div class="space-y-0.5 text-xs">
          <button
            @click="$emit('filter-group', 'all')"
            :class="selectedGroup === 'all' ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
          >
            <span class="flex items-center gap-2">
              <Layers class="w-3.5 h-3.5 text-slate-400" />
              <span>全部服务器</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500">{{ totalHosts }}</span>
          </button>

          <button
            @click="$emit('filter-group', 'production')"
            :class="selectedGroup === 'production' ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
          >
            <span class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-rose-500 ring-2 ring-rose-500/20"></span>
              <span>生产集群 (Prod)</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500">{{ groupCounts.production || 0 }}</span>
          </button>

          <button
            @click="$emit('filter-group', 'staging')"
            :class="selectedGroup === 'staging' ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
          >
            <span class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-amber-500 ring-2 ring-amber-500/20"></span>
              <span>测试环境 (Stage)</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500">{{ groupCounts.staging || 0 }}</span>
          </button>

          <button
            @click="$emit('filter-group', 'cloud')"
            :class="selectedGroup === 'cloud' ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
          >
            <span class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full bg-blue-500 ring-2 ring-blue-500/20"></span>
              <span>个人 VPS & 实验机</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500">{{ groupCounts.cloud || 0 }}</span>
          </button>
        </div>
      </div>

      <!-- 安全与扩展 -->
      <div>
        <div class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase px-2 mb-2 font-mono">安全与扩展</div>
        <div class="space-y-0.5 text-xs">
          <button @click="$emit('open-vault')" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md text-surface-subtle hover:text-slate-200 hover:bg-surface-card transition-colors">
            <span class="flex items-center gap-2">
              <KeyRound class="w-3.5 h-3.5 text-amber-400" />
              <span>SSH 密钥保险箱</span>
            </span>
            <span class="font-mono text-[10px] text-slate-500">4 组密钥</span>
          </button>
          <button @click="$emit('open-snippets')" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md text-surface-subtle hover:text-slate-200 hover:bg-surface-card transition-colors">
            <span class="flex items-center gap-2">
              <Code2 class="w-3.5 h-3.5 text-purple-400" />
              <span>常用指令代码库</span>
            </span>
            <span class="font-mono text-[10px] text-slate-500">12 条</span>
          </button>
          <button @click="$emit('open-tunnels')" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md text-surface-subtle hover:text-slate-200 hover:bg-surface-card transition-colors">
            <span class="flex items-center gap-2">
              <Network class="w-3.5 h-3.5 text-cyan-400" />
              <span>端口转发 & 隧道</span>
            </span>
            <span class="font-mono text-[10px] text-slate-500">1 活跃</span>
          </button>
        </div>
      </div>

      <!-- 快速直连历史 (Recent) -->
      <div>
        <div class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase px-2 mb-2 font-mono">快速直连历史</div>
        <div class="space-y-1">
          <div
            v-for="item in recentHosts"
            :key="item.id"
            @click="$emit('connect-host', item)"
            class="p-2 rounded-lg bg-surface-card/60 hover:bg-surface-card border border-surface-border/50 hover:border-brand-500/30 cursor-pointer transition-all flex items-center justify-between"
          >
            <div class="truncate pr-1">
              <div class="text-xs font-medium text-slate-200 truncate">{{ item.name }}</div>
              <div class="text-[10px] font-mono text-slate-500">{{ item.host }}:{{ item.port }}</div>
            </div>
            <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
          </div>
        </div>
      </div>
    </div>

    <!-- 侧边栏底部：运行环境与网络状态 -->
    <div class="p-3 border-t border-surface-border bg-surface-card/30 flex items-center justify-between text-xs text-surface-subtle">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full bg-emerald-500"></div>
        <span class="font-mono text-[11px]">SSH 引擎: 就绪</span>
      </div>
      <span class="font-mono text-[11px] text-slate-500">Ping: 18ms</span>
    </div>
  </aside>
</template>

<script setup lang="ts">
import {
  Server,
  SquareTerminal,
  FolderGit2,
  FolderPlus,
  Layers,
  KeyRound,
  Code2,
  Network
} from 'lucide-vue-next'
import type { HostItem } from '@/types'

defineProps<{
  activeView: 'hosts' | 'terminal' | 'sftp'
  selectedGroup: string
  totalHosts: number
  activeSessionsCount: number
  groupCounts: Record<string, number>
  recentHosts: HostItem[]
}>()

defineEmits<{
  (e: 'switch-view', view: 'hosts' | 'terminal' | 'sftp'): void
  (e: 'filter-group', group: string): void
  (e: 'connect-host', host: HostItem): void
  (e: 'add-group'): void
  (e: 'open-vault'): void
  (e: 'open-snippets'): void
  (e: 'open-tunnels'): void
}>()
</script>
