<template>
  <div class="h-screen w-screen flex flex-col bg-surface-base text-slate-100 font-sans antialiased overflow-hidden select-none">
    <!-- 1. 现代化顶栏 (含主机会话多标签栏) -->
    <TitleBar
      :active-transfers-count="activeTransfersCount"
      :transfer-speed="currentTransferSpeed"
      :sessions="sessions"
      :active-session-id="activeSessionId"
      @select-session="handleSelectSession"
      @close-session="handleCloseSession"
      @open-palette="paletteOpen = true"
      @toggle-transfers="transfersOpen = !transfersOpen"
      @open-new-host="openNewHostModal"
      @open-settings="triggerToast('设置中心尚未实现', 'info')"
      @close-window="handleCloseWindow"
    />

    <!-- 2. 主体工作区 -->
    <div class="flex flex-1 overflow-hidden relative">
      <!-- 视图 A: 服务器资产管理 (activeSessionId === 'hosts') -->
      <div v-show="activeSessionId === 'hosts'" class="flex flex-1 overflow-hidden">
        <!-- 主机分类侧边栏 -->
        <Sidebar
          active-view="hosts"
          :selected-group="selectedGroup"
          :total-hosts="hosts.length"
          :active-sessions-count="sessions.length"
          :group-counts="groupCounts"
          :recent-hosts="hosts.slice(0, 2)"
          @switch-view="handleSidebarSwitchView"
          @filter-group="val => selectedGroup = val"
          @connect-host="handleConnectSsh"
          @add-group="triggerToast('自定义分组尚未实现', 'info')"
          @open-vault="triggerToast('SSH 密钥保险箱尚未实现', 'info')"
          @open-snippets="triggerToast('命令片段库尚未实现', 'info')"
          @open-tunnels="triggerToast('端口转发隧道尚未实现', 'info')"
        />

        <!-- 主机资产管理视图 -->
        <main class="flex-1 flex flex-col overflow-hidden bg-surface-base relative">
          <HostManager
            :hosts="hosts"
            :selected-group="selectedGroup"
            @connect-ssh="handleConnectSsh"
            @connect-sftp="handleConnectSsh"
            @edit-host="handleEditHost"
            @delete-host="handleDeleteHost"
            @open-new-host="openNewHostModal"
            @import-export="triggerToast('主机导入/导出尚未实现', 'info')"
          />
        </main>
      </div>

      <!-- 视图 B: 主机会话协同工作区 (左侧树形文件 + 右侧交互终端) -->
      <SessionWorkspace
        v-for="s in sessions"
        :key="s.id"
        v-show="s.id === activeSessionId"
        :session="s"
        :is-active="s.id === activeSessionId"
        :refresh-signal="sftpRefreshSignal"
        @switch-terminal-tab="(idx: number) => s.activeTerminalIndex = idx"
        @close-terminal-tab="(idx: number) => handleCloseSessionTerminalTab(s, idx)"
        @add-terminal-tab="() => handleAddTerminalTab(s)"
        @duplicate-terminal-tab="() => handleDuplicateTerminalTab(s)"
        @reconnect-terminal-tab="(idx: number) => handleReconnectTerminalTab(s, idx)"
        @rename-terminal-tab="(idx: number) => handleRenameTerminalTab(s, idx)"
        @copy-terminal-info="(idx: number, kind: 'ip' | 'login' | 'ssh-command') => handleCopyTerminalInfo(s, idx, kind)"
        @close-other-terminal-tabs="(idx: number) => handleCloseOtherTerminalTabs(s, idx)"
        @close-tabs-after="(idx: number) => handleCloseTerminalTabsAfter(s, idx)"
        @session-closed="(tabId: string, reason: string) => handleSessionClosed(s, tabId, reason)"
        @edit-file="handleOpenEditor"
        @download-file="(file: SftpFileItem, path: string) => handleDownloadFile(s, file, path)"
        @upload-trigger="(dir: string) => handleUploadTrigger(s, dir)"
        @toast="triggerToast"
      />

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
      :session-id="currentEditingSessionId"
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
import SessionWorkspace from '@/components/SessionWorkspace.vue'
import TransferDrawer from '@/components/TransferDrawer.vue'
import CommandPalette from '@/components/CommandPalette.vue'
import HostModal from '@/components/HostModal.vue'
import EditorModal from '@/components/EditorModal.vue'
import DialogHost from '@/components/DialogHost.vue'
import Toast from '@/components/Toast.vue'
import { confirmDialog, promptDialog, dialogState } from '@/composables/useDialog'
import type { HostItem, HostSession, SftpFileItem, TransferTask, ToastMessage } from '@/types'
import {
  getHostsApi,
  saveHostApi,
  deleteHostApi,
  createSshSessionApi,
  closeSshSessionApi,
  probeHostInfoApi,
  sftpUploadFileApi,
  sftpDownloadFileApi,
  sftpDownloadDirApi,
  onSftpProgress,
  pickLocalFile,
  pickSavePath,
  isTauri
} from '@/api/tauri'

// 核心多会话状态
const activeSessionId = ref<string>(isTauri() ? 'hosts' : 'session-demo-144')
const selectedGroup = ref<string>('all')

// 数据状态
const hosts = ref<HostItem[]>([])

// 浏览器端演示用的初始会话（还原与截图一致的初始呈现）
const demoSession: HostSession = {
  id: 'session-demo-144',
  hostId: 'prod-k8s-master-01',
  host: {
    id: 'prod-k8s-master-01',
    name: 'quickea144',
    host: '47.98.112.24',
    port: 22,
    username: 'root',
    authType: 'key',
    group: 'production',
    tags: ['k8s', 'master'],
    os: 'ubuntu',
    osName: 'Ubuntu 22.04 LTS',
    status: 'online',
    ping: '24ms',
    cpu: 28,
    mem: 64,
    disk: 42,
    updatedAt: '2分钟前'
  },
  title: 'quickea144',
  terminalTabs: [
    {
      id: 'tab-0',
      hostId: 'prod-k8s-master-01',
      title: '终端 1',
      sub: 'root@47.98.112.24',
      connected: true
    }
  ],
  activeTerminalIndex: 0,
  sftpSessionId: 'session-demo-144',
  currentPath: '/root',
  sidebarCollapsed: false,
  sidebarWidth: 280,
  viewMode: 'tree',
  autoFollowTerminal: false
}

const sessions = ref<HostSession[]>(isTauri() ? [] : [demoSession])

// 传输队列
const transfersOpen = ref(false)
const transferTasks = ref<TransferTask[]>([])
const sftpRefreshSignal = ref(0)

// 弹窗状态
const paletteOpen = ref(false)
const hostModalOpen = ref(false)
const editingHost = ref<HostItem | null>(null)
const editorOpen = ref(false)
const editingFilePath = ref('')
const currentEditingSessionId = ref('')

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

// 动作方法
const triggerToast = (text: string, type: 'info' | 'success' | 'warning' | 'error' = 'info') => {
  const item: ToastMessage = { id: 'toast-' + Date.now() + Math.random(), text, type }
  toastList.value.push(item)
  if (type !== 'error') {
    setTimeout(() => dismissToast(item.id), 3200)
  }
}

const dismissToast = (id: string) => {
  toastList.value = toastList.value.filter(t => t.id !== id)
}

const handleSelectSession = (sessionId: string) => {
  activeSessionId.value = sessionId
}

const handleSidebarSwitchView = (v: string) => {
  if (v === 'hosts') {
    activeSessionId.value = 'hosts'
  } else if (sessions.value.length > 0) {
    activeSessionId.value = sessions.value[0].id
  }
}

// 加载服务器数据
const loadHosts = async () => {
  hosts.value = await getHostsApi()
}

// ==========================================
// SSH 会话建立
// ==========================================
const probingHosts = new Set<string>()

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

// 连接成功后异步采集真实指标
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
    await saveHostApi(updated)
  } catch (e) {
    console.warn('probe_host_info failed:', e)
  } finally {
    probingHosts.delete(host.id)
  }
}

// 连接主机：创建主机会话工作区（左侧文件树 + 右侧交互终端）
const handleConnectSsh = async (host: HostItem) => {
  // 如果已存在该主机会话，直接激活
  const existing = sessions.value.find(s => s.hostId === host.id)
  if (existing) {
    activeSessionId.value = existing.id
    triggerToast(`已切换至会话: ${existing.title}`, 'info')
    return
  }

  triggerToast(`正在建立 SSH 会话至 ${host.name} (${host.host})...`, 'info')

  let sessionId: string | null = null
  if (isTauri()) {
    sessionId = await openSession(host)
    if (!sessionId) return
  } else {
    sessionId = 'session-mock-' + Date.now()
  }

  const newSession: HostSession = {
    id: sessionId,
    hostId: host.id,
    host,
    title: host.name,
    terminalTabs: [
      {
        id: sessionId,
        hostId: host.id,
        title: '终端 1',
        sub: `${host.username}@${host.host}`,
        connected: true
      }
    ],
    activeTerminalIndex: 0,
    sftpSessionId: sessionId,
    currentPath: '/root',
    sidebarCollapsed: false,
    sidebarWidth: 280,
    viewMode: 'tree',
    autoFollowTerminal: false
  }

  sessions.value.push(newSession)
  activeSessionId.value = newSession.id

  triggerToast(`已连通 ${host.name}，文件树与终端就绪`, 'success')
  refreshHostMetrics(host, sessionId)
}

// 关闭整个主机会话
const handleCloseSession = (sessionId: string) => {
  const idx = sessions.value.findIndex(s => s.id === sessionId)
  if (idx < 0) return
  const s = sessions.value[idx]

  if (isTauri()) {
    s.terminalTabs.forEach(t => {
      if (t.id.startsWith('ssh-')) closeSshSessionApi(t.id).catch(() => {})
    })
  }

  sessions.value.splice(idx, 1)
  if (activeSessionId.value === sessionId) {
    if (sessions.value.length > 0) {
      activeSessionId.value = sessions.value[Math.max(0, idx - 1)].id
    } else {
      activeSessionId.value = 'hosts'
    }
  }
  triggerToast(`已关闭会话 ${s.title}`, 'info')
}

// 远端断开
const handleSessionClosed = (session: HostSession, tabId: string, reason: string) => {
  const tab = session.terminalTabs.find(t => t.id === tabId)
  if (tab) tab.connected = false
  triggerToast(`终端连接已断开: ${reason}`, 'warning')
}

// 单会话内的终端管理
const handleCloseSessionTerminalTab = (session: HostSession, index: number) => {
  const tab = session.terminalTabs[index]
  if (!tab) return

  if (isTauri() && tab.id.startsWith('ssh-')) {
    closeSshSessionApi(tab.id).catch(() => {})
  }

  session.terminalTabs.splice(index, 1)
  if (session.terminalTabs.length === 0) {
    // 若该主机所有终端均关闭，则关闭该会话
    handleCloseSession(session.id)
  } else if (session.activeTerminalIndex >= session.terminalTabs.length) {
    session.activeTerminalIndex = session.terminalTabs.length - 1
  }
}

const handleAddTerminalTab = async (session: HostSession) => {
  triggerToast(`正在为 ${session.host.name} 打开新终端...`, 'info')
  let newId = 'sub-mock-' + Date.now()
  if (isTauri()) {
    const resId = await openSession(session.host)
    if (!resId) return
    newId = resId
  }
  const idx = session.terminalTabs.length + 1
  session.terminalTabs.push({
    id: newId,
    hostId: session.host.id,
    title: `终端 ${idx}`,
    sub: `${session.host.username}@${session.host.host}`,
    connected: true
  })
  session.activeTerminalIndex = session.terminalTabs.length - 1
  triggerToast(`终端 ${idx} 已就绪`, 'success')
}

const handleDuplicateTerminalTab = (session: HostSession) => {
  handleAddTerminalTab(session)
}

const handleReconnectTerminalTab = async (session: HostSession, index: number) => {
  const tab = session.terminalTabs[index]
  if (!tab) return
  if (isTauri() && tab.id.startsWith('ssh-')) {
    closeSshSessionApi(tab.id).catch(() => {})
  }
  let newId = tab.id
  if (isTauri()) {
    const resId = await openSession(session.host)
    if (!resId) return
    newId = resId
  }
  tab.id = newId
  tab.connected = true
  triggerToast(`已重新连接终端 ${tab.title}`, 'success')
}

const handleRenameTerminalTab = async (session: HostSession, index: number) => {
  const tab = session.terminalTabs[index]
  if (!tab) return
  const name = await promptDialog({
    title: '重命名终端',
    message: `为终端设置显示名称`,
    input: { label: '名称', value: tab.title, placeholder: '例如：监控、日志', required: true },
    confirmText: '保存'
  })
  if (!name) return
  tab.title = name.trim()
}

const handleCopyTerminalInfo = async (session: HostSession, _index: number, kind: 'ip' | 'login' | 'ssh-command') => {
  const host = session.host
  const text = kind === 'ip'
    ? host.host
    : kind === 'login'
      ? `${host.username}@${host.host}`
      : `ssh -p ${host.port} ${host.username}@${host.host}`
  const label = kind === 'ip' ? 'IP 地址' : kind === 'login' ? '登录信息' : 'SSH 命令'
  await navigator.clipboard.writeText(text)
  triggerToast(`已复制${label}: ${text}`, 'success')
}

const handleCloseOtherTerminalTabs = (session: HostSession, index: number) => {
  const keep = session.terminalTabs[index]
  if (!keep) return
  for (let i = session.terminalTabs.length - 1; i >= 0; i--) {
    if (i !== index) handleCloseSessionTerminalTab(session, i)
  }
  session.activeTerminalIndex = 0
}

const handleCloseTerminalTabsAfter = (session: HostSession, index: number) => {
  for (let i = session.terminalTabs.length - 1; i > index; i--) {
    handleCloseSessionTerminalTab(session, i)
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

// 安全关闭窗口
const handleCloseWindow = async () => {
  const running = transferTasks.value.filter(t => t.status === 'running').length
  const activeSessCount = sessions.value.length

  if (running > 0 || activeSessCount > 0) {
    const parts: string[] = []
    if (running > 0) parts.push(`${running} 个传输任务正在执行`)
    if (activeSessCount > 0) parts.push(`${activeSessCount} 个会话仍处于连接状态`)
    const confirmed = await confirmDialog({
      title: '退出 SnailSSH',
      variant: 'warning',
      message: `当前${parts.join('，')}，确定要退出应用吗？`,
      confirmText: '退出',
      cancelText: '取消'
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
  currentEditingSessionId.value = activeSessionId.value
  editorOpen.value = true
}

const formatBytes = (bytes: number) => {
  if (!bytes) return '0 B'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const createTransferTask = (name: string, type: 'upload' | 'download', size = '准备中...'): TransferTask => {
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
    sftpRefreshSignal.value++
  } catch (e: any) {
    task.status = 'failed'
    task.speed = `失败: ${e?.message || e}`
    triggerToast(`传输失败: ${e?.message || e}`, 'error')
  } finally {
    unlisten()
  }
}

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

const handleDownloadFile = async (session: HostSession, file: SftpFileItem, remotePath: string) => {
  const sessionId = session.sftpSessionId || session.id
  if (!isTauri()) {
    runMockTransfer(createTransferTask(`下载: ${file.name}`, 'download', file.size))
    return
  }

  const localPath = await pickSavePath(file.name)
  if (!localPath) return

  // 计算远程绝对路径：如果 remotePath 已包含文件名或以文件名结尾，则直接使用；否则进行拼接
  let fullRemote = remotePath
  if (!fullRemote.endsWith('/' + file.name) && fullRemote !== file.name) {
    fullRemote = remotePath === '/' ? `/${file.name}` : `${remotePath.replace(/\/+$/, '')}/${file.name}`
  }

  const task = createTransferTask(`下载: ${file.name}`, 'download', file.size)
  if (file.isDir) {
    await runTransfer(task, (taskId) =>
      sftpDownloadDirApi(sessionId, fullRemote, localPath, taskId)
    )
  } else {
    await runTransfer(task, (taskId) =>
      sftpDownloadFileApi(sessionId, fullRemote, localPath, taskId)
    )
  }
}

const handleUploadTrigger = async (session: HostSession, remoteDir: string) => {
  const sessionId = session.sftpSessionId || session.id
  if (!isTauri()) {
    runMockTransfer(createTransferTask('上传: mock_package.tar.gz', 'upload', '24.5 MB'))
    return
  }

  const localPath = await pickLocalFile()
  if (!localPath) return

  const fileName = localPath.split(/[/\\]/).pop() || 'file'
  const fullRemote = remoteDir === '/' ? `/${fileName}` : `${remoteDir}/${fileName}`
  const task = createTransferTask(`上传: ${fileName}`, 'upload')
  await runTransfer(task, (taskId) =>
    sftpUploadFileApi(sessionId, localPath, fullRemote, taskId)
  )
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

const isTerminalOrEditorTarget = (target: EventTarget | null): boolean => {
  const el = target as HTMLElement | null
  if (!el || typeof el.closest !== 'function') return false
  if (el.tagName === 'TEXTAREA' || el.isContentEditable) return true
  return !!el.closest('.xterm')
}

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

onBeforeUnmount(() => {
  if (!isTauri()) return
  for (const s of sessions.value) {
    for (const tab of s.terminalTabs) {
      if (tab.id.startsWith('ssh-')) closeSshSessionApi(tab.id).catch(() => {})
    }
  }
})
</script>
