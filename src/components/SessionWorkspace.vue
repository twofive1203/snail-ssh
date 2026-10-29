<template>
  <div class="flex-1 flex h-full w-full overflow-hidden relative select-none bg-surface-base">
    <!-- 模式 A：独立全屏 SFTP 专业视图 -->
    <SftpView
      v-if="session.workspaceMode === 'sftp'"
      ref="sftpViewRef"
      :hosts="hosts"
      :current-host="session.host"
      :active-session-id="session.sftpSessionId || session.id"
      :refresh-signal="refreshSignal"
      :initial-path="session.currentPath || '~'"
      @switch-to-terminal="handleSwitchToTerminal"
      @open-in-terminal="handleOpenInTerminal"
      @select-host="(h) => emit('select-host', h)"
      @update:current-path="(path) => (session.currentPath = path)"
      @edit-file="(path) => emit('edit-file', path)"
      @download-file="(file, path) => emit('download-file', file, path)"
      @download-files="(refs) => emit('download-files', refs)"
      @upload-trigger="(dir) => emit('upload-trigger', dir)"
      @toast="(msg, type) => emit('toast', msg, type)"
    />

    <!-- 模式 B：终端协同分栏工作区 -->
    <template v-else>
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
          :auto-follow="activeFollow"
          :remote-monitor-open="session.remoteMonitorOpen || false"
          :terminal-cwd="currentActiveTabCwd"
          :group-id="session.host.group"
          :initial-path="session.currentPath || '~'"
          @update:current-path="(path) => (session.currentPath = path)"
          @update:auto-follow="setActiveFollow"
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
          :active="!!isActive"
          :host-key="terminalHostKey"
          :group-id="session.host.group"
          @switch-tab="(idx) => emit('switch-terminal-tab', idx)"
          @close-tab="(idx) => emit('close-terminal-tab', idx)"
          @add-tab="emit('add-terminal-tab')"
          @toggle-sidebar="toggleCollapse"
          @jump-to-sftp="handleSwitchToSftp"
          @open-tab-sftp="(idx) => handleSwitchToSftp(idx)"
          @duplicate-tab="(idx) => emit('duplicate-terminal-tab', idx)"
          @reconnect-tab="(idx) => emit('reconnect-terminal-tab', idx)"
          @rename-tab="(idx) => emit('rename-terminal-tab', idx)"
          @copy-tab-info="(idx, kind) => emit('copy-terminal-info', idx, kind)"
          @close-other-tabs="(idx) => emit('close-other-terminal-tabs', idx)"
          @close-tabs-after="(idx) => emit('close-terminal-tabs-after', idx)"
          @session-closed="(tabId, reason) => emit('session-closed', tabId, reason)"
          @terminal-cwd-change="handleTerminalCwdChange"
          @terminal-enter="handleTerminalEnter"
          @toast="(msg, type) => emit('toast', msg, type)"
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
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, nextTick } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import FileTree from '@/components/FileTree.vue'
import TerminalView from '@/components/TerminalView.vue'
import RemoteMonitorBar from '@/components/RemoteMonitorBar.vue'
import SftpView from '@/components/SftpView.vue'
import type { HostItem, HostSession, RemoteFileRef, RemoteSystemMetrics, SftpFileItem } from '@/types'
import { getMetricsTargetApi, getRemoteMetricsApi, isTauri, onSshMetrics, setMetricsTargetApi } from '@/api/tauri'

const props = defineProps<{
  session: HostSession
  hosts?: HostItem[]
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
  (e: 'download-files', refs: RemoteFileRef[]): void
  (e: 'upload-trigger', dir: string): void
  (e: 'select-host', host: HostItem): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const terminalRef = ref<InstanceType<typeof TerminalView> | null>(null)
const fileTreeRef = ref<InstanceType<typeof FileTree> | null>(null)
const sftpViewRef = ref<InstanceType<typeof SftpView> | null>(null)

// 命令历史按 "user@host:port" 归属：比 hostId 稳定，临时会话/重新导入资产后历史不丢
const terminalHostKey = computed(
  () => `${props.session.host.username}@${props.session.host.host}:${props.session.host.port}`
)

// 远端监控状态与数据
const remoteMetrics = ref<RemoteSystemMetrics | null>(null)
const metricsLoading = ref(false)
// 当前指标订阅的注销句柄；Tauri 下指标由后端推事件，前端不再定时拉取
let metricsUnlisten: UnlistenFn | null = null
// 浏览器预览没有后端事件，只能用轻量定时器兜底，否则 demo 的监控条永远空白
let previewTimer: number | null = null
// 切换标签/开关面板很快时，靠递增的令牌丢弃旧订阅的回写，避免旧会话数据盖掉新会话
let monitorToken = 0
// 本组件真正下发到后端的采样目标；卸载时据此判断是否由自己收尾清空
let ownedMetricsTarget: string | null = null
// 每个会话上一帧指标：切回标签时先用缓存显示，避免等下一个采样周期（≤2.5s）才出数据。
// 只缓存本组件见过的会话，随会话关闭回收，不会无限增长。
const metricsBySession = new Map<string, RemoteSystemMetrics>()

const isCollapsed = ref(props.session.sidebarCollapsed || false)
const sidebarWidth = ref(props.session.sidebarWidth || 280)

const currentActiveTabCwd = computed(() => {
  return props.session.terminalTabs[props.session.activeTerminalIndex]?.cwd || ''
})

const activeTerminalTab = computed(() => props.session.terminalTabs[props.session.activeTerminalIndex])

// 指标采样目标沿用现有取值顺序：活动终端标签的真实 SSH 会话优先
const monitorSessionId = computed(
  () => activeTerminalTab.value?.id || props.session.sftpSessionId || props.session.id
)

// 自动跟随挂在当前终端标签页上，同一会话内各标签页可独立开关
const activeFollow = computed(() => activeTerminalTab.value?.autoFollowTerminal ?? false)

// 文件树工具栏的开关只作用于当前标签页；打开时立即按该标签页目录同步一次
const setActiveFollow = (val: boolean) => {
  const tab = activeTerminalTab.value
  if (!tab) return
  tab.autoFollowTerminal = val
  if (val) {
    nextTick(() => performAutoSync())
  }
}

let enterDebounceTimer: any = null

const handleTerminalCwdChange = (tabId: string, cwd: string) => {
  const tab = activeTerminalTab.value
  if (tab && tab.id === tabId) {
    if (tab.autoFollowTerminal) {
      const current = fileTreeRef.value?.getCurrentPath()
      if (current !== cwd) {
        fileTreeRef.value?.jumpToPath(cwd)
      }
    }
  }
}

const handleTerminalEnter = (tabId: string) => {
  const tab = activeTerminalTab.value
  if (!tab || !tab.autoFollowTerminal || tab.id !== tabId) return

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
    // 丢弃上一次回车遗留的防抖任务，避免切到新标签页后用它的目录误跳
    clearTimeout(enterDebounceTimer)
    enterDebounceTimer = null
    if (activeFollow.value) {
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

// 远端监控：订阅后端指标事件 + 告知后端采样目标
const teardownMonitoring = () => {
  monitorToken += 1
  if (metricsUnlisten) {
    const unlisten = metricsUnlisten
    metricsUnlisten = null
    unlisten()
  }
  if (previewTimer !== null) {
    clearInterval(previewTimer)
    previewTimer = null
  }
}

// 浏览器预览的兜底轮询：只在非 Tauri 环境启动，桌面端完全交给后端事件
const startPreviewPolling = (sessionId: string, token: number) => {
  const poll = async () => {
    try {
      const res = await getRemoteMetricsApi(sessionId)
      if (token !== monitorToken) return
      if (res) metricsBySession.set(sessionId, res)
      remoteMetrics.value = res
      metricsLoading.value = false
    } catch (e) {
      console.warn('获取远程监控指标失败:', e)
      if (token === monitorToken) metricsLoading.value = false
    }
  }
  void poll()
  previewTimer = window.setInterval(poll, 2500)
}

// 采样目标是后端全局状态：只在自己仍持有时才清空，
// 否则切标签这类同一 tick 内出现的「先接管再释放」会互相抹掉目标
const releaseMetricsTarget = async (ownedSessionId: string | null) => {
  ownedMetricsTarget = null
  if (!ownedSessionId) return
  await nextTick()
  if (getMetricsTargetApi() === ownedSessionId) await setMetricsTargetApi(null)
}

const syncMonitoring = async () => {
  teardownMonitoring()
  const token = monitorToken
  // 只有「活动且面板打开」才采样；切走的会话必须让后端停下，否则隐藏会话也在跑远程命令
  const sessionId = props.isActive && props.session.remoteMonitorOpen ? monitorSessionId.value : null
  if (!sessionId) {
    metricsLoading.value = false
    await releaseMetricsTarget(ownedMetricsTarget)
    return
  }

  // 有上一帧缓存就直接显示（切换标签/关掉再打开面板不闪空白），没有才进 loading
  remoteMetrics.value = metricsBySession.get(sessionId) ?? null
  metricsLoading.value = !remoteMetrics.value
  const unlisten = await onSshMetrics(sessionId, (metrics) => {
    if (token !== monitorToken) return
    metricsBySession.set(sessionId, metrics)
    remoteMetrics.value = metrics
    metricsLoading.value = false
  })
  // await 期间可能又发生一次切换，这次订阅已过期就当场收掉，不留泄漏
  if (token !== monitorToken) {
    unlisten()
    return
  }
  metricsUnlisten = unlisten
  ownedMetricsTarget = sessionId
  await setMetricsTargetApi(sessionId)
  // await 期间又切走/卸载：本次下发可能落在新目标之后，按归属补发一次清空
  if (token !== monitorToken) {
    if (ownedMetricsTarget === sessionId) await releaseMetricsTarget(sessionId)
    return
  }
  if (!isTauri()) {
    startPreviewPolling(sessionId, token)
  }
}

const handleToggleRemoteMonitor = () => {
  // 重采样由下面监听 remoteMonitorOpen 的 watch 统一处理
  props.session.remoteMonitorOpen = !props.session.remoteMonitorOpen
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

const handleCloseRemoteMonitor = () => {
  props.session.remoteMonitorOpen = false
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

// 三个依赖都要监听：开关面板、切走标签页、会话内切换终端标签都会改变采样目标
watch(
  () => [props.isActive, props.session.remoteMonitorOpen, monitorSessionId.value],
  () => {
    void syncMonitoring()
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
  // 卸载（关闭会话）必须注销监听；但采样目标只在自己仍持有时清空：
  // 关闭活动会话时新会话可能已抢先接管目标，一律发 null 会把它抹掉
  const ownedSessionId = ownedMetricsTarget
  teardownMonitoring()
  void releaseMetricsTarget(ownedSessionId)
  // 会话关闭后缓存永远不会再被读到，留着只是占内存
  metricsBySession.clear()
  clearTimeout(enterDebounceTimer)
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
})

// 模式切换与联动
const handleSwitchToSftp = (tabIndex?: number) => {
  props.session.workspaceMode = 'sftp'
  const targetTab = typeof tabIndex === 'number'
    ? props.session.terminalTabs[tabIndex]
    : activeTerminalTab.value
  const cwd = targetTab?.cwd
  if (cwd && cwd.startsWith('/')) {
    props.session.currentPath = cwd
    nextTick(() => {
      sftpViewRef.value?.jumpToPath(cwd)
    })
  }
}

const handleSwitchToTerminal = () => {
  props.session.workspaceMode = 'split'
  nextTick(() => {
    terminalRef.value?.fit()
  })
}

// 深度联动：在右侧终端中打开指定目录
const handleOpenInTerminal = (path: string) => {
  if (props.session.workspaceMode === 'sftp') {
    handleSwitchToTerminal()
  }
  nextTick(() => {
    if (!terminalRef.value) return
    const sanitized = path.replace(/"/g, '\\"')
    terminalRef.value.sendCommand(`cd "${sanitized}"\n`)
    emit('toast', `已在终端定位至: ${path}`, 'info')
  })
}
</script>
