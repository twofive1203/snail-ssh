<template>
  <div class="flex-1 flex h-full w-full overflow-hidden relative select-none bg-surface-base">
    <!-- 1. 左侧：树形文件工作区 -->
    <aside
      class="h-full flex flex-col shrink-0 transition-all duration-150 relative border-r border-surface-border bg-surface-sidebar"
      :style="{
        width: isCollapsed ? '0px' : `${sidebarWidth}px`,
        opacity: isCollapsed ? 0 : 1,
        pointerEvents: isCollapsed ? 'none' : 'auto'
      }"
    >
      <FileTree
        v-if="!isCollapsed"
        ref="fileTreeRef"
        :host="session.host"
        :session-id="session.sftpSessionId || session.id"
        :refresh-signal="refreshSignal"
        :auto-follow="session.autoFollowTerminal || false"
        :remote-monitor-open="session.remoteMonitorOpen || false"
        :terminal-cwd="currentActiveTabCwd"
        @update:auto-follow="(val) => session.autoFollowTerminal = val"
        @update:remote-monitor-open="(val) => session.remoteMonitorOpen = val"
        @toggle-remote-monitor="handleToggleRemoteMonitor"
        @manual-sync="handleManualSync"
        @toggle-collapse="toggleCollapse"
        @open-in-terminal="handleOpenInTerminal"
        @edit-file="(path) => emit('edit-file', path)"
        @download-file="(file, path) => emit('download-file', file, path)"
        @upload-trigger="(dir) => emit('upload-trigger', dir)"
        @toast="(msg, type) => emit('toast', msg, type)"
      />
    </aside>

    <!-- 2. 中间：可拖拽调整宽度的 Splitter 分割条 -->
    <div
      v-if="!isCollapsed"
      class="w-1.5 h-full cursor-col-resize hover:bg-brand-500/50 active:bg-brand-500 transition-colors shrink-0 z-20 flex items-center justify-center group -ml-0.5"
      @mousedown="startResize"
      title="拖拽调节宽度 (双击恢复默认 280px)"
      @dblclick="resetWidth"
    >
      <div class="w-0.5 h-6 bg-slate-600 group-hover:bg-brand-300 rounded-full transition-colors"></div>
    </div>

    <!-- 3. 右侧：交互式终端 (单会话支持多终端 Tab) -->
    <main class="flex-1 h-full overflow-hidden flex flex-col relative bg-[#0c0e14]">
      <TerminalView
        ref="terminalRef"
        :tabs="session.terminalTabs"
        :active-tab-index="session.activeTerminalIndex"
        :sidebar-collapsed="isCollapsed"
        @switch-tab="(idx) => emit('switch-terminal-tab', idx)"
        @close-tab="(idx) => emit('close-terminal-tab', idx)"
        @add-tab="emit('add-terminal-tab')"
        @toggle-sidebar="toggleCollapse"
        @duplicate-tab="(idx) => emit('duplicate-terminal-tab', idx)"
        @reconnect-tab="(idx) => emit('reconnect-terminal-tab', idx)"
        @rename-tab="(idx) => emit('rename-terminal-tab', idx)"
        @copy-tab-info="(idx, kind) => emit('copy-terminal-info', idx, kind)"
        @close-other-tabs="(idx) => emit('close-other-terminal-tabs', idx)"
        @close-tabs-after="(idx) => emit('close-terminal-tabs-after', idx)"
        @session-closed="(tabId, reason) => emit('session-closed', tabId, reason)"
        @terminal-cwd-change="handleTerminalCwdChange"
        @terminal-enter="handleTerminalEnter"
      />

      <!-- 4. 底部远程监控信息栏 (对标 MobaXterm 底部服务器资源监控) -->
      <RemoteMonitorBar
        v-if="session.remoteMonitorOpen"
        :metrics="remoteMetrics"
        :loading="metricsLoading"
        :default-hostname="session.host.host"
        @close="handleCloseRemoteMonitor"
      />
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, nextTick } from 'vue'
import FileTree from '@/components/FileTree.vue'
import TerminalView from '@/components/TerminalView.vue'
import RemoteMonitorBar from '@/components/RemoteMonitorBar.vue'
import type { HostSession, RemoteSystemMetrics, SftpFileItem } from '@/types'
import { getRemoteMetricsApi } from '@/api/tauri'

const props = defineProps<{
  session: HostSession
  refreshSignal?: number
  isActive?: boolean
}>()

const emit = defineEmits<{
  (e: 'switch-terminal-tab', index: number): void
  (e: 'close-terminal-tab', index: number): void
  (e: 'add-terminal-tab'): void
  (e: 'duplicate-terminal-tab', index: number): void
  (e: 'reconnect-terminal-tab', index: number): void
  (e: 'rename-terminal-tab', index: number): void
  (e: 'copy-terminal-info', index: number, kind: 'ip' | 'login' | 'ssh-command'): void
  (e: 'close-other-terminal-tabs', index: number): void
  (e: 'close-terminal-tabs-after', index: number): void
  (e: 'session-closed', tabId: string, reason: string): void
  (e: 'edit-file', path: string): void
  (e: 'download-file', file: SftpFileItem, path: string): void
  (e: 'upload-trigger', dir: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const terminalRef = ref<InstanceType<typeof TerminalView> | null>(null)
const fileTreeRef = ref<InstanceType<typeof FileTree> | null>(null)

// 远端监控状态与数据
const remoteMetrics = ref<RemoteSystemMetrics | null>(null)
const metricsLoading = ref(false)
let monitorTimer: any = null

const isCollapsed = ref(props.session.sidebarCollapsed || false)
const sidebarWidth = ref(props.session.sidebarWidth || 280)

const currentActiveTabCwd = computed(() => {
  return props.session.terminalTabs[props.session.activeTerminalIndex]?.cwd || ''
})

let enterDebounceTimer: any = null

const handleTerminalCwdChange = (tabId: string, cwd: string) => {
  const activeTab = props.session.terminalTabs[props.session.activeTerminalIndex]
  if (activeTab && activeTab.id === tabId) {
    if (props.session.autoFollowTerminal) {
      const current = fileTreeRef.value?.getCurrentPath()
      if (current !== cwd) {
        fileTreeRef.value?.jumpToPath(cwd)
      }
    }
  }
}

const handleTerminalEnter = (tabId: string) => {
  if (!props.session.autoFollowTerminal) return
  const activeTab = props.session.terminalTabs[props.session.activeTerminalIndex]
  if (!activeTab || activeTab.id !== tabId) return

  clearTimeout(enterDebounceTimer)
  enterDebounceTimer = setTimeout(async () => {
    await performAutoSync()
  }, 350)
}

const performAutoSync = async () => {
  if (!terminalRef.value) return
  try {
    const cwd = await terminalRef.value.getCurrentCwd()
    if (cwd) {
      const current = fileTreeRef.value?.getCurrentPath()
      if (current !== cwd) {
        await fileTreeRef.value?.jumpToPath(cwd)
      }
    }
  } catch (e) {
    console.warn('自动跟随获取终端目录失败:', e)
  }
}

const handleManualSync = async () => {
  if (!terminalRef.value) return
  try {
    const cwd = await terminalRef.value.getCurrentCwd()
    if (cwd) {
      const current = fileTreeRef.value?.getCurrentPath()
      if (current === cwd) {
        emit('toast', `工作区已处于当前终端目录: ${cwd}`, 'info')
      } else {
        await fileTreeRef.value?.jumpToPath(cwd)
        emit('toast', `已同步至终端目录: ${cwd}`, 'success')
      }
    } else {
      emit('toast', '未能获取到当前终端目录', 'warning')
    }
  } catch (e: any) {
    emit('toast', `目录同步失败: ${e?.message || e}`, 'error')
  }
}

watch(
  () => props.session.activeTerminalIndex,
  () => {
    if (props.session.autoFollowTerminal) {
      nextTick(() => {
        performAutoSync()
      })
    }
  }
)

watch(
  () => props.session.autoFollowTerminal,
  (enabled) => {
    if (enabled) {
      nextTick(() => {
        performAutoSync()
      })
    }
  }
)

watch(
  () => props.isActive,
  async (active) => {
    if (active) {
      await nextTick()
      requestAnimationFrame(() => {
        terminalRef.value?.fit()
        terminalRef.value?.focus()
      })
    }
  }
)

const toggleCollapse = () => {
  isCollapsed.value = !isCollapsed.value
  props.session.sidebarCollapsed = isCollapsed.value
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

const resetWidth = () => {
  sidebarWidth.value = 280
  props.session.sidebarWidth = 280
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

// 远程监控定时轮询与控制
const fetchRemoteMetrics = async () => {
  const activeTab = props.session.terminalTabs[props.session.activeTerminalIndex]
  const sessionId = activeTab?.id || props.session.sftpSessionId || props.session.id
  if (!sessionId) return

  try {
    metricsLoading.value = true
    const res = await getRemoteMetricsApi(sessionId)
    if (res) {
      remoteMetrics.value = res
    }
  } catch (e) {
    console.warn('获取远程监控指标失败:', e)
  } finally {
    metricsLoading.value = false
  }
}

const startMonitoring = () => {
  stopMonitoring()
  fetchRemoteMetrics()
  monitorTimer = setInterval(() => {
    if (props.isActive && props.session.remoteMonitorOpen) {
      fetchRemoteMetrics()
    }
  }, 2500)
}

const stopMonitoring = () => {
  if (monitorTimer) {
    clearInterval(monitorTimer)
    monitorTimer = null
  }
}

const handleToggleRemoteMonitor = () => {
  props.session.remoteMonitorOpen = !props.session.remoteMonitorOpen
  if (props.session.remoteMonitorOpen) {
    startMonitoring()
  } else {
    stopMonitoring()
  }
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

const handleCloseRemoteMonitor = () => {
  props.session.remoteMonitorOpen = false
  stopMonitoring()
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

watch(
  () => [props.isActive, props.session.remoteMonitorOpen],
  ([active, open]) => {
    if (active && open) {
      startMonitoring()
    } else {
      stopMonitoring()
    }
    nextTick(() => {
      terminalRef.value?.fit()
    })
  },
  { immediate: true }
)

// 拖拽调整宽度逻辑
let isResizing = false
let startX = 0
let startWidth = 280

const onMouseMove = (e: MouseEvent) => {
  if (!isResizing) return
  const deltaX = e.clientX - startX
  const newWidth = Math.max(180, Math.min(680, startWidth + deltaX))
  sidebarWidth.value = newWidth
  props.session.sidebarWidth = newWidth
}

const onMouseUp = () => {
  if (!isResizing) return
  isResizing = false
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

const startResize = (e: MouseEvent) => {
  isResizing = true
  startX = e.clientX
  startWidth = sidebarWidth.value
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
}

onBeforeUnmount(() => {
  stopMonitoring()
  clearTimeout(enterDebounceTimer)
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
})

// 深度联动：在右侧终端中打开指定目录
const handleOpenInTerminal = (path: string) => {
  if (!terminalRef.value) return
  // 转义单双引号，安全 cd
  const sanitized = path.replace(/"/g, '\\"')
  terminalRef.value.sendCommand(`cd "${sanitized}"\n`)
  emit('toast', `已在终端定位至: ${path}`, 'info')
}
</script>
