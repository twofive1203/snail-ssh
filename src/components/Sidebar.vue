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
        @click="$emit('open-local-terminal')"
        class="w-full flex items-center justify-between px-2.5 py-2 rounded-lg text-xs font-medium transition-all text-surface-subtle hover:text-slate-200 hover:bg-surface-card"
        title="打开本机 PowerShell。已打开时只是聚焦，不新建第二个工作区"
      >
        <div class="flex items-center gap-2.5">
          <SquareTerminal class="w-4 h-4 text-sky-400" />
          <span>本地终端</span>
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
      </button>
    </div>

    <!-- 分组过滤器 (Host Groups)：分组与颜色来自设置中心 -->
    <div class="flex-1 overflow-y-auto px-3 py-3 space-y-4 custom-scrollbar">
      <div>
        <div class="flex items-center justify-between px-2 mb-2">
          <span class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase font-mono">主机分组</span>
          <button
            @click="$emit('open-settings', 'groups')"
            class="text-slate-500 hover:text-brand-400 transition-colors p-0.5 rounded"
            title="管理分组（设置中心）"
          >
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
            v-for="g in settings.groups"
            :key="g.id"
            @click="$emit('filter-group', g.id)"
            :class="selectedGroup === g.id ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
          >
            <span class="flex items-center gap-2 min-w-0">
              <span class="w-2 h-2 rounded-full shrink-0 ring-2 ring-white/5" :class="groupColorOf(g.color).dot"></span>
              <span class="truncate">{{ g.label }}</span>
              <span
                v-if="g.danger"
                class="text-[9px] px-1 py-0.2 rounded bg-rose-500/15 text-rose-400 border border-rose-500/30 font-mono shrink-0"
                title="高风险分组：危险命令与批量删除会额外确认"
              >!</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500 shrink-0">{{ groupCounts[g.id] || 0 }}</span>
          </button>

          <!-- 主机里出现过、但设置里已删除的分组：不静默隐藏，否则用户会以为主机丢了 -->
          <button
            v-for="orphan in orphanGroups"
            :key="orphan"
            @click="$emit('filter-group', orphan)"
            :class="selectedGroup === orphan ? 'bg-surface-elevated text-white font-medium' : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md transition-colors"
            :title="`分组「${orphan}」已不在设置中心，建议编辑主机重新归类`"
          >
            <span class="flex items-center gap-2 min-w-0">
              <span class="w-2 h-2 rounded-full shrink-0 bg-slate-600"></span>
              <span class="truncate">{{ orphan }}</span>
              <span class="text-[9px] px-1 py-0.2 rounded bg-slate-500/15 text-slate-400 border border-slate-500/30 font-mono shrink-0">未登记</span>
            </span>
            <span class="font-mono text-[11px] text-slate-500 shrink-0">{{ groupCounts[orphan] || 0 }}</span>
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
          </button>
          <button
            @click="$emit('open-settings', 'snippets')"
            class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md text-surface-subtle hover:text-slate-200 hover:bg-surface-card transition-colors"
            title="管理命令片段（设置中心）"
          >
            <span class="flex items-center gap-2">
              <Code2 class="w-3.5 h-3.5 text-purple-400" />
              <span>常用指令代码库</span>
            </span>
            <span class="text-[10px] font-mono text-slate-500">{{ settings.snippets.length }}</span>
          </button>
          <button @click="$emit('open-tunnels')" class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md text-surface-subtle hover:text-slate-200 hover:bg-surface-card transition-colors">
            <span class="flex items-center gap-2">
              <Network class="w-3.5 h-3.5 text-cyan-400" />
              <span>端口转发 & 隧道</span>
            </span>
          </button>
        </div>
      </div>

      <!-- 快捷入口 -->
      <div>
        <div class="text-[11px] font-semibold text-slate-500 tracking-wider uppercase px-2 mb-2 font-mono">快捷入口</div>
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
            <span class="w-2 h-2 rounded-full shrink-0" :class="groupColorOf(groupById(item.group).color).dot"></span>
          </div>
          <div v-if="recentHosts.length === 0" class="px-2.5 py-1.5 text-[11px] text-slate-500">
            暂无最近连接
          </div>
        </div>
      </div>
    </div>

    <!-- 侧边栏底部：会话状态 -->
    <div class="p-3 border-t border-surface-border bg-surface-card/30 flex items-center justify-between text-xs text-surface-subtle">
      <div class="flex items-center gap-2">
        <div class="w-2 h-2 rounded-full" :class="activeSessionsCount > 0 ? 'bg-emerald-500' : 'bg-slate-600'"></div>
        <span class="font-mono text-[11px]">
          {{ activeSessionsCount > 0 ? `${activeSessionsCount} 个活动会话` : '当前无活动会话' }}
        </span>
      </div>
      <button
        @click="$emit('open-settings')"
        class="flex items-center gap-1 font-mono text-[11px] text-slate-500 hover:text-brand-400 transition-colors"
        title="设置中心 (Ctrl+,)"
      >
        <Settings class="w-3 h-3" />
        <span>设置</span>
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Server,
  SquareTerminal,
  FolderGit2,
  FolderPlus,
  Layers,
  KeyRound,
  Code2,
  Network,
  Settings
} from 'lucide-vue-next'
import type { HostItem } from '@/types'
import { settings, groupById, groupColorOf } from '@/composables/useAppSettings'

const props = defineProps<{
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
  (e: 'open-vault'): void
  (e: 'open-tunnels'): void
  (e: 'open-settings', tab?: string): void
  (e: 'open-local-terminal'): void
}>()

/**
 * 主机里出现、但设置中心已删除的分组。
 * 必须显式列出来：否则这些主机会在「全部服务器」里可见却无法按分组筛选，
 * 用户会误以为资产丢失。
 */
const orphanGroups = computed(() => {
  const known = new Set(settings.groups.map(g => g.id))
  return Object.keys(props.groupCounts).filter(id => !known.has(id) && props.groupCounts[id] > 0)
})
</script>
