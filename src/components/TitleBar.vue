<template>
  <header
    class="h-10 bg-[#0e1117] border-b border-[#1f2430] flex items-center justify-between pl-3 pr-0 shrink-0 z-40 select-none"
    data-tauri-drag-region
    @dblclick="handleHeaderDblClick"
  >
    <!-- 1. 左侧：品牌 LOGO -->
    <div class="flex items-center gap-2.5 shrink-0 pr-3 pointer-events-none">
      <img
        src="/logo.svg"
        alt="SnailSSH"
        class="w-5 h-5 rounded-md shadow-glow-emerald object-contain select-none"
      />
      <span class="font-bold text-xs tracking-wider bg-gradient-to-r from-white via-slate-200 to-brand-300 bg-clip-text text-transparent">
        SnailSSH
      </span>
    </div>

    <div class="h-4 w-[1px] bg-slate-700/60 shrink-0"></div>

    <!-- 2. 中间：固定入口 + 可滚动会话标签。
         drag-region 用裸属性（非 deep）：只有直接点到容器留白/标签间隙才拖窗口，
         点到会话标签本身仍是切换会话，滚轮横滚也不受影响。 -->
    <div class="flex items-center flex-1 min-w-0 h-full" data-tauri-drag-region>
      <button
        @click="$emit('select-session', 'hosts')"
        :class="activeSessionId === 'hosts' ? 'bg-[#1a202c] text-emerald-400 font-semibold border border-brand-500/30 shadow-sm' : 'text-slate-400 hover:text-white hover:bg-surface-card/60 border border-transparent'"
        class="flex items-center gap-1.5 px-3 py-1 ml-2 rounded-md text-xs transition-all shrink-0 font-medium"
      >
        <Server class="w-3.5 h-3.5 text-brand-400" />
        <span>主机资产</span>
      </button>

      <div class="relative flex-1 min-w-0 h-full">
        <div
          v-show="canScrollSessionTabsLeft"
          class="pointer-events-none absolute inset-y-0 left-0 z-10 w-7 bg-gradient-to-r from-[#0e1117] to-transparent"
        ></div>
        <button
          v-show="canScrollSessionTabsLeft"
          @click="scrollSessionTabs(-1)"
          class="absolute left-0.5 top-1/2 z-20 -translate-y-1/2 flex h-6 w-5 items-center justify-center rounded text-slate-300 hover:text-white hover:bg-white/10 transition-colors"
          title="查看前面的会话"
        >
          <ChevronLeft class="w-3.5 h-3.5" />
        </button>

        <div
          ref="sessionTabScroller"
          data-tauri-drag-region
          class="h-full overflow-x-auto overflow-y-hidden no-scrollbar px-1.5"
          @scroll.passive="updateSessionTabOverflow"
        >
          <div ref="sessionTabRow" data-tauri-drag-region class="flex items-center gap-1.5 h-full w-max">
            <div
              v-for="s in sessions"
              :key="s.id"
              :data-session-tab="s.id"
              @click="$emit('select-session', s.id)"
              :class="activeSessionId === s.id ? 'bg-[#1a202c] text-emerald-400 font-semibold border border-brand-500/30 shadow-sm' : 'text-slate-400 hover:text-white hover:bg-surface-card/60 border border-transparent'"
              class="group flex items-center gap-2 px-2.5 py-1 rounded-md text-xs font-mono cursor-pointer transition-all shrink-0 max-w-[190px]"
            >
              <SquareTerminal
                v-if="isLocalWorkspace(s)"
                class="w-3.5 h-3.5 text-sky-400 shrink-0"
                title="本地终端"
              />
              <span
                v-else
                class="w-2 h-2 rounded-full shrink-0"
                :class="[groupColorOf(groupById(hostGroupId(s)).color).dot, activeSessionId === s.id ? 'animate-pulse' : '']"
                :title="groupById(hostGroupId(s)).label"
              ></span>
              <span class="truncate">{{ s.title }}</span>
              <span
                v-if="isHostSession(s) && s.workspaceMode === 'sftp'"
                class="text-[9px] px-1 py-0.2 rounded bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 shrink-0 font-sans"
              >
                SFTP
              </span>
              <button
                @click.stop="$emit('close-session', s.id)"
                class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated hover:text-white text-slate-400 transition-opacity ml-0.5"
                title="关闭该会话"
              >
                <X class="w-3 h-3" />
              </button>
            </div>
          </div>
        </div>

        <div
          v-show="canScrollSessionTabsRight"
          class="pointer-events-none absolute inset-y-0 right-0 z-10 w-7 bg-gradient-to-l from-[#0e1117] to-transparent"
        ></div>
        <button
          v-show="canScrollSessionTabsRight"
          @click="scrollSessionTabs(1)"
          class="absolute right-0.5 top-1/2 z-20 -translate-y-1/2 flex h-6 w-5 items-center justify-center rounded text-slate-300 hover:text-white hover:bg-white/10 transition-colors"
          title="查看后面的会话"
        >
          <ChevronRight class="w-3.5 h-3.5" />
        </button>
      </div>

      <!-- 加号固定在滚动区外，会话再多也能直接连下一台 -->
      <button
        @click="$emit('open-palette')"
        class="p-1 rounded-md text-slate-400 hover:text-white hover:bg-surface-card transition-colors shrink-0 ml-0.5 mr-1"
        title="连接服务器 / 搜索 (Ctrl+K)"
      >
        <Plus class="w-3.5 h-3.5" />
      </button>
    </div>

    <!-- 3. 右侧：操作区与 Windows 风格窗口控制按钮 -->
    <div class="flex items-center h-full text-xs shrink-0">
      <!-- 搜索触发按键 -->
      <button
        @click="$emit('open-palette')"
        class="hidden lg:flex items-center gap-2 px-2.5 py-1 rounded-md bg-surface-card/70 border border-surface-border text-surface-subtle hover:text-white hover:border-brand-500/40 hover:bg-surface-card transition-all text-xs mr-2"
        title="全局快速搜索 (Ctrl+K)"
      >
        <Search class="w-3.5 h-3.5 text-slate-400" />
        <span class="text-[11px]">搜索</span>
        <kbd class="font-mono text-[9px] bg-surface-highlight text-slate-300 px-1 py-0.2 rounded border border-surface-border/80">Ctrl+K</kbd>
      </button>

      <!-- 传输任务指示悬浮胶囊 -->
      <button
        @click="$emit('toggle-transfers')"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card/70 border border-surface-border hover:border-brand-500/40 text-slate-300 hover:text-white transition-all text-xs mr-2"
        :title="activeTransfersCount > 0 ? '查看进行中的传输任务' : '当前没有进行中的传输'"
      >
        <div class="relative flex items-center justify-center">
          <ArrowUpDown class="w-3.5 h-3.5 text-brand-400" />
          <span v-if="activeTransfersCount > 0" class="absolute -top-1 -right-1 w-1.5 h-1.5 rounded-full bg-brand-500 animate-pulse"></span>
        </div>
        <template v-if="activeTransfersCount > 0">
          <span class="hidden md:inline font-mono text-[11px]">{{ activeTransfersCount }} 任务</span>
          <span class="text-brand-400 font-mono font-semibold text-[11px]">{{ transferSpeed }}</span>
        </template>
        <span v-else class="hidden md:inline font-mono text-[11px] text-surface-subtle">传输</span>
      </button>

      <!-- 快速新建主机按钮 -->
      <button
        @click="$emit('open-new-host')"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold transition-all shadow-glow-emerald active:scale-95 text-xs mr-1"
      >
        <Plus class="w-3.5 h-3.5 stroke-[2.5]" />
        <span class="hidden sm:inline">新建</span>
      </button>

      <!-- 钥匙串异常提醒：徽标常驻，避免凭证明文回退只写日志没人看见 -->
      <button
        v-if="keyringBroken"
        @click="$emit('open-settings', 'security')"
        class="p-1.5 rounded-md bg-amber-500/15 border border-amber-500/40 text-amber-400 hover:bg-amber-500/25 hover:text-amber-300 transition-colors mr-2"
        title="系统钥匙串不可用，凭证将以明文保存在 hosts.json；点击查看详情"
      >
        <ShieldAlert class="w-3.5 h-3.5" />
      </button>

      <!-- 系统设置 -->
      <button
        @click="$emit('open-settings')"
        class="p-1.5 rounded-md hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors mr-2"
        title="系统设置"
      >
        <Settings class="w-3.5 h-3.5" />
      </button>

      <div class="h-4 w-[1px] bg-slate-700/60 shrink-0 mr-1"></div>

      <!-- Windows 风格自定义窗口控制按钮组 (Window Controls) -->
      <div class="flex items-center h-full">
        <!-- 最小化 -->
        <button
          @click="minimizeWindow"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 active:bg-white/15 transition-colors"
          title="最小化"
        >
          <Minus class="w-3.5 h-3.5" />
        </button>

        <!-- 最大化 / 还原 -->
        <button
          @click="toggleMaximize"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-white/10 active:bg-white/15 transition-colors"
          :title="isMaximized ? '向下还原' : '最大化'"
        >
          <!-- 还原状态双层框图标 -->
          <svg v-if="isMaximized" class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="4.5" y="1.5" width="10" height="10" rx="0.5" />
            <path d="M1.5 5.5v9h9" />
          </svg>
          <!-- 最大化状态单方框图标 -->
          <Square v-else class="w-3 h-3 stroke-[1.2]" />
        </button>

        <!-- 关闭窗口 -->
        <button
          @click="closeWindow"
          class="w-11 h-full flex items-center justify-center text-slate-400 hover:text-white hover:bg-[#e81123] active:bg-[#c4101e] transition-colors"
          title="关闭窗口"
        >
          <X class="w-4 h-4" />
        </button>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import {
  Search,
  ArrowUpDown,
  Plus,
  Settings,
  Server,
  SquareTerminal,
  X,
  Minus,
  Square,
  ShieldAlert,
  ChevronLeft,
  ChevronRight
} from 'lucide-vue-next'
import { getStorageHealthApi, isTauri } from '@/api/tauri'
import { groupById, groupColorOf } from '@/composables/useAppSettings'
import { isHostSession, isLocalWorkspace, type StorageHealth, type WorkspaceSession } from '@/types'

const props = withDefaults(
  defineProps<{
    activeTransfersCount: number
    transferSpeed: string
    sessions?: WorkspaceSession[]
    activeSessionId?: string
  }>(),
  {
    sessions: () => [],
    activeSessionId: 'hosts'
  }
)

// 滚动区固定 px-1.5。箭头叠在两端，不额外占位，避免滚到头时出现空白。
const SESSION_TAB_GUTTER = 6

const sessionTabScroller = ref<HTMLElement | null>(null)
const sessionTabRow = ref<HTMLElement | null>(null)
const canScrollSessionTabsLeft = ref(false)
const canScrollSessionTabsRight = ref(false)
const sessionTabsOverflow = ref(false)
let sessionTabResizeObserver: ResizeObserver | null = null

const sessionTabSignature = computed(() =>
  (props.sessions ?? [])
    .map((session) => {
      const mode = isHostSession(session) ? session.workspaceMode : ''
      return `${session.id}:${session.title}:${mode}`
    })
    .join('|')
)

const updateSessionTabOverflow = () => {
  const scroller = sessionTabScroller.value
  const row = sessionTabRow.value
  if (!scroller || !row) {
    canScrollSessionTabsLeft.value = false
    canScrollSessionTabsRight.value = false
    sessionTabsOverflow.value = false
    return
  }
  const overflows = row.offsetWidth > scroller.clientWidth - SESSION_TAB_GUTTER * 2 + 1
  sessionTabsOverflow.value = overflows
  const maxScroll = scroller.scrollWidth - scroller.clientWidth
  canScrollSessionTabsLeft.value = overflows && scroller.scrollLeft > 1
  canScrollSessionTabsRight.value = overflows && scroller.scrollLeft < maxScroll - 1
}

const scrollSessionTabs = (direction: -1 | 1) => {
  const scroller = sessionTabScroller.value
  if (!scroller) return
  const distance = Math.max(160, Math.round(scroller.clientWidth * 0.6))
  scroller.scrollBy({ left: direction * distance, behavior: 'smooth' })
}

const revealActiveSessionTab = () => {
  updateSessionTabOverflow()
  // 新标签插入后再量，避免还没布局完就把当前会话算偏
  void nextTick(() => {
    updateSessionTabOverflow()
    const scroller = sessionTabScroller.value
    const id = props.activeSessionId
    if (!scroller || !id || id === 'hosts') return
    const escaped = typeof CSS !== 'undefined' && typeof CSS.escape === 'function' ? CSS.escape(id) : id
    const tab = scroller.querySelector<HTMLElement>(`[data-session-tab="${escaped}"]`)
    if (!tab) return
    const scrollerRect = scroller.getBoundingClientRect()
    const tabRect = tab.getBoundingClientRect()
    const gutter = sessionTabsOverflow.value ? 28 : 4
    if (tabRect.left < scrollerRect.left + gutter) {
      scroller.scrollBy({ left: tabRect.left - scrollerRect.left - gutter, behavior: 'auto' })
    } else if (tabRect.right > scrollerRect.right - gutter) {
      scroller.scrollBy({ left: tabRect.right - scrollerRect.right + gutter, behavior: 'auto' })
    }
  })
}

const onSessionTabWheel = (event: WheelEvent) => {
  const scroller = sessionTabScroller.value
  if (!scroller || !sessionTabsOverflow.value) return
  const dominant = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY
  if (dominant === 0) return
  let delta = dominant
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) delta *= 16
  else if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) delta *= scroller.clientWidth
  event.preventDefault()
  scroller.scrollLeft += delta
}

watch(sessionTabSignature, () => {
  void nextTick(revealActiveSessionTab)
})
watch(
  () => props.activeSessionId,
  () => {
    void nextTick(revealActiveSessionTab)
  }
)

const hostGroupId = (session: WorkspaceSession) => isHostSession(session) ? session.host.group : ''

const emit = defineEmits<{
  (e: 'open-palette'): void
  (e: 'toggle-transfers'): void
  (e: 'open-new-host'): void
  (e: 'open-settings', tab?: string): void
  (e: 'select-session', sessionId: string): void
  (e: 'close-session', sessionId: string): void
  (e: 'close-window'): void
}>()

const isMaximized = ref(false)

// null 视为未知（浏览器预览或后端未注册命令），只在明确 keyringOk === false 时告警
const storageHealth = ref<StorageHealth | null>(null)
const keyringBroken = computed(() => storageHealth.value?.keyringOk === false)

const checkMaximized = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      isMaximized.value = await getCurrentWindow().isMaximized()
    } catch {}
  }
}

const minimizeWindow = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().minimize()
    } catch (e) {
      console.warn('Tauri window minimize error:', e)
    }
  }
}

const toggleMaximize = async () => {
  if (isTauri()) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const win = getCurrentWindow()
      await win.toggleMaximize()
      isMaximized.value = await win.isMaximized()
    } catch (e) {
      console.warn('Tauri window toggleMaximize error:', e)
    }
  }
}

const closeWindow = () => {
  emit('close-window')
}

const handleHeaderDblClick = (e: MouseEvent) => {
  // 排除按钮点击引起的双击
  const target = e.target as HTMLElement
  if (target && target.closest('button')) return
  toggleMaximize()
}

const bindSessionTabScroller = () => {
  const scroller = sessionTabScroller.value
  const row = sessionTabRow.value
  if (!scroller) return
  scroller.addEventListener('wheel', onSessionTabWheel, { passive: false })
  if (typeof ResizeObserver !== 'undefined') {
    sessionTabResizeObserver = new ResizeObserver(() => updateSessionTabOverflow())
    sessionTabResizeObserver.observe(scroller)
    if (row) sessionTabResizeObserver.observe(row)
  }
  revealActiveSessionTab()
}

const unbindSessionTabScroller = () => {
  sessionTabResizeObserver?.disconnect()
  sessionTabResizeObserver = null
  sessionTabScroller.value?.removeEventListener('wheel', onSessionTabWheel)
}

onMounted(async () => {
  bindSessionTabScroller()

  // 启动后只拉一次，不做轮询：钥匙串状态在进程生命周期内不会自愈，用户可在设置中心重新检测
  void getStorageHealthApi().then((health) => {
    storageHealth.value = health
  })

  if (isTauri()) {
    await checkMaximized()
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const win = getCurrentWindow()
      await win.onResized(async () => {
        isMaximized.value = await win.isMaximized()
      })
    } catch {}
  }
})

onBeforeUnmount(unbindSessionTabScroller)
</script>
