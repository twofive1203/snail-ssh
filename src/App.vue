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
          @session-closed="handleSessionClosed"
        />

        <!-- 视图 3: SFTP 文件管理 -->
        <SftpView
          v-else-if="activeView === 'sftp'"
          :hosts="hosts"
          :current-host="currentSftpHost"
          :active-session-id="currentSftpSessionId"
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

    <!-- 全局 Toast 提示组件 -->
    <Toast :messages="toastList" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import Sidebar from '@/components/Sidebar.vue'
import HostManager from '@/components/HostManager.vue'
import TerminalView from '@/components/TerminalView.vue'
import SftpView from '@/components/SftpView.vue'
import TransferDrawer from '@/components/TransferDrawer.vue'
import CommandPalette from '@/components/CommandPalette.vue'
import HostModal from '@/components/HostModal.vue'
import EditorModal from '@/components/EditorModal.vue'
import Toast from '@/components/Toast.vue'
import type { HostItem, RemoteFileRef, TerminalTab, TransferTask, ToastMessage } from '@/types'
import {
  getHostsApi,
  saveHostApi,
  deleteHostApi,
  createSshSessionApi,
  closeSshSessionApi,
  writeSshSessionApi,
  probeHostInfoApi,
  sftpUploadFileApi,
  sftpDownloadFileApi,
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
// 每个主机复用一个 SSH 会话，终端与 SFTP 共用同一个 sessionId
const sessionByHost = new Map<string, string>()
const currentSftpSessionId = ref<string>('')

// 传输队列只承载真实任务，启动时为空（避免展示无法对应真实连接的假进度）
const transfersOpen = ref(false)
const transferTasks = ref<TransferTask[]>([])

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
  setTimeout(() => {
    toastList.value = toastList.value.filter(t => t.id !== item.id)
  }, 2800)
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
// SSH / SFTP 会话建立（同一主机复用同一连接）
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
      const proceed = confirm(
        `警告：${host.name}（${host.host}:${host.port}）的主机密钥与 ~/.ssh/known_hosts 记录不一致！\n\n` +
          `本次服务器提供的指纹：\n${newFp}\n\n` +
          `已知记录：\n${oldFps.join('\n') || '（无法读取）'}\n\n` +
          `这可能是服务器重装/更换了密钥，也可能是中间人攻击。\n` +
          `仅在确认服务器确实变更过密钥时确认，将替换 known_hosts 中的旧记录并继续连接。`
      )
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
      const trust = confirm(
        `首次连接 ${host.name}（${host.host}:${host.port}）。\n\n主机密钥指纹：\n${unknown[1]}\n\n是否信任并写入 ~/.ssh/known_hosts？`
      )
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

const ensureSession = async (host: HostItem): Promise<string | null> => {
  if (!isTauri()) return `mock-${host.id}`

  const existing = sessionByHost.get(host.id)
  if (existing && terminalTabs.value.some(t => t.id === existing)) return existing

  const sessionId = await openSession(host)
  if (sessionId) sessionByHost.set(host.id, sessionId)
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

// SSH 直连
const handleConnectSsh = async (host: HostItem) => {
  activeView.value = 'terminal'
  triggerToast(`正在建立 SSH 会话至 ${host.name} (${host.host})...`, 'info')

  const sessionId = await ensureSession(host)
  if (!sessionId) {
    activeView.value = 'hosts'
    return
  }

  const existingIdx = terminalTabs.value.findIndex(t => t.hostId === host.id)
  if (existingIdx >= 0) {
    activeTabIndex.value = existingIdx
  } else {
    terminalTabs.value.push({
      id: sessionId,
      hostId: host.id,
      title: host.name,
      sub: `${host.username}@${host.host}`,
      connected: true
    })
    activeTabIndex.value = terminalTabs.value.length - 1
  }

  triggerToast(`已连通 ${host.name} (SSH-2.0)`, 'success')
  refreshHostMetrics(host, sessionId)
}

// SFTP 直连：复用该主机已有的 SSH 会话
const handleConnectSftp = async (host: HostItem) => {
  currentSftpHost.value = host

  const sessionId = await ensureSession(host)
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
  for (const [hostId, sessionId] of Array.from(sessionByHost.entries())) {
    if (sessionId === tab.id) sessionByHost.delete(hostId)
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
  // 真实环境下不存在“凭空新建的本地 shell”，引导用户先选主机
  if (isTauri()) {
    activeView.value = 'hosts'
    triggerToast('请选择一台服务器发起 SSH 连接', 'info')
    return
  }

  const host = hosts.value[0] || {
    id: 'custom',
    name: 'shell',
    host: 'localhost',
    port: 22,
    username: 'root',
    authType: 'password',
    group: 'staging',
    tags: []
  }
  terminalTabs.value.push({
    id: 'mock-tab-' + Date.now(),
    hostId: host.id,
    title: host.name,
    sub: `${host.username}@${host.host}`,
    connected: true
  })
  activeTabIndex.value = terminalTabs.value.length - 1
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
  if (confirm(`确定要删除服务器 ${host.name} 吗？`)) {
    await deleteHostApi(host.id)
    await loadHosts()
    triggerToast(`已移除主机: ${host.name}`, 'warning')
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
  const task: TransferTask = {
    id: 'task-' + Date.now() + '-' + Math.floor(Math.random() * 1000),
    name,
    type,
    progress: 0,
    speed: '准备中...',
    size,
    status: 'running'
  }
  transferTasks.value.unshift(task)
  transfersOpen.value = true
  return task
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
      clearInterval(timer)
    }
  }, 400)
}

const handleDownloadFile = async (file: any, remotePath: string) => {
  const sessionId = currentSftpSessionId.value
  if (isTauri() && !sessionId) {
    triggerToast('请先连接该主机的 SSH/SFTP 会话', 'warning')
    return
  }

  const localPath = await pickSavePath(file.name)
  if (isTauri() && !localPath) return

  const task = createTransferTask(`下载: ${file.name}`, 'download', file.size)
  if (!isTauri()) {
    runMockTransfer(task)
    return
  }

  await runTransfer(task, (taskId) =>
    sftpDownloadFileApi(sessionId, remotePath, localPath as string, taskId)
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

  const task = createTransferTask(`上传: ${fileName} → ${remoteDir}`, 'upload')
  if (!isTauri()) {
    runMockTransfer(task)
    return
  }

  const remotePath = remoteDir === '/' ? `/${fileName}` : `${remoteDir}/${fileName}`
  await runTransfer(task, (taskId) =>
    sftpUploadFileApi(sessionId, localPath as string, remotePath, taskId)
  )
}

const handleOpenInTerminal = (path: string) => {
  activeView.value = 'terminal'
  const tab = terminalTabs.value[activeTabIndex.value]
  if (isTauri() && tab?.connected) {
    // 真实会话：把 cd 命令送入当前 Shell
    writeSshSessionApi(tab.id, `cd ${shellQuote(path)}\n`).catch(() => {})
    triggerToast(`已在终端切换至 ${path}`, 'info')
  } else {
    triggerToast('当前没有活动终端会话，无法切换目录', 'warning')
  }
}

// 远端路径可能含空格/特殊字符，统一单引号包裹
const shellQuote = (value: string) => `'${value.replace(/'/g, `'\\''`)}'`

// 批量下载：选一次目标目录，逐个文件复用同一套传输任务与进度事件
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
    const task = createTransferTask(`下载: ${ref.file.name}`, 'download', ref.file.size)
    if (!isTauri()) {
      runMockTransfer(task)
      continue
    }
    const sep = targetDir.includes('\\') ? '\\' : '/'
    const localPath = `${targetDir}${targetDir.endsWith(sep) ? '' : sep}${ref.file.name}`
    await runTransfer(task, (taskId) =>
      sftpDownloadFileApi(sessionId, ref.remotePath, localPath, taskId)
    )
  }
}

const handleClearCompletedTasks = () => {
  transferTasks.value = transferTasks.value.filter(t => t.status !== 'completed')
  triggerToast('已清理完成的传输任务', 'info')
}

const handlePaletteAction = (act: string) => {
  if (act === 'new-host') openNewHostModal()
  if (act === 'toggle-transfers') transfersOpen.value = !transfersOpen.value
}

// 全局快捷键注册
const setupGlobalKeybindings = () => {
  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      paletteOpen.value = true
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n' && !e.shiftKey) {
      e.preventDefault()
      openNewHostModal()
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't') {
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
</script>
