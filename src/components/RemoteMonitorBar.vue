<template>
  <div
    class="h-8 bg-[#090b10] border-t border-surface-border/80 px-2.5 flex items-center justify-between text-xs font-mono select-none overflow-x-auto no-scrollbar shrink-0 z-20 text-slate-300"
  >
    <!-- 左侧指标组：各块之间使用细分隔线分割，完美对齐 MobaXterm 底部信息栏 -->
    <div class="flex items-center divide-x divide-slate-800/80 overflow-x-auto no-scrollbar py-0.5">
      <!-- 1. 主机/系统名称 -->
      <div
        class="flex items-center gap-1.5 pr-2.5 shrink-0"
        :title="`操作系统: ${metrics?.osName || 'Linux'} | 主机: ${displayHostname}`"
      >
        <!-- 红帽 / Linux 发行版徽标 -->
        <svg class="w-3.5 h-3.5 shrink-0" viewBox="0 0 24 24" fill="none">
          <ellipse cx="12" cy="14" rx="10" ry="5" fill="#EE0000" />
          <path d="M7 13.5C7 9.5 9 6 12 6C15 6 17 9.5 17 13.5C17 14 16.5 14.5 16 14.5C14 14.5 13.5 15.5 12 15.5C10.5 15.5 10 14.5 8 14.5C7.5 14.5 7 14 7 13.5Z" fill="#151515" />
          <path d="M12 6C12.8 6 13.5 5.2 13.5 4.5C13.5 3.8 12.8 3 12 3C11.2 3 10.5 3.8 10.5 4.5C10.5 5.2 11.2 6 12 6Z" fill="#EE0000" />
        </svg>
        <span class="text-slate-200 font-semibold tracking-tight text-[11px] truncate max-w-[130px]">
          {{ displayHostname }}
        </span>
      </div>

      <!-- 2. CPU 利用率 -->
      <div
        class="flex items-center gap-1.5 px-2.5 shrink-0"
        :title="`CPU 使用率: ${metrics?.cpu ?? 0}%`"
      >
        <Cpu class="w-3.5 h-3.5 text-emerald-400 shrink-0" />
        <span class="text-[11px] min-w-[28px]" :class="cpuColorClass">
          {{ metrics?.cpu != null ? `${metrics.cpu}%` : '--%' }}
        </span>
        <!-- 迷你利用率进度条 -->
        <div class="w-10 h-2 bg-slate-900 border border-slate-700/60 rounded-[2px] overflow-hidden p-[0.5px] flex items-center">
          <div
            class="h-full rounded-[1px] transition-all duration-300"
            :class="cpuBarClass"
            :style="{ width: `${Math.min(100, Math.max(2, metrics?.cpu ?? 0))}%` }"
          ></div>
        </div>
      </div>

      <!-- 3. 内存占用 (已用 / 总量) -->
      <div
        class="flex items-center gap-1.5 px-2.5 shrink-0"
        :title="`内存使用: ${formatBytes(metrics?.memUsedBytes || 0)} / ${formatBytes(metrics?.memTotalBytes || 0)} (${metrics?.memUsagePercent || 0}%)`"
      >
        <Server class="w-3.5 h-3.5 text-emerald-400 shrink-0" />
        <span class="text-[11px] font-medium" :class="memColorClass">
          {{ formatBytes(metrics?.memUsedBytes || 0) }} / {{ formatBytes(metrics?.memTotalBytes || 0) }}
        </span>
        <!-- 内存迷你进度条 -->
        <div class="w-8 h-2 bg-slate-900 border border-slate-700/60 rounded-[2px] overflow-hidden p-[0.5px] flex items-center">
          <div
            class="h-full rounded-[1px] transition-all duration-300"
            :class="memBarClass"
            :style="{ width: `${Math.min(100, Math.max(2, metrics?.memUsagePercent ?? 0))}%` }"
          ></div>
        </div>
      </div>

      <!-- 4. 实时网络吞吐 (上行 / 下行) -->
      <div
        class="flex items-center gap-2.5 px-2.5 shrink-0"
        :title="`实时网速: 上行 ${formatSpeed(metrics?.netUploadSpeed || 0)} | 下行 ${formatSpeed(metrics?.netDownloadSpeed || 0)}`"
      >
        <div class="flex items-center gap-1 text-emerald-400">
          <ArrowUp class="w-3 h-3 stroke-[2.5]" />
          <span class="text-[11px] font-medium text-emerald-300 min-w-[55px]">
            {{ formatSpeed(metrics?.netUploadSpeed || 0) }}
          </span>
        </div>
        <div class="flex items-center gap-1 text-cyan-400">
          <ArrowDown class="w-3 h-3 stroke-[2.5]" />
          <span class="text-[11px] font-medium text-cyan-300 min-w-[55px]">
            {{ formatSpeed(metrics?.netDownloadSpeed || 0) }}
          </span>
        </div>
      </div>

      <!-- 5. 开机运行时间 (Uptime) -->
      <div
        class="flex items-center gap-1.5 px-2.5 shrink-0"
        :title="`系统运行时长: ${metrics?.uptime || '检测中'}`"
      >
        <Clock class="w-3.5 h-3.5 text-emerald-400 shrink-0" />
        <span class="text-[11px] text-slate-300">
          {{ metrics?.uptime || '--' }}
        </span>
      </div>

      <!-- 6. 在线登录终端用户 -->
      <div
        class="flex items-center gap-1.5 px-2.5 shrink-0 max-w-[200px]"
        :title="`当前活跃登录会话: ${userListText}`"
      >
        <User class="w-3.5 h-3.5 text-amber-400 shrink-0" />
        <span class="text-[11px] text-amber-200/90 truncate">
          {{ userListText }}
        </span>
      </div>

      <!-- 7. 主要磁盘分区使用率 -->
      <div
        class="flex items-center gap-1.5 px-2.5 shrink-0"
        :title="diskTooltip"
      >
        <HardDrive class="w-3.5 h-3.5 text-slate-400 shrink-0" />
        <div class="flex items-center gap-2 text-[11px]">
          <span
            v-for="(disk, idx) in displayDisks"
            :key="disk.mountPoint || idx"
            class="flex items-center gap-1"
            :class="getDiskColorClass(disk.usePercent)"
          >
            <span class="text-slate-400 font-mono">{{ disk.mountPoint }}:</span>
            <span>{{ disk.usePercent }}%</span>
          </span>
          <span v-if="!displayDisks.length" class="text-slate-500">--</span>
        </div>
      </div>
    </div>

    <!-- 右侧：活动状态点 + 关闭按钮 -->
    <div class="flex items-center gap-2 shrink-0 pl-2">
      <!-- 采样刷新动态微指示灯 -->
      <span
        class="w-1.5 h-1.5 rounded-full transition-all duration-300"
        :class="loading ? 'bg-emerald-400 shadow-glow-emerald scale-125' : 'bg-slate-600'"
        :title="loading ? '正在同步采集远端指标...' : '每 2.5 秒自动采样'"
      ></span>

      <!-- 红色圆形关闭按钮 (对标 MobaXterm 右侧关闭按钮) -->
      <button
        @click="$emit('close')"
        class="w-4 h-4 rounded-full bg-rose-500/80 hover:bg-rose-500 active:bg-rose-600 text-white flex items-center justify-center transition-colors shadow-sm group"
        title="关闭远程监控栏"
      >
        <X class="w-2.5 h-2.5 stroke-[3] group-hover:scale-110 transition-transform" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Cpu,
  Server,
  ArrowUp,
  ArrowDown,
  Clock,
  User,
  HardDrive,
  X
} from 'lucide-vue-next'
import type { RemoteSystemMetrics } from '@/types'

const props = defineProps<{
  metrics: RemoteSystemMetrics | null
  loading?: boolean
  defaultHostname?: string
}>()

defineEmits<{
  (e: 'close'): void
}>()

const displayHostname = computed(() => {
  return props.metrics?.hostname || props.defaultHostname || 'localhost'
})

const userListText = computed(() => {
  if (!props.metrics?.users || props.metrics.users.length === 0) return 'root'
  return props.metrics.users.slice(0, 5).join(' ')
})

const displayDisks = computed(() => {
  if (!props.metrics?.disks || props.metrics.disks.length === 0) return []
  return props.metrics.disks.slice(0, 3)
})

const diskTooltip = computed(() => {
  if (!props.metrics?.disks || props.metrics.disks.length === 0) return '磁盘指标加载中'
  return props.metrics.disks
    .map(d => `${d.mountPoint}: ${d.used}/${d.total} (${d.usePercent}%)`)
    .join(' | ')
})

const cpuColorClass = computed(() => {
  const cpu = props.metrics?.cpu ?? 0
  if (cpu >= 90) return 'text-rose-400 font-semibold'
  if (cpu >= 70) return 'text-amber-400'
  return 'text-emerald-300'
})

const cpuBarClass = computed(() => {
  const cpu = props.metrics?.cpu ?? 0
  if (cpu >= 90) return 'bg-rose-500'
  if (cpu >= 70) return 'bg-amber-400'
  return 'bg-emerald-400'
})

const memColorClass = computed(() => {
  const pct = props.metrics?.memUsagePercent ?? 0
  if (pct >= 90) return 'text-rose-400'
  if (pct >= 75) return 'text-amber-300'
  return 'text-emerald-300'
})

const memBarClass = computed(() => {
  const pct = props.metrics?.memUsagePercent ?? 0
  if (pct >= 90) return 'bg-rose-500'
  if (pct >= 75) return 'bg-amber-400'
  return 'bg-emerald-400'
})

const getDiskColorClass = (percent: number) => {
  if (percent >= 90) return 'text-rose-400 font-semibold'
  if (percent >= 80) return 'text-amber-300'
  return 'text-slate-200'
}

/** 格式化字节为 GB/MB，对齐 MobaXterm 的 14.30 GB 格式 */
const formatBytes = (bytes: number): string => {
  if (!bytes || isNaN(bytes) || bytes <= 0) return '0.00 GB'
  const gb = bytes / (1024 * 1024 * 1024)
  if (gb >= 1) {
    return `${gb.toFixed(2)} GB`
  }
  const mb = bytes / (1024 * 1024)
  return `${mb.toFixed(1)} MB`
}

/** 格式化网速为 Mb/s (兆比特/秒)，对标 MobaXterm 的 0.05 Mb/s 格式 */
const formatSpeed = (bytesPerSec: number): string => {
  if (!bytesPerSec || isNaN(bytesPerSec) || bytesPerSec <= 0) return '0.00 Mb/s'
  const bitsPerSec = bytesPerSec * 8
  const mbits = bitsPerSec / (1000 * 1000)
  return `${mbits.toFixed(2)} Mb/s`
}
</script>
