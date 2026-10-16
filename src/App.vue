<template>
  <div class="h-screen w-screen flex flex-col bg-surface-base text-slate-100 font-sans antialiased overflow-hidden select-none">
    <!-- 1. 现代化顶栏 -->
    <TitleBar
      :active-transfers-count="activeTransfersCount"
      :transfer-speed="currentTransferSpeed"
      @open-palette="paletteOpen = true"
      @toggle-transfers="transfersOpen = !transfersOpen"
      @open-new-host="openNewHostModal"
      @open-settings="triggerToast('设置中心：支持终端主题 (Catppuccin, Tokyo Night) 与字体定制', 'info')"
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
        @add-group="triggerToast('可自定义新增分组与环境标签', 'info')"
        @open-vault="triggerToast('SSH 密钥保险箱：支持 RSA / Ed25519 密钥对生成与导入', 'info')"
        @open-snippets="triggerToast('常用命令片段库：支持团队快捷运维脚本共享', 'info')"
        @open-tunnels="triggerToast('端口转发隧道管理器：支持 Local/Remote/Dynamic 转发', 'info')"
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
          @import-export="triggerToast('支持从 SSH Config、FinalShell、Termius 导入主机', 'info')"
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
          @toggle-split="triggerToast('已激活水平分屏视图', 'info')"
          @broadcast-mode="triggerToast('多会话同步指令广播模式已就绪', 'info')"
        />

        <!-- 视图 3: SFTP 文件管理 -->
        <SftpView
          v-else-if="activeView === 'sftp'"
          :hosts="hosts"
          :current-host="currentSftpHost"
          :active-session-id="currentSessionId"
          @select-host="host => currentSftpHost = host"
          @edit-file="handleOpenEditor"
          @download-file="handleDownloadFile"
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
      :session-id="currentSessionId"
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
import type { HostItem, TerminalTab, TransferTask, ToastMessage } from '@/types'
import { getHostsApi, saveHostApi, deleteHostApi, createSshSessionApi, isTauri } from '@/api/tauri'

// 核心视图状态
const activeView = ref<'hosts' | 'terminal' | 'sftp'>('hosts')
const selectedGroup = ref<string>('all')

// 数据状态
const hosts = ref<HostItem[]>([])
const terminalTabs = ref<TerminalTab[]>([
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
])
const activeTabIndex = ref(0)
const currentSftpHost = ref<HostItem | undefined>(undefined)

// 传输队列状态
const transfersOpen = ref(false)
const transferTasks = ref<TransferTask[]>([
  {
    id: 't-1',
    name: 'frontend-dist-v2.1.tar.gz',
    type: 'upload',
    progress: 78,
    speed: '14.2 MB/s · 剩余 6秒',
    size: '312 MB / 400 MB',
    status: 'running'
  },
  {
    id: 't-2',
    name: 'nginx-access-20250220.log',
    type: 'download',
    progress: 45,
    speed: '4.2 MB/s · 剩余 13秒',
    size: '45 MB / 100 MB',
    status: 'running'
  },
  {
    id: 't-3',
    name: 'app-config-production.yaml',
    type: 'upload',
    progress: 100,
    speed: '已完成',
    size: '24 KB',
    status: 'completed'
  }
])

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
  return activeTransfersCount.value > 0 ? '18.4 MB/s' : '0 B/s'
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
}

// 加载服务器数据
const loadHosts = async () => {
  hosts.value = await getHostsApi()
  if (!currentSftpHost.value && hosts.value.length > 0) {
    currentSftpHost.value = hosts.value[0]
  }
}

// SSH 直连
const handleConnectSsh = async (host: HostItem) => {
  activeView.value = 'terminal'
  triggerToast(`正在建立 SSH 会话至 ${host.name} (${host.host})...`, 'info')

  let sessionId = 'tab-' + Date.now()
  if (isTauri()) {
    try {
      const res = await createSshSessionApi(host, 80, 24)
      sessionId = res.sessionId
    } catch (e: any) {
      triggerToast(`SSH 连接失败: ${e?.message || e}`, 'error')
      return
    }
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
}

// SFTP 直连
const handleConnectSftp = (host: HostItem) => {
  currentSftpHost.value = host
  activeView.value = 'sftp'
  triggerToast(`已加载 ${host.name} 的 SFTP 目录系统`, 'success')
}

// 终端 Tab 操作
const handleCloseTerminalTab = (index: number) => {
  if (terminalTabs.value.length <= 1) {
    triggerToast('至少保留一个活动终端会话', 'warning')
    return
  }
  terminalTabs.value.splice(index, 1)
  if (activeTabIndex.value >= terminalTabs.value.length) {
    activeTabIndex.value = terminalTabs.value.length - 1
  }
}

const handleAddTerminalTab = () => {
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
    id: 'tab-' + Date.now(),
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

const handleDownloadFile = (file: any, _remotePath: string) => {
  const task: TransferTask = {
    id: 'task-' + Date.now(),
    name: `下载: ${file.name}`,
    type: 'download',
    progress: 10,
    speed: '12.4 MB/s',
    size: file.size,
    status: 'running'
  }
  transferTasks.value.unshift(task)
  transfersOpen.value = true
  triggerToast(`已将 ${file.name} 添加至下载队列`, 'info')

  // 进度模拟
  const interval = setInterval(() => {
    task.progress += 25
    if (task.progress >= 100) {
      task.progress = 100
      task.status = 'completed'
      clearInterval(interval)
      triggerToast(`文件 ${file.name} 下载完成！`, 'success')
    }
  }, 400)
}

const handleUploadTrigger = (remoteDir: string) => {
  const input = document.createElement('input')
  input.type = 'file'
  input.onchange = (e: any) => {
    const f = e.target?.files?.[0]
    if (f) {
      const task: TransferTask = {
        id: 'task-' + Date.now(),
        name: `上传: ${f.name} -> ${remoteDir}`,
        type: 'upload',
        progress: 15,
        speed: '16.8 MB/s',
        size: `${(f.size / 1024 / 1024).toFixed(1)} MB`,
        status: 'running'
      }
      transferTasks.value.unshift(task)
      transfersOpen.value = true
      triggerToast(`已开始上传 ${f.name}`, 'info')

      const interval = setInterval(() => {
        task.progress += 30
        if (task.progress >= 100) {
          task.progress = 100
          task.status = 'completed'
          clearInterval(interval)
          triggerToast(`文件 ${f.name} 上传完成！`, 'success')
        }
      }, 350)
    }
  }
  input.click()
}

const handleOpenInTerminal = (path: string) => {
  activeView.value = 'terminal'
  triggerToast(`已在终端切至 ${path}`, 'info')
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
