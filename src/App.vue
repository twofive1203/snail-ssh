<template>
  <div class="h-screen w-screen flex flex-col bg-surface-base text-slate-100 font-sans antialiased overflow-hidden select-none">
    <!-- 1. 现代化顶栏 -->
    <TitleBar
      :active-transfers-count="activeTransfersCount"
      :transfer-speed="currentTransferSpeed"
      @open-palette="paletteOpen = true"
      @toggle-transfers="transfersOpen = !transfersOpen"
      @open-new-host="openNewHostModal"
      @open-settings="triggerToast('设置中心尚未实现', 'info')"
      @close-window="handleCloseWindow"
    />

    <!-- 2. 主体工作区 -->
    <div class="flex flex-1 overflow-hidden relative">
      <!-- 左侧边栏 -->
      <Sidebar
        :active-view="activeView"
        :selected-group="selectedGroup"
        :total-hosts="hosts.length"
        :active-sessions-count="terminalTabs.filter(t => t.connected).length"
        :group-counts="groupCounts"
        :recent-hosts="hosts.slice(0, 2)"
        @switch-view="switchView"
        @filter-group="val => selectedGroup = val"
        @connect-host="handleConnectSsh"
        @add-group="triggerToast('自定义分组尚未实现', 'info')"
        @open-vault="triggerToast('SSH 密钥保险箱尚未实现', 'info')"
        @open-snippets="triggerToast('命令片段库尚未实现', 'info')"
        @open-tunnels="triggerToast('端口转发隧道尚未实现', 'info')"
      />

      <!-- 主视图容器 -->
      <main class="flex-1 flex flex-col overflow-hidden bg-surface-base relative">
        <!-- 视图 1: 服务器资产管理 -->
        <HostManager
          v-if="activeView === 'hosts'"
          :hosts="hosts"
          :selected-group="selectedGroup"
          @connect-ssh="handleConnectSsh"
          @connect-sftp="handleConnectSftp"
          @edit-host="handleEditHost"
          @delete-host="handleDeleteHost"
          @open-new-host="openNewHostModal"
          @import-export="triggerToast('主机导入/导出尚未实现', 'info')"
        />

        <!-- 视图 2: SSH 终端 -->
        <TerminalView
          v-else-if="activeView === 'terminal'"
          :tabs="terminalTabs"
          :active-tab-index="activeTabIndex"
          @switch-tab="idx => activeTabIndex = idx"
          @close-tab="handleCloseTerminalTab"
          @add-tab="handleAddTerminalTab"
          @jump-to-sftp="handleJumpToCurrentSftp"
          @duplicate-tab="handleDuplicateTerminalTab"
          @reconnect-tab="handleReconnectTerminalTab"
          @rename-tab="handleRenameTerminalTab"
          @copy-tab-info="handleCopyTabInfo"
          @open-tab-sftp="handleOpenTabSftp"
          @close-other-tabs="handleCloseOtherTerminalTabs"
          @close-tabs-after="handleCloseTerminalTabsAfter"
          @session-closed="handleSessionClosed"
        />

        <!-- 视图 3: SFTP 文件管理 -->
        <SftpView
          v-else-if="activeView === 'sftp'"
          :hosts="hosts"
          :current-host="currentSftpHost"
          :active-session-id="currentSftpSessionId"
          :refresh-signal="sftpRefreshSignal"
          @select-host="handleConnectSftp"
          @edit-file="handleOpenEditor"
          @download-file="handleDownloadFile"
          @download-files="handleDownloadFiles"
          @upload-trigger="handleUploadTrigger"
          @open-in-terminal="handleOpenInTerminal"
          @toast="triggerToast"
        />
      </main>

      <!-- 传输任务浮动抽屉 -->
      <TransferDrawer
        :open="transfersOpen"
        :tasks="transferTasks"
        @close="transfersOpen = false"
        @clear-completed="handleClearCompletedTasks"
        @remove-task="handleRemoveTask"
      />
    </div>

    <!-- 弹窗 1: 新建/编辑服务器 -->
    <HostModal
      :open="hostModalOpen"
      :host-data="editingHost"
      @close="hostModalOpen = false"
      @save="handleSaveHost"
      @toast="triggerToast"
    />

    <!-- 弹窗 2: 在线文件代码编辑器 -->
    <EditorModal
      :open="editorOpen"
      :file-path="editingFilePath"
      :session-id="currentSftpSessionId || currentSessionId"
      @close="editorOpen = false"
      @toast="triggerToast"
    />

    <!-- 弹窗 3: 全局命令面板 (Ctrl+K) -->
    <CommandPalette
      :open="paletteOpen"
      :hosts="hosts"
      @close="paletteOpen = false"
      @select-host="handleConnectSsh"
      @action="handlePaletteAction"
    />

    <!-- 弹窗 4: 应用内确认 / 文本输入（替代原生 confirm / prompt） -->
    <DialogHost />

    <!-- 全局 Toast 提示组件 -->
    <Toast :messages="toastList" @dismiss="dismissToast" />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import Sidebar from '@/components/Sidebar.vue'
import HostManager from '@/components/HostManager.vue'
import TerminalView from '@/components/TerminalView.vue'
import SftpView from '@/components/SftpView.vue'
import TransferDrawer from '@/components/TransferDrawer.vue'
import CommandPalette from '@/components/CommandPalette.vue'
import HostModal from '@/components/HostModal.vue'
import EditorModal from '@/components/EditorModal.vue'
import DialogHost from '@/components/DialogHost.vue'
import Toast from '@/components/Toast.vue'
import { confirmDialog, promptDialog, dialogState } from '@/composables/useDialog'
import type { HostItem, RemoteFileRef, SftpFileItem, TerminalTab, TransferTask, ToastMessage } from '@/types'
import {
  getHostsApi,
  saveHostApi,
  deleteHostApi,
  createSshSessionApi,
  closeSshSessionApi,
  writeSshSessionApi,
  probeHostInfoApi,
  sftpReadDirApi,
  sftpUploadFileApi,
  sftpDownloadFileApi,
  sftpDownloadDirApi,
  onSftpProgress,
  pickLocalFile,
  pickSavePath,
  pickDirectory,
  isTauri
} from '@/api/tauri'

// 核心视图状态
const activeView = ref<'hosts' | 'terminal' | 'sftp'>('hosts')
const selectedGroup = ref<string>('all')

// 数据状态
const hosts = ref<HostItem[]>([])
// 浏览器预览保留演示标签页；桌面端从空会话开始，避免出现无法写入的假会话
const demoTabs: TerminalTab[] = [
  {
    id: 'tab-0',
    hostId: 'prod-k8s-master-01',
    title: 'prod-k8s-master-01',
    sub: 'root@47.98.112.24',
    connected: true
  },
  {
    id: 'tab-1',
    hostId: 'aliyun-hk-edge',
    title: 'aliyun-hk-edge',
    sub: 'deploy@43.154.201.89',
    connected: true
  }
]
const terminalTabs = ref<TerminalTab[]>(isTauri() ? [] : demoTabs)
const activeTabIndex = ref(0)
const currentSftpHost = ref<HostItem | undefined>(undefined)
const currentSftpSessionId = ref<string>('')
// 同一主机可多开：每次连接都新建独立会话，tab.id 即 sessionId
// 为同一主机的多个 tab 编号，便于区分同名会话
const tabSeqByHost = new Map<string, number>()
// 该主机没有终端 tab 时，为其只开 SFTP 而建立的会话（切走时释放，避免连接泄露）
let sftpOnlyHostId = ''
let sftpOnlySessionId = ''

// 传输队列只承载真实任务，启动时为空（避免展示无法对应真实连接的假进度）
const transfersOpen = ref(false)
const transferTasks = ref<TransferTask[]>([])
// 传输完成后自增，通知 SftpView 刷新当前目录，避免用户以为上传失败了
const sftpRefreshSignal = ref(0)

// 弹窗状态
const paletteOpen = ref(false)
const hostModalOpen = ref(false)
const editingHost = ref<HostItem | null>(null)
const editorOpen = ref(false)
const editingFilePath = ref('')

// Toast 状态
const toastList = ref<ToastMessage[]>([])

// 计算属性
const activeTransfersCount = computed(() => {
  return transferTasks.value.filter(t => t.status === 'running').length
})

const currentTransferSpeed = computed(() => {
  const running = transferTasks.value.find(t => t.status === 'running')
  return running ? running.speed : '0 B/s'
})

const groupCounts = computed(() => {
  const counts: Record<string, number> = {}
  hosts.value.forEach(h => {
    counts[h.group] = (counts[h.group] || 0) + 1
  })
  return counts
})

const currentSessionId = computed(() => {
  return terminalTabs.value[activeTabIndex.value]?.id
})

// 动作方法
const triggerToast = (text: string, type: 'info' | 'success' | 'warning' | 'error' = 'info') => {
  const item: ToastMessage = { id: 'toast-' + Date.now() + Math.random(), text, type }
  toastList.value.push(item)
  // 错误信息保留在屏幕上直到用户手动关闭，避免“报错一闪而过”无从排查
  if (type !== 'error') {
    setTimeout(() => dismissToast(item.id), 3200)
  }
}

const dismissToast = (id: string) => {
  toastList.value = toastList.value.filter(t => t.id !== id)
}

const switchView = (v: 'hosts' | 'terminal' | 'sftp') => {
  activeView.value = v
  // 直接进入 SFTP 视图时，先确保该主机已建立会话
  if (v === 'sftp' && !currentSftpSessionId.value && currentSftpHost.value) {
    handleConnectSftp(currentSftpHost.value)
  }
}

// 加载服务器数据
const loadHosts = async () => {
  hosts.value = await getHostsApi()
  if (!currentSftpHost.value && hosts.value.length > 0) {
    currentSftpHost.value = hosts.value[0]
  }
}

// ==========================================
// SSH / SFTP 会话建立（同一主机允许并行多开会话）
// ==========================================
const probingHosts = new Set<string>()

// 把后端返回的 HOST_KEY_UNKNOWN|指纹 转成用户可确认的信任流程
const openSession = async (host: HostItem, cols = 80, rows = 24): Promise<string | null> => {
  try {
    const res = await createSshSessionApi(host, cols, rows, false)
    return res.sessionId
  } catch (e: any) {
    const msg = String(e?.message || e)
    if (msg.startsWith('HOST_KEY_MISMATCH|')) {
      const parts = msg.split('|')
      const newFp = parts[1] || '未知'
      const oldFps = (parts[2] || '').split(';').filter(Boolean)
      const proceed = await confirmDialog({
        title: '主机密钥不一致',
        variant: 'danger',
        message:
          `${host.name}（${host.host}:${host.port}）的主机密钥与 ~/.ssh/known_hosts 记录不一致。\n` +
          '这可能是服务器重装/更换了密钥，也可能是中间人攻击。',
        detail:
          `本次服务器提供的指纹：\n${newFp}\n\n已知记录：\n${oldFps.join('\n') || '（无法读取）'}`,
        confirmText: '确认更换并继续',
        cancelText: '取消连接'
      })
      if (!proceed) {
        triggerToast('已取消连接：主机密钥不一致', 'warning')
        return null
      }
      try {
        const res = await createSshSessionApi(host, cols, rows, true)
        return res.sessionId
      } catch (e2: any) {
        triggerToast(`SSH 连接失败: ${e2?.message || e2}`, 'error')
        return null
      }
    }
    const unknown = msg.match(/HOST_KEY_UNKNOWN\|(.+)/)
    if (unknown) {
      const trust = await confirmDialog({
        title: '首次连接该主机',
        variant: 'warning',
        message: `首次连接 ${host.name}（${host.host}:${host.port}）。\n是否信任该主机密钥并写入 ~/.ssh/known_hosts？`,
        detail: `主机密钥指纹：\n${unknown[1]}`,
        confirmText: '信任并继续',
        cancelText: '取消连接'
      })
      if (!trust) {
        triggerToast('已取消连接：未信任主机密钥', 'warning')
        return null
      }
      try {
        const res = await createSshSessionApi(host, cols, rows, true)
        return res.sessionId
      } catch (e2: any) {
        triggerToast(`SSH 连接失败: ${e2?.message || e2}`, 'error')
        return null
      }
    }
    triggerToast(`SSH 连接失败: ${msg}`, 'error')
    return null
  }
}

// 释放仅为 SFTP 建立的专用会话
const releaseSftpOnlySession = () => {
  if (sftpOnlySessionId && isTauri()) {
    closeSshSessionApi(sftpOnlySessionId).catch(() => {})
  }
  sftpOnlyHostId = ''
  sftpOnlySessionId = ''
}

// SFTP 需要一份指向目标主机的会话：
// 优先复用当前活动 tab（就是该主机时），其次该主机任一 tab，
// 都没有时才建立一份仅供 SFTP 使用的会话
const resolveSftpSession = async (host: HostItem): Promise<string | null> => {
  const activeTab = terminalTabs.value[activeTabIndex.value]
  if (activeTab && activeTab.hostId === host.id && activeTab.connected) {
    return activeTab.id
  }
  const anyTab = terminalTabs.value.find(t => t.hostId === host.id && t.connected)
  if (anyTab) return anyTab.id

  if (sftpOnlySessionId && sftpOnlyHostId === host.id) return sftpOnlySessionId

  releaseSftpOnlySession()
  const sessionId = await openSession(host)
  if (sessionId) {
    sftpOnlyHostId = host.id
    sftpOnlySessionId = sessionId
  }
  return sessionId
}

// 连接成功后异步采集真实指标（发行版 / CPU / 内存 / 磁盘 / 延迟）
const refreshHostMetrics = async (host: HostItem, sessionId: string) => {
  if (!isTauri() || probingHosts.has(host.id)) return
  probingHosts.add(host.id)
  try {
    const info = await probeHostInfoApi(sessionId)
    const updated: HostItem = {
      ...host,
      os: info.os || host.os,
      osName: info.osName || host.osName,
      cpu: info.cpu ?? host.cpu,
      mem: info.mem ?? host.mem,
      disk: info.disk ?? host.disk,
      ping: info.pingMs != null ? `${info.pingMs}ms` : host.ping,
      status: 'online',
      updatedAt: '刚刚'
    }
    const idx = hosts.value.findIndex(h => h.id === host.id)
    if (idx >= 0) hosts.value[idx] = updated
    if (currentSftpHost.value?.id === host.id) currentSftpHost.value = updated
    await saveHostApi(updated)
  } catch (e) {
    console.warn('probe_host_info failed:', e)
  } finally {
    probingHosts.delete(host.id)
  }
}

// SSH 直连：每次都在新标签页中建立独立会话
const handleConnectSsh = async (host: HostItem) => {
  activeView.value = 'terminal'
  triggerToast(`正在建立 SSH 会话至 ${host.name} (${host.host})...`, 'info')

  const sessionId = await openSession(host)
  if (!sessionId) {
    activeView.value = 'hosts'
    return
  }

  const seq = (tabSeqByHost.get(host.id) || 0) + 1
  tabSeqByHost.set(host.id, seq)
  terminalTabs.value.push({
    id: sessionId,
    hostId: host.id,
    title: seq > 1 ? `${host.name} (${seq})` : host.name,
    sub: `${host.username}@${host.host}`,
    connected: true
  })
  activeTabIndex.value = terminalTabs.value.length - 1

  triggerToast(`已连通 ${host.name} (SSH-2.0)`, 'success')
  refreshHostMetrics(host, sessionId)
}

// SFTP 直连：优先复用当前主机已有的会话，避免多余连接
const handleConnectSftp = async (host: HostItem) => {
  currentSftpHost.value = host

  const sessionId = await resolveSftpSession(host)
  if (!sessionId) return

  currentSftpSessionId.value = sessionId
  activeView.value = 'sftp'
  triggerToast(`已连接 ${host.name} 的 SFTP 文件系统`, 'success')
  refreshHostMetrics(host, sessionId)
}

// 远端主动断开（Shell 退出 / 链路中断）
const handleSessionClosed = (tabId: string, reason: string) => {
  const tab = terminalTabs.value.find(t => t.id === tabId)
  if (tab) tab.connected = false
  triggerToast(`会话已断开: ${reason}`, 'warning')
}

// 终端 Tab 操作
const handleCloseTerminalTab = (index: number) => {
  const tab = terminalTabs.value[index]
  if (!tab) return

  if (isTauri() && tab.id.startsWith('ssh-')) {
    closeSshSessionApi(tab.id).catch(() => {})
  }
  if (currentSftpSessionId.value === tab.id) currentSftpSessionId.value = ''

  terminalTabs.value.splice(index, 1)
  if (terminalTabs.value.length === 0) {
    activeTabIndex.value = 0
  } else if (activeTabIndex.value >= terminalTabs.value.length) {
    activeTabIndex.value = terminalTabs.value.length - 1
  }
}

const handleAddTerminalTab = () => {
  // 在当前主机上再开一个独立会话（同主机多开）
  const current = terminalTabs.value[activeTabIndex.value]
  const host = current ? hosts.value.find(h => h.id === current.hostId) : undefined
  if (host) {
    handleConnectSsh(host)
    return
  }

  // 没有可参考的当前会话时，引导用户先选择主机
  activeView.value = 'hosts'
  triggerToast('请选择一台服务器发起 SSH 连接', 'info')
}

const handleJumpToCurrentSftp = () => {
  const curTab = terminalTabs.value[activeTabIndex.value]
  if (curTab) {
    const found = hosts.value.find(h => h.id === curTab.hostId)
    if (found) {
      handleConnectSftp(found)
    }
  }
}

// ==========================================
// 终端标签右键菜单动作
// ==========================================

/** 写入系统剪贴板；WebKitGTK 等环境下 Clipboard API 不可用时退回 execCommand */
const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch {
    // 继续尝试降级方案
  }
  try {
    const el = document.createElement('textarea')
    el.value = text
    el.setAttribute('readonly', '')
    el.style.position = 'fixed'
    el.style.opacity = '0'
    document.body.appendChild(el)
    el.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(el)
    return ok
  } catch {
    return false
  }
}

/** 右键菜单动作都基于被右键的那一个标签页，而不是当前活动标签 */
const hostOfTab = (index: number) => {
  const tab = terminalTabs.value[index]
  return tab ? hosts.value.find(h => h.id === tab.hostId) : undefined
}

// 复制会话：在同一台服务器上再开一个独立标签页
const handleDuplicateTerminalTab = (index: number) => {
  const host = hostOfTab(index)
  if (!host) {
    triggerToast('该会话对应的服务器配置已不存在', 'warning')
    return
  }
  handleConnectSsh(host)
}

// 重新连接：先释放旧会话，避免同一标签反复重连后留下无法回收的连接
const handleReconnectTerminalTab = (index: number) => {
  const host = hostOfTab(index)
  if (!host) {
    triggerToast('该会话对应的服务器配置已不存在', 'warning')
    return
  }
  handleCloseTerminalTab(index)
  handleConnectSsh(host)
}

const handleRenameTerminalTab = async (index: number) => {
  const tab = terminalTabs.value[index]
  if (!tab) return
  const name = await promptDialog({
    title: '重命名标签',
    message: `为会话 ${tab.sub} 设置显示名称`,
    input: { label: '标签名称', value: tab.title, placeholder: '例如：生产主库', required: true },
    confirmText: '保存'
  })
  if (!name) return
  tab.title = name.trim()
}

const handleCopyTabInfo = async (index: number, kind: 'ip' | 'login' | 'ssh-command') => {
  const tab = terminalTabs.value[index]
  const host = hostOfTab(index)
  if (!tab || !host) {
    triggerToast('该会话对应的服务器配置已不存在', 'warning')
    return
  }
  const text = kind === 'ip'
    ? host.host
    : kind === 'login'
      ? `${host.username}@${host.host}`
      : `ssh -p ${host.port} ${host.username}@${host.host}`
  const label = kind === 'ip' ? 'IP 地址' : kind === 'login' ? '登录信息' : 'SSH 命令'
  const ok = await copyToClipboard(text)
  if (ok) {
    triggerToast(`已复制${label}: ${text}`, 'success')
  } else {
    triggerToast('复制失败：当前环境不允许访问剪贴板', 'error')
  }
}

const handleOpenTabSftp = (index: number) => {
  const host = hostOfTab(index)
  if (host) handleConnectSftp(host)
}

const handleCloseOtherTerminalTabs = (index: number) => {
  const keep = terminalTabs.value[index]
  if (!keep) return
  // 从后往前关闭，避免索引位移导致关错标签
  for (let i = terminalTabs.value.length - 1; i >= 0; i--) {
    if (i !== index) handleCloseTerminalTab(i)
  }
  activeTabIndex.value = Math.max(0, terminalTabs.value.findIndex(t => t.id === keep.id))
}

const handleCloseTerminalTabsAfter = (index: number) => {
  for (let i = terminalTabs.value.length - 1; i > index; i--) {
    handleCloseTerminalTab(i)
  }
}

// 主机增删改查
const openNewHostModal = () => {
  editingHost.value = null
  hostModalOpen.value = true
}

const handleEditHost = (host: HostItem) => {
  editingHost.value = host
  hostModalOpen.value = true
}

const handleSaveHost = async (host: HostItem) => {
  await saveHostApi(host)
  await loadHosts()
  hostModalOpen.value = false
  triggerToast(`已保存服务器: ${host.name}`, 'success')
}

const handleDeleteHost = async (host: HostItem) => {
  const confirmed = await confirmDialog({
    title: '删除服务器',
    variant: 'danger',
    message: `确定要删除服务器 ${host.name} 吗？`,
    detail: `${host.username}@${host.host}:${host.port}\n保存在系统钥匙串中的密码 / 私钥也会一并删除。`,
    confirmText: '删除',
    cancelText: '取消'
  })
  if (!confirmed) return

  await deleteHostApi(host.id)
  await loadHosts()
  triggerToast(`已移除主机: ${host.name}`, 'warning')
}

// 窗口关闭：有正在进行的传输或活动会话时先确认，避免误关丢失工作
const handleCloseWindow = async () => {
  const running = transferTasks.value.filter(t => t.status === 'running').length
  const sessions = terminalTabs.value.filter(t => t.connected).length

  if (running > 0 || sessions > 0) {
    const parts: string[] = []
    if (running > 0) parts.push(`${running} 个传输任务正在执行`)
    if (sessions > 0) parts.push(`${sessions} 个 SSH 会话仍处于连接状态`)
    const confirmed = await confirmDialog({
      title: '退出 SnailSSH',
      variant: 'warning',
      message: `当前${parts.join('，')}，确定要退出吗？`,
      confirmText: '退出',
      cancelText: '留在应用'
    })
    if (!confirmed) return
  }

  if (!isTauri()) return
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().close()
  } catch (e) {
    console.warn('Tauri window close error:', e)
  }
}

// 文件编辑与传输
const handleOpenEditor = (filePath: string) => {
  editingFilePath.value = filePath
  editorOpen.value = true
}

// ==========================================
// 文件传输（真实 SFTP 上传/下载 + 实时进度）
// ==========================================
const formatBytes = (bytes: number) => {
  if (!bytes) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const createTransferTask = (name: string, type: 'upload' | 'download', size = '准备中...'): TransferTask => {
  // 必须用 reactive 包装：任务对象的 progress/status 会被 runTransfer 持续原地更新，
  // 若放进数组的是普通对象，属性变更不会触发重渲染（进度条会永远停在 0%）。
  const task = reactive<TransferTask>({
    id: 'task-' + Date.now() + '-' + Math.floor(Math.random() * 1000),
    name,
    type,
    progress: 0,
    speed: '准备中...',
    size,
    status: 'running'
  })
  transferTasks.value.unshift(task)
  transfersOpen.value = true
  return task
}

// 本地路径拼接（保留 Windows 反斜杠）
const joinLocalPath = (dir: string, name: string) => {
  const sep = dir.includes('\\') ? '\\' : '/'
  return `${dir}${dir.endsWith(sep) ? '' : sep}${name}`
}

// 订阅后端进度事件 -> 驱动任务卡片，结束后自动退订
const runTransfer = async (task: TransferTask, start: (taskId: string) => Promise<void>) => {
  const unlisten = await onSftpProgress(task.id, (payload) => {
    task.progress = payload.progress
    task.speed = payload.speed
    task.size = `${formatBytes(payload.transferred)} / ${formatBytes(payload.total)}`
  })

  try {
    await start(task.id)
    task.progress = 100
    task.status = 'completed'
    task.speed = '传输完成'
    triggerToast(`${task.name} 已完成`, 'success')
    // 远端目录内容已变化，通知 SFTP 视图刷新，避免用户以为传输没生效
    sftpRefreshSignal.value++
  } catch (e: any) {
    task.status = 'failed'
    task.speed = `失败: ${e?.message || e}`
    triggerToast(`传输失败: ${e?.message || e}`, 'error')
  } finally {
    unlisten()
  }
}

// 浏览器预览下的进度模拟
const runMockTransfer = (task: TransferTask) => {
  const timer = setInterval(() => {
    task.progress = Math.min(100, task.progress + 25)
    task.speed = '12.4 MB/s'
    if (task.progress >= 100) {
      task.status = 'completed'
      task.speed = '传输完成'
      sftpRefreshSignal.value++
      clearInterval(timer)
    }
  }, 400)
}

const handleDownloadFile = async (file: SftpFileItem, remotePath: string) => {
  const sessionId = currentSftpSessionId.value
  if (isTauri() && !sessionId) {
    triggerToast('请先连接该主机的 SSH/SFTP 会话', 'warning')
    return
  }

  if (!isTauri()) {
    runMockTransfer(createTransferTask(`下载: ${file.name}`, 'download', file.size))
    return
  }

  // 目录：选一个本地父目录，后端递归下载并保留目录结构
  if (file.isDir) {
    const targetDir = await pickDirectory()
    if (!targetDir) return
    const localPath = joinLocalPath(targetDir, file.name)
    const task = createTransferTask(`下载目录: ${file.name}`, 'download', file.size)
    await runTransfer(task, (taskId) =>
      sftpDownloadDirApi(sessionId, remotePath, localPath, taskId)
    )
    return
  }

  const localPath = await pickSavePath(file.name)
  if (!localPath) return

  const task = createTransferTask(`下载: ${file.name}`, 'download', file.size)
  await runTransfer(task, (taskId) =>
    sftpDownloadFileApi(sessionId, remotePath, localPath, taskId)
  )
}

const handleUploadTrigger = async (remoteDir: string) => {
  const sessionId = currentSftpSessionId.value
  if (isTauri() && !sessionId) {
    triggerToast('请先连接该主机的 SSH/SFTP 会话', 'warning')
    return
  }

  const localPath = await pickLocalFile()
  if (isTauri() && !localPath) return

  const fileName = isTauri()
    ? (localPath as string).split(/[\\/]/).pop() || 'upload.bin'
    : `mock-upload-${Date.now()}.tar.gz`
  const remotePath = remoteDir === '/' ? `/${fileName}` : `${remoteDir}/${fileName}`

  if (!isTauri()) {
    runMockTransfer(createTransferTask(`上传: ${fileName} → ${remoteDir}`, 'upload'))
    return
  }

  // 后端会直接覆盖同名文件，所以覆盖前先给出明确选择，避免误覆盖生产配置
  try {
    const list = await sftpReadDirApi(sessionId, remoteDir)
    if (list.some(f => !f.isDir && f.name === fileName)) {
      const overwrite = await confirmDialog({
        title: '远端已存在同名文件',
        variant: 'warning',
        message: `是否用本地文件覆盖远端已有文件 ${fileName}？`,
        detail: remotePath,
        confirmText: '覆盖上传',
        cancelText: '取消上传'
      })
      if (!overwrite) {
        triggerToast('已取消上传，未修改远端文件', 'info')
        return
      }
    }
  } catch (e) {
    console.warn('检查远端同名文件失败：', e)
  }

  const task = createTransferTask(`上传: ${fileName} → ${remoteDir}`, 'upload')
  await runTransfer(task, (taskId) =>
    sftpUploadFileApi(sessionId, localPath as string, remotePath, taskId)
  )
}

const handleOpenInTerminal = async (path: string) => {
  if (!isTauri()) {
    activeView.value = 'terminal'
    triggerToast('浏览器预览模式无法切换远端目录', 'info')
    return
  }

  const host = currentSftpHost.value
  if (!host) {
    triggerToast('尚未选择服务器', 'warning')
    return
  }

  // 必须定位到 SFTP 所连主机的终端，而不是“当前活动标签页”，
  // 否则会把 cd 写进另一台主机的会话
  const existingIndex = terminalTabs.value.findIndex(t => t.hostId === host.id && t.connected)
  let targetTabId = existingIndex >= 0 ? terminalTabs.value[existingIndex].id : ''
  let openedNew = false

  if (!targetTabId) {
    const sessionId = await openSession(host)
    if (!sessionId) return
    const seq = (tabSeqByHost.get(host.id) || 0) + 1
    tabSeqByHost.set(host.id, seq)
    terminalTabs.value.push({
      id: sessionId,
      hostId: host.id,
      title: seq > 1 ? `${host.name} (${seq})` : host.name,
      sub: `${host.username}@${host.host}`,
      connected: true
    })
    targetTabId = sessionId
    openedNew = true
    refreshHostMetrics(host, sessionId)
  }

  activeView.value = 'terminal'
  activeTabIndex.value = Math.max(0, terminalTabs.value.findIndex(t => t.id === targetTabId))
  writeSshSessionApi(targetTabId, `cd ${shellQuote(path)}\n`).catch(() => {})
  triggerToast(openedNew ? `已新建终端并切换至 ${path}` : `已在终端切换至 ${path}`, 'info')
}

// 远端路径可能含空格/特殊字符，统一单引号包裹
const shellQuote = (value: string) => `'${value.replace(/'/g, `'\\''`)}'`

// 批量下载：选一次目标目录，逐个文件/目录复用同一套传输任务与进度事件
const handleDownloadFiles = async (refs: RemoteFileRef[]) => {
  if (refs.length === 0) return
  const sessionId = currentSftpSessionId.value
  if (isTauri() && !sessionId) {
    triggerToast('请先连接该主机的 SSH/SFTP 会话', 'warning')
    return
  }

  let targetDir = ''
  if (isTauri()) {
    const picked = await pickDirectory()
    if (!picked) return
    targetDir = picked
  }

  for (const ref of refs) {
    const label = ref.file.isDir ? `下载目录: ${ref.file.name}` : `下载: ${ref.file.name}`
    const task = createTransferTask(label, 'download', ref.file.size)
    if (!isTauri()) {
      runMockTransfer(task)
      continue
    }
    const localPath = joinLocalPath(targetDir, ref.file.name)
    await runTransfer(task, (taskId) =>
      ref.file.isDir
        ? sftpDownloadDirApi(sessionId, ref.remotePath, localPath, taskId)
        : sftpDownloadFileApi(sessionId, ref.remotePath, localPath, taskId)
    )
  }
}

const handleRemoveTask = (taskId: string) => {
  transferTasks.value = transferTasks.value.filter(t => t.id !== taskId)
}

const handleClearCompletedTasks = () => {
  const before = transferTasks.value.length
  transferTasks.value = transferTasks.value.filter(t => t.status === 'running')
  const removed = before - transferTasks.value.length
  triggerToast(removed > 0 ? `已清理 ${removed} 条已结束的传输记录` : '没有可清理的传输记录', 'info')
}

const handlePaletteAction = (act: string) => {
  if (act === 'new-host') openNewHostModal()
  if (act === 'toggle-transfers') transfersOpen.value = !transfersOpen.value
}

// 快捷键只在“非输入态”生效：终端/编辑器里 Ctrl+K、Ctrl+T、Ctrl+N 有各自的含义
// （例如终端里 Ctrl+K 是 readline 的删到行尾），不能被全局快捷键吞掉
const isTerminalOrEditorTarget = (target: EventTarget | null): boolean => {
  const el = target as HTMLElement | null
  if (!el || typeof el.closest !== 'function') return false
  if (el.tagName === 'TEXTAREA' || el.isContentEditable) return true
  return !!el.closest('.xterm')
}

// 全局快捷键注册
const setupGlobalKeybindings = () => {
  window.addEventListener('keydown', (e) => {
    if (dialogState.open) return
    const mod = e.ctrlKey || e.metaKey
    if (!mod) return
    const key = e.key.toLowerCase()
    if (isTerminalOrEditorTarget(e.target)) return

    if (key === 'k') {
      e.preventDefault()
      paletteOpen.value = true
    }
    if (key === 'n' && !e.shiftKey) {
      e.preventDefault()
      openNewHostModal()
    }
    if (key === 't') {
      e.preventDefault()
      transfersOpen.value = !transfersOpen.value
    }
  })
}

onMounted(async () => {
  await loadHosts()
  setupGlobalKeybindings()
  triggerToast('SnailSSH 桌面客户端已就绪！支持 Ctrl+K 全局搜索', 'success')
})

// 退出前释放所有后端会话，避免连接泄漏
onBeforeUnmount(() => {
  if (!isTauri()) return
  releaseSftpOnlySession()
  for (const tab of terminalTabs.value) {
    if (tab.id.startsWith('ssh-')) closeSshSessionApi(tab.id).catch(() => {})
  }
})
</script>
