<template>
  <div
    class="h-full flex flex-col bg-surface-sidebar border-r border-surface-border overflow-hidden select-none"
    data-drop-target="sftp"
    :data-remote-path="pathReady ? currentPath : ''"
    :data-session-id="sessionId"
  >
    <!-- 1. 顶部工作区标头栏 (仿现代化标签与窗格控制) -->
    <div class="h-9 bg-surface-sidebar border-b border-surface-border px-3 flex items-center justify-between shrink-0">
      <div class="flex items-center gap-1.5">
        <div class="flex items-center gap-1 px-2.5 py-1 rounded bg-[#0c0e14] border border-surface-border/60 text-xs font-mono font-medium text-emerald-400">
          <FolderGit2 class="w-3.5 h-3.5" />
          <span>文件工作区</span>
        </div>
        <!-- 环境标识：颜色与终端标签一致，避免在错误的环境里删文件 -->
        <span
          v-if="currentGroup.danger"
          class="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono border"
          :class="[groupColorOf(currentGroup.color).text, groupColorOf(currentGroup.color).border, groupColorOf(currentGroup.color).bg]"
          :title="`当前分组：${currentGroup.label}`"
        >
          <span class="w-1.5 h-1.5 rounded-full" :class="groupColorOf(currentGroup.color).dot"></span>
          <span>{{ currentGroup.label }}</span>
        </span>
      </div>
      <div class="flex items-center gap-1 text-slate-400">
        <button
          @click="emit('toggle-collapse')"
          class="p-1 rounded hover:bg-surface-elevated hover:text-white transition-colors"
          title="折叠文件区 (纯终端模式)"
        >
          <PanelLeftClose class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- 2. 工具栏与视图模式切换 -->
    <div class="h-8 bg-surface-card/40 border-b border-surface-border/60 px-2 flex items-center justify-between shrink-0">
      <div class="flex items-center gap-0.5">
        <!-- 树形 / 列表 切换 -->
        <button
          @click="viewMode = 'tree'"
          class="p-1 rounded transition-colors"
          :class="viewMode === 'tree' ? 'bg-surface-elevated text-emerald-400 font-semibold' : 'text-slate-400 hover:text-white hover:bg-surface-elevated/50'"
          title="树形结构视图"
        >
          <ListTree class="w-3.5 h-3.5" />
        </button>
        <button
          @click="viewMode = 'list'"
          class="p-1 rounded transition-colors"
          :class="viewMode === 'list' ? 'bg-surface-elevated text-emerald-400 font-semibold' : 'text-slate-400 hover:text-white hover:bg-surface-elevated/50'"
          title="平铺列表视图"
        >
          <List class="w-3.5 h-3.5" />
        </button>

        <div class="h-3 w-[1px] bg-surface-border mx-1"></div>

        <!-- 在终端打开当前目录 -->
        <button
          @click="openInTerminal(currentPath)"
          class="p-1 rounded text-slate-400 hover:text-emerald-400 hover:bg-surface-elevated transition-colors"
          title="在右侧终端中打开当前目录"
        >
          <Terminal class="w-3.5 h-3.5" />
        </button>

        <!-- 上传 -->
        <button
          @click="triggerUpload"
          class="p-1 rounded text-slate-400 hover:text-brand-300 hover:bg-surface-elevated transition-colors"
          title="上传文件到当前目录"
        >
          <Upload class="w-3.5 h-3.5" />
        </button>

        <!-- 下载选中项 (文件或目录) -->
        <button
          v-if="selectedNode"
          @click="downloadSelectedNode"
          class="p-1 rounded text-slate-400 hover:text-cyan-300 hover:bg-surface-elevated transition-colors"
          :title="selectedNode.isDir ? `下载目录「${selectedNode.name}」到本地` : `下载「${selectedNode.name}」到本地`"
        >
          <Download class="w-3.5 h-3.5" />
        </button>

        <!-- 新建文件夹 -->
        <button
          @click="createNewFolder"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-surface-elevated transition-colors"
          title="新建文件夹"
        >
          <FolderPlus class="w-3.5 h-3.5" />
        </button>

        <!-- 刷新 -->
        <button
          @click="refreshCurrent"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-surface-elevated transition-colors"
          title="刷新目录"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        </button>

        <!-- 隐藏文件开关 -->
        <button
          @click="showHidden = !showHidden"
          class="p-1 rounded transition-colors"
          :class="showHidden ? 'text-brand-400 bg-brand-500/10' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="显示/隐藏点开头文件"
        >
          <Eye class="w-3.5 h-3.5" />
        </button>
      </div>

      <!-- 快速书签 -->
      <div class="relative">
        <button
          @click="showBookmarks = !showBookmarks"
          class="p-1 rounded text-slate-400 hover:text-amber-400 hover:bg-surface-elevated transition-colors"
          title="常用目录快捷书签"
        >
          <Bookmark class="w-3.5 h-3.5" />
        </button>
        <!-- 书签下拉 -->
        <div
          v-if="showBookmarks"
          class="absolute right-0 top-full mt-1 w-48 bg-surface-sidebar border border-surface-border rounded-lg shadow-xl z-30 py-1 text-xs"
        >
          <div
            v-for="b in settings.bookmarks"
            :key="b.id"
            @click="jumpBookmark(b.path)"
            class="px-2.5 py-1.5 hover:bg-surface-elevated text-slate-300 hover:text-white cursor-pointer flex items-center justify-between gap-2"
          >
            <span class="truncate">{{ b.label }}</span>
            <span class="text-[10px] text-slate-500 font-mono truncate max-w-[90px]">{{ b.path }}</span>
          </div>
          <div v-if="settings.bookmarks.length === 0" class="px-2.5 py-2 text-[11px] text-slate-500">
            暂无书签，可在设置中心添加
          </div>
        </div>
      </div>
    </div>

    <!-- 3. 路径面包屑导航栏 -->
    <div class="h-8 bg-[#0c0e14]/70 border-b border-surface-border/60 px-2 flex items-center gap-1 shrink-0 text-xs font-mono">
      <div v-if="!editingPathInput && !pathReady" class="flex-1 text-[11px] text-slate-500 truncate">
        {{ initialPath === '~' || !initialPath ? '正在打开家目录...' : '正在打开目录...' }}
      </div>
      <div v-else-if="!editingPathInput" class="flex items-center gap-1 overflow-x-auto no-scrollbar flex-1">
        <span
          @click="jumpToPath('/')"
          class="px-1 py-0.5 rounded text-slate-400 hover:text-white hover:bg-surface-elevated cursor-pointer"
        >/</span>
        <template v-for="(seg, idx) in pathSegments" :key="idx">
          <span class="text-slate-600">></span>
          <span
            @click="jumpToSegment(idx)"
            class="px-1 py-0.5 rounded cursor-pointer transition-colors"
            :class="idx === pathSegments.length - 1 ? 'text-brand-300 font-bold bg-surface-card/60' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          >
            {{ seg }}
          </span>
        </template>
      </div>

      <!-- 路径直接编辑框 -->
      <form v-else @submit.prevent="submitPathInput" class="flex-1">
        <input
          v-model="pathInputBuffer"
          ref="pathInputRef"
          type="text"
          @blur="editingPathInput = false"
          @keydown.esc="editingPathInput = false"
          class="w-full bg-surface-card border border-brand-500/60 rounded px-1.5 py-0.5 text-xs text-white focus:outline-none"
        />
      </form>

      <button
        @click="startEditPath"
        class="p-0.5 rounded text-slate-500 hover:text-white"
        title="直接输入路径"
      >
        <FolderSearch class="w-3 h-3" />
      </button>
    </div>

    <!-- 4. 主内容区：树形视图 或 列表视图 -->
    <div class="flex-1 overflow-y-auto custom-scrollbar relative p-1" @contextmenu.prevent="handleEmptyAreaContextMenu($event)">
      <!-- 加载遮罩 -->
      <div
        v-if="loading && rootNodes.length === 0"
        class="w-full h-full flex flex-col items-center justify-center gap-2 text-slate-500 py-12"
      >
        <RefreshCw class="w-5 h-5 animate-spin text-brand-400" />
        <span class="text-xs font-mono">加载远端文件...</span>
      </div>

      <!-- A. 树形模式 -->
      <div v-else-if="viewMode === 'tree'" class="space-y-0.5">
        <FileTreeNodeItem
          v-for="node in filteredRootNodes"
          :key="node.path"
          :node="node"
          :depth="0"
          :selected-path="selectedPath"
          :show-hidden="showHidden"
          @select="handleSelectNode"
          @toggle="handleToggleNode"
          @open-file="handleOpenFile"
          @context-menu="handleNodeContextMenu"
        />
      </div>

      <!-- B. 平铺列表模式 -->
      <div v-else class="space-y-0.5">
        <div
          v-if="currentPath !== '/'"
          @click="navigateUp"
          class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-surface-card/60 cursor-pointer text-xs font-mono text-cyan-300"
        >
          <CornerLeftUp class="w-3.5 h-3.5 text-cyan-400" />
          <span>.. (返回上一级)</span>
        </div>

        <div
          v-for="file in currentListFiles"
          :key="file.path"
          @click="handleSelectNode(file)"
          @dblclick="file.isDir ? handleToggleNode(file) : handleOpenFile(file)"
          @contextmenu.prevent.stop="handleNodeContextMenu($event, file)"
          class="flex items-center justify-between px-2 py-1.5 rounded cursor-pointer text-xs font-mono transition-colors"
          :class="selectedPath === file.path ? 'bg-brand-500/25 text-emerald-300 font-semibold border-l-2 border-brand-500' : 'hover:bg-surface-card/70 text-slate-300'"
        >
          <div class="flex items-center gap-2 truncate">
            <component :is="file.isDir ? Folder : File" class="w-3.5 h-3.5 shrink-0" :class="file.isDir ? 'text-amber-400' : 'text-slate-400'" />
            <span class="truncate">{{ file.name }}</span>
          </div>
          <span class="text-[10px] text-slate-500 shrink-0">{{ file.size }}</span>
        </div>
      </div>
    </div>

    <!-- 5. 底部目录跟随工具栏 -->
    <div class="h-8 bg-surface-card/40 border-t border-surface-border/60 px-2 flex items-center justify-between shrink-0 select-none">
      <div class="flex items-center gap-1.5 shrink-0">
        <!-- 自动跟随开关按钮 -->
        <button
          @click="toggleAutoFollow"
          :class="autoFollow ? 'bg-brand-500/15 text-emerald-400 border-brand-500/40 hover:bg-brand-500/25' : 'bg-surface-card text-slate-400 border-surface-border hover:text-white hover:bg-surface-elevated'"
          class="flex items-center gap-1.5 px-2 py-1 rounded-md border text-[11px] font-mono transition-all group shrink-0 whitespace-nowrap"
          :title="autoFollow ? '自动跟随：开启中（终端切目录，工作区自动进入）' : '自动跟随：已关闭（点击开启）'"
        >
          <span
            class="w-1.5 h-1.5 rounded-full transition-all"
            :class="autoFollow ? 'bg-emerald-400 shadow-glow-emerald animate-pulse' : 'bg-slate-600'"
          ></span>
          <Compass class="w-3 h-3 transition-transform" :class="{ 'rotate-45 text-emerald-400': autoFollow }" />
          <span>{{ autoFollow ? '跟随:开' : '跟随:关' }}</span>
        </button>

        <!-- 手动跟随按钮 -->
        <button
          @click="triggerManualSync"
          :disabled="syncing"
          class="flex items-center gap-1 px-2 py-1 rounded-md bg-surface-card hover:bg-surface-elevated border border-surface-border text-slate-300 hover:text-white text-[11px] font-mono transition-all disabled:opacity-50 disabled:cursor-not-allowed shrink-0 whitespace-nowrap"
          title="手动跟随：点击立即进入终端所在目录"
        >
          <Locate class="w-3 h-3 text-cyan-400" :class="{ 'animate-spin': syncing }" />
          <span>同步</span>
        </button>

        <!-- 远程监控按钮 (对标 MobaXterm 底部远程监控) -->
        <button
          @click="toggleRemoteMonitor"
          :class="remoteMonitorOpen ? 'bg-brand-500/15 text-emerald-400 border-brand-500/40 hover:bg-brand-500/25' : 'bg-surface-card text-slate-300 hover:text-white hover:bg-surface-elevated border-surface-border'"
          class="flex items-center gap-1 px-2 py-1 rounded-md border text-[11px] font-mono transition-all group shrink-0 whitespace-nowrap"
          :title="remoteMonitorOpen ? '远程监控：开启中（点击收起监控栏）' : '远程监控：已关闭（点击展开服务器资源监控）'"
        >
          <span
            class="w-1.5 h-1.5 rounded-full transition-all"
            :class="remoteMonitorOpen ? 'bg-emerald-400 shadow-glow-emerald animate-pulse' : 'bg-slate-600'"
          ></span>
          <Activity class="w-3 h-3 text-emerald-400" />
          <span>监控</span>
        </button>
      </div>

      <!-- 右侧当前终端路径提示：仅在与浏览目录不一致时出现，一致时面包屑已表达，不再重复 -->
      <div
        v-if="terminalCwd && terminalCwd !== currentPath"
        class="text-[10px] font-mono text-slate-500 truncate min-w-0 shrink pl-1 hover:text-slate-300 transition-colors"
        :title="`终端当前在 ${terminalCwd}，点「同步」可跟随过去`"
      >
        终端: {{ terminalCwd }}
      </div>
    </div>

    <!-- 6. 底部状态栏：只承载选中项信息，当前路径由面包屑表达，不再重复 -->
    <div class="h-6 bg-surface-sidebar border-t border-surface-border px-3 flex items-center justify-between text-[10px] font-mono text-slate-500 shrink-0">
      <span v-if="selectedNode" class="truncate max-w-[160px]" :title="selectedNode.path">
        {{ selectedNode.name }}
      </span>
      <span v-else></span>
      <span v-if="selectedNode && !selectedNode.isDir">
        {{ selectedNode.size }} | {{ selectedNode.permissions }}
      </span>
      <span v-else>
        {{ rootNodes.length }} 项
      </span>
    </div>

    <!-- 右键上下文菜单 -->
    <ContextMenu
      :open="contextMenu.open"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :items="contextMenuItems"
      @select="handleContextMenuSelect"
      @close="contextMenu.open = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onUnmounted } from 'vue'
import {
  FolderGit2,
  PanelLeftClose,
  ListTree,
  List,
  Terminal,
  Upload,
  FolderPlus,
  RefreshCw,
  Eye,
  Bookmark,
  FolderSearch,
  CornerLeftUp,
  Folder,
  File,
  Edit3,
  Download,
  Copy,
  Trash2,
  FilePlus,
  Compass,
  Lock,
  Users,
  Info,
  Locate,
  Activity
} from 'lucide-vue-next'
import ContextMenu from '@/components/ContextMenu.vue'
import FileTreeNodeItem from '@/components/FileTreeNodeItem.vue'
import type { ContextMenuItem, FileTreeNode, HostItem, SftpFileItem } from '@/types'
import {
  sftpReadDirApi,
  sftpHomeDirApi,
  sftpMkdirApi,
  sftpRemoveFileApi,
  sftpTouchApi,
  sftpRenameApi,
  sftpChmodApi,
  sftpChownApi,
  sftpStatApi
} from '@/api/tauri'
import { confirmDialog, promptDialog } from '@/composables/useDialog'
import { settings, groupById, groupColorOf } from '@/composables/useAppSettings'

const props = withDefaults(
  defineProps<{
    host: HostItem
    sessionId?: string
    refreshSignal?: number
    terminalCwd?: string
    autoFollow?: boolean
    remoteMonitorOpen?: boolean
    /** 所属分组 id：用于环境色标识与高风险删除确认 */
    groupId?: string
    /** 会话记下的目录。新会话为 `~`，折叠侧边栏再展开时用来恢复，而不是重新停在根目录 */
    initialPath?: string
  }>(),
  {
    terminalCwd: '',
    autoFollow: false,
    remoteMonitorOpen: false,
    groupId: '',
    initialPath: '~'
  }
)

const emit = defineEmits<{
  (e: 'toggle-collapse'): void
  (e: 'open-in-terminal', path: string): void
  (e: 'edit-file', filePath: string): void
  (e: 'download-file', file: SftpFileItem, remotePath: string): void
  (e: 'upload-trigger', remoteDir: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
  (e: 'update:autoFollow', val: boolean): void
  (e: 'toggle-remote-monitor'): void
  (e: 'manual-sync'): void
  (e: 'update:currentPath', path: string): void
}>()

// 只上报一次翻转意图；父组件负责改状态，避免重复取反回到原点
const toggleRemoteMonitor = () => {
  emit('toggle-remote-monitor')
}

const viewMode = ref<'tree' | 'list'>('list')
// 首次列目录完成前不响应刷新信号，避免用尚未替换的 `/` 覆盖家目录
const pathReady = ref(false)
const currentPath = ref('/')
const selectedPath = ref('')
const selectedNode = ref<FileTreeNode | null>(null)
const showHidden = ref(true)
const loading = ref(false)
const showBookmarks = ref(false)
const editingPathInput = ref(false)
const pathInputBuffer = ref('/')
const pathInputRef = ref<HTMLInputElement | null>(null)

// 根级节点集合
const rootNodes = ref<FileTreeNode[]>([])

// 当前分组（决定环境色与高风险确认）
const currentGroup = computed(() => groupById(props.groupId || props.host.group))

const pathSegments = computed(() => {
  return currentPath.value.split('/').filter(Boolean)
})

const filteredRootNodes = computed(() => {
  return rootNodes.value.filter(n => showHidden.value || !n.isHidden)
})

const currentListFiles = computed(() => {
  return rootNodes.value.filter(n => showHidden.value || !n.isHidden)
})

// 右键菜单状态
const contextMenu = ref({
  open: false,
  x: 0,
  y: 0,
  targetNode: null as FileTreeNode | null
})

const contextMenuItems = computed<ContextMenuItem[]>(() => {
  const node = contextMenu.value.targetNode
  if (!node) {
    return [
      { key: 'new-folder', label: '新建文件夹', icon: FolderPlus },
      { key: 'new-file', label: '新建空白文件', icon: FilePlus },
      { divider: true },
      { key: 'upload', label: '上传文件到此处', icon: Upload },
      { key: 'open-term', label: '在终端打开此目录', icon: Terminal },
      { divider: true },
      { key: 'refresh', label: '刷新当前目录', icon: RefreshCw }
    ]
  }

  const items: ContextMenuItem[] = []
  if (!node.isDir) {
    items.push({ key: 'download', label: '下载到本地', icon: Download })
    if (node.editable) {
      items.push({ key: 'edit', label: '在线编辑代码', icon: Edit3 })
    }
  } else {
    items.push({ key: 'download-dir', label: '下载整个目录', icon: Download })
    items.push({ key: 'open-term', label: '在右侧终端打开', icon: Terminal })
  }

  items.push({ divider: true })
  items.push({ key: 'copy-path', label: '复制绝对路径', icon: Copy })
  items.push({ key: 'new-folder-child', label: '在此新建子目录', icon: FolderPlus })
  items.push({ divider: true })
  items.push({ key: 'rename', label: '重命名', icon: Compass })
  items.push({ key: 'chmod', label: '修改权限', icon: Lock })
  items.push({ key: 'chown', label: '修改属主/属组', icon: Users })
  items.push({ key: 'props', label: '属性', icon: Info })
  items.push({ divider: true })
  items.push({ key: 'delete', label: '删除', icon: Trash2, danger: true })
  items.push({ divider: true })
  items.push({ key: 'refresh', label: '刷新', icon: RefreshCw })

  return items
})

const joinPath = (dir: string, name: string) => {
  if (dir === '/') return `/${name}`
  return `${dir.replace(/\/+$/, '')}/${name}`
}

/** 从绝对路径反推父目录（用 lastIndexOf 而非字符串替换） */
const parentDirOf = (p: string) => {
  const idx = p.lastIndexOf('/')
  return idx <= 0 ? '/' : p.slice(0, idx)
}

/** 从 "-rw-r--r-- (644)" 中提取八进制位，供属性/权限弹窗做默认值 */
const currentOctal = (permissions: string) => {
  const m = permissions.match(/\((\d{3,4})\)/)
  return m ? m[1] : '644'
}

/** FileTreeNode -> SftpFileItem，供下载/属性复用 */
const toSftpItem = (node: FileTreeNode): SftpFileItem => ({
  name: node.name,
  isDir: node.isDir,
  size: node.size,
  sizeBytes: node.sizeBytes,
  permissions: node.permissions,
  owner: node.owner,
  group: node.group,
  modifiedTime: node.modifiedTime,
  editable: node.editable,
  isHidden: node.isHidden
})

// 加载指定目录下的子项
const fetchDirChildren = async (dirPath: string): Promise<FileTreeNode[]> => {
  if (!props.sessionId) return []
  try {
    const rawItems = await sftpReadDirApi(props.sessionId, dirPath)
    return rawItems
      .filter(item => item.name !== '..')
      .map(item => ({
        path: joinPath(dirPath, item.name),
        name: item.name,
        isDir: item.isDir,
        size: item.size,
        sizeBytes: item.sizeBytes,
        permissions: item.permissions,
        owner: item.owner,
        group: item.group,
        modifiedTime: item.modifiedTime,
        editable: item.editable,
        isHidden: item.isHidden,
        children: item.isDir ? [] : undefined,
        expanded: false,
        loading: false,
        loaded: false
      }))
  } catch (err: any) {
    emit('toast', `读取目录 ${dirPath} 失败: ${err?.message || err}`, 'error')
    return []
  }
}

/** 展开 `~` / `~/xxx`。家目录探测失败时，纯 `~` 回退到根目录，避免文件区空着。 */
const expandHome = async (targetPath: string): Promise<string> => {
  const trimmed = targetPath.trim()
  if (!props.sessionId) return trimmed || '/'
  if (trimmed !== '~' && !trimmed.startsWith('~/')) return trimmed || '/'
  const home = await sftpHomeDirApi(props.sessionId).catch(() => '')
  if (!home) return trimmed === '~' ? '/' : trimmed
  if (trimmed === '~') return home
  const rest = trimmed.slice(2).replace(/^\/+/, '')
  return rest ? `${home.replace(/\/+$/, '')}/${rest}` : home
}

// 丢弃过期请求：家目录探测比直接列 `/` 慢，不能让后发的跳转被先发的结果盖掉
let loadSeq = 0

// 初始化加载家目录或指定目录
const loadInitialPath = async (targetPath: string) => {
  if (!props.sessionId) return
  const seq = ++loadSeq
  loading.value = true
  try {
    const resolved = await expandHome(targetPath)
    if (seq !== loadSeq) return
    currentPath.value = resolved
    if (resolved.startsWith('/')) emit('update:currentPath', resolved)
    const children = await fetchDirChildren(resolved)
    if (seq !== loadSeq) return
    rootNodes.value = children
  } finally {
    if (seq === loadSeq) {
      loading.value = false
      pathReady.value = true
    }
  }
}

// 展开/收起目录节点
const handleToggleNode = async (node: FileTreeNode) => {
  if (!node.isDir) return

  if (viewMode.value === 'list') {
    // 列表模式下双击进入目录
    currentPath.value = node.path
    loading.value = true
    try {
      rootNodes.value = await fetchDirChildren(node.path)
    } finally {
      loading.value = false
    }
    return
  }

  // 树形模式
  if (!node.loaded) {
    node.loading = true
    try {
      const children = await fetchDirChildren(node.path)
      node.children = children
      node.loaded = true
      node.expanded = true
    } finally {
      node.loading = false
    }
  } else {
    node.expanded = !node.expanded
  }
}

const handleSelectNode = (node: FileTreeNode) => {
  selectedPath.value = node.path
  selectedNode.value = node
}

const handleOpenFile = (node: FileTreeNode) => {
  if (!node.isDir && node.editable) {
    emit('edit-file', node.path)
  } else if (!node.isDir) {
    const sftpItem: SftpFileItem = {
      name: node.name,
      isDir: node.isDir,
      size: node.size,
      sizeBytes: node.sizeBytes,
      permissions: node.permissions,
      owner: node.owner,
      group: node.group,
      modifiedTime: node.modifiedTime,
      editable: node.editable,
      isHidden: node.isHidden
    }
    emit('download-file', sftpItem, node.path)
  }
}

const jumpToPath = async (p: string) => {
  await loadInitialPath(p)
}

const jumpToSegment = async (index: number) => {
  const parts = pathSegments.value.slice(0, index + 1)
  const p = '/' + parts.join('/')
  await jumpToPath(p)
}

const navigateUp = async () => {
  const parts = pathSegments.value.slice(0, -1)
  const p = parts.length === 0 ? '/' : '/' + parts.join('/')
  await jumpToPath(p)
}

const startEditPath = () => {
  pathInputBuffer.value = currentPath.value
  editingPathInput.value = true
  nextTick(() => {
    pathInputRef.value?.focus()
    pathInputRef.value?.select()
  })
}

const submitPathInput = () => {
  editingPathInput.value = false
  if (pathInputBuffer.value.trim()) {
    jumpToPath(pathInputBuffer.value.trim())
  }
}

const jumpBookmark = async (path: string) => {
  showBookmarks.value = false
  await loadInitialPath(path)
}

const refreshCurrent = async () => {
  await loadInitialPath(currentPath.value)
}

const openInTerminal = (path: string) => {
  // 家目录还没解析完时 currentPath 仍是占位 `/`，不能当成用户要打开的目录
  if (!pathReady.value || !path.startsWith('/')) return
  emit('open-in-terminal', path)
}

const triggerUpload = () => {
  const targetDir = selectedNode.value?.isDir ? selectedNode.value.path : currentPath.value
  if (!pathReady.value || !targetDir.startsWith('/')) return
  emit('upload-trigger', targetDir)
}

const createNewFolder = async () => {
  if (!props.sessionId) return
  const targetDir = selectedNode.value?.isDir ? selectedNode.value.path : currentPath.value
  const name = await promptDialog({
    title: '新建文件夹',
    message: `在 ${targetDir} 下创建新文件夹：`,
    input: {
      label: '文件夹名称',
      placeholder: '例如: logs, build, backup',
      required: true
    },
    confirmText: '创建'
  })
  if (!name || !name.trim()) return

  const fullPath = joinPath(targetDir, name.trim())
  try {
    await sftpMkdirApi(props.sessionId, fullPath)
    emit('toast', `文件夹「${name}」创建成功`, 'success')
    await refreshCurrent()
  } catch (err: any) {
    emit('toast', `创建失败: ${err?.message || err}`, 'error')
  }
}

/** 新建空白文件（空区域菜单） */
const createNewFile = async () => {
  if (!props.sessionId) return
  const targetDir = selectedNode.value?.isDir ? selectedNode.value.path : currentPath.value
  const name = await promptDialog({
    title: '新建空白文件',
    message: `在 ${targetDir} 下创建新文件：`,
    input: {
      label: '文件名',
      placeholder: '例如: app.log, config.yaml',
      required: true
    },
    confirmText: '创建'
  })
  if (!name || !name.trim()) return
  const fileName = name.trim()
  if (fileName.includes('/')) {
    emit('toast', '文件名不能包含 "/"', 'warning')
    return
  }
  try {
    await sftpTouchApi(props.sessionId, joinPath(targetDir, fileName))
    emit('toast', `文件「${fileName}」创建成功`, 'success')
    await refreshCurrent()
  } catch (err: any) {
    emit('toast', `创建失败: ${err?.message || err}`, 'error')
  }
}

/** 重命名（目标已存在时先询问是否覆盖） */
const renameNode = async (node: FileTreeNode) => {
  if (!props.sessionId) return
  const input = await promptDialog({
    title: node.isDir ? '重命名目录' : '重命名文件',
    message: `为 ${node.name} 输入新名称：`,
    detail: node.path,
    input: { label: '新名称', value: node.name, placeholder: node.name, required: true },
    confirmText: '重命名'
  })
  if (!input) return
  const newName = input.trim()
  if (!newName || newName === node.name) return
  if (newName.includes('/') || newName === '.' || newName === '..') {
    emit('toast', '名称无效（不能包含 "/"）', 'warning')
    return
  }

  const target = joinPath(parentDirOf(node.path), newName)
  let overwrite = false
  const existing = await sftpStatApi(props.sessionId, target).catch(() => null)
  if (existing) {
    overwrite = await confirmDialog({
      title: '目标已存在',
      variant: 'warning',
      message: `${newName} 已存在，确定要覆盖吗？`,
      detail: target,
      confirmText: '覆盖',
      cancelText: '取消'
    })
    if (!overwrite) return
  }

  try {
    await sftpRenameApi(props.sessionId, node.path, target, overwrite)
    emit('toast', `已重命名为 ${newName}`, 'success')
    await refreshCurrent()
  } catch (err: any) {
    emit('toast', `重命名失败: ${err?.message || err}`, 'error')
  }
}

/** 修改权限位（八进制 3~4 位） */
const changePermissions = async (node: FileTreeNode) => {
  if (!props.sessionId) return
  const input = await promptDialog({
    title: '修改权限',
    message: `修改 ${node.name} 的权限位（八进制）：`,
    detail: `${node.path}\n当前: ${node.permissions}`,
    input: {
      label: '权限（3~4 位八进制）',
      value: currentOctal(node.permissions),
      placeholder: '755',
      required: true
    },
    confirmText: '应用'
  })
  if (!input) return
  const mode = input.trim()
  if (!/^[0-7]{3,4}$/.test(mode)) {
    emit('toast', '权限格式不正确，请输入 3~4 位八进制（如 644、0755）', 'warning')
    return
  }
  try {
    await sftpChmodApi(props.sessionId, node.path, parseInt(mode, 8))
    emit('toast', `已修改 ${node.name} 权限为 ${mode}`, 'success')
    await refreshCurrent()
  } catch (err: any) {
    emit('toast', `修改权限失败: ${err?.message || err}`, 'error')
  }
}

/** 修改属主 / 属组（数字 id，格式 uid:gid） */
const changeOwnership = async (node: FileTreeNode) => {
  if (!props.sessionId) return
  const input = await promptDialog({
    title: '修改属主 / 属组',
    message: `修改 ${node.name} 的属主/属组（数字 id）：`,
    detail: `${node.path}\n当前: uid=${node.owner} gid=${node.group}\n格式 uid:gid，留空表示不修改该项`,
    input: { label: 'uid:gid', value: '', placeholder: '例如 1000:1000', required: true },
    confirmText: '应用'
  })
  if (input === null) return

  const [uidStr, gidStr] = input.trim().split(':')
  const parseId = (s?: string): number | undefined | 'invalid' => {
    if (s === undefined || s.trim() === '') return undefined
    return /^\d+$/.test(s.trim()) ? Number(s.trim()) : 'invalid'
  }
  const uid = parseId(uidStr)
  const gid = parseId(gidStr)
  if (uid === 'invalid' || gid === 'invalid') {
    emit('toast', 'uid / gid 必须是数字', 'warning')
    return
  }
  if (uid === undefined && gid === undefined) {
    emit('toast', '未填写 uid 或 gid', 'warning')
    return
  }

  try {
    await sftpChownApi(props.sessionId, node.path, uid, gid)
    emit('toast', `已更新 ${node.name} 的属主/属组`, 'success')
    await refreshCurrent()
  } catch (err: any) {
    emit('toast', `修改属主失败: ${err?.message || err}`, 'error')
  }
}

/** 属性查看（优先取远端最新 stat，失败时用列表缓存） */
const showProperties = async (node: FileTreeNode) => {
  const remote = props.sessionId
    ? await sftpStatApi(props.sessionId, node.path).catch(() => null)
    : null
  const item = remote ?? toSftpItem(node)
  await confirmDialog({
    title: '文件属性',
    message: item.name,
    detail: [
      `路径: ${node.path}`,
      `类型: ${item.isDir ? '目录' : '普通文件'}`,
      `大小: ${item.size}（${item.sizeBytes} 字节）`,
      `权限: ${item.permissions}`,
      `属主: uid=${item.owner}`,
      `属组: gid=${item.group}`,
      `修改时间: ${item.modifiedTime}`
    ].join('\n'),
    confirmText: '关闭',
    hideCancel: true
  })
}

// 右键处理
const handleNodeContextMenu = (evt: MouseEvent, node: FileTreeNode) => {
  evt.stopPropagation()
  selectedPath.value = node.path
  selectedNode.value = node
  contextMenu.value = {
    open: true,
    x: evt.clientX,
    y: evt.clientY,
    targetNode: node
  }
}

const handleEmptyAreaContextMenu = (evt: MouseEvent) => {
  contextMenu.value = {
    open: true,
    x: evt.clientX,
    y: evt.clientY,
    targetNode: null
  }
}

const handleContextMenuSelect = async (item: ContextMenuItem) => {
  const node = contextMenu.value.targetNode
  if (!item.key) return

  switch (item.key) {
    case 'edit':
      if (node) emit('edit-file', node.path)
      break
    case 'download':
    case 'download-dir':
      if (node) {
        const sftpItem: SftpFileItem = {
          name: node.name,
          isDir: node.isDir,
          size: node.size,
          sizeBytes: node.sizeBytes,
          permissions: node.permissions,
          owner: node.owner,
          group: node.group,
          modifiedTime: node.modifiedTime,
          editable: node.editable,
          isHidden: node.isHidden
        }
        emit('download-file', sftpItem, node.path)
      }
      break
    case 'open-term': {
      const p = node ? (node.isDir ? node.path : currentPath.value) : currentPath.value
      emit('open-in-terminal', p)
      break
    }
    case 'copy-path':
      if (node) {
        await navigator.clipboard.writeText(node.path)
        emit('toast', '已复制绝对路径到剪贴板', 'success')
      }
      break
    case 'new-folder':
    case 'new-folder-child':
      await createNewFolder()
      break
    case 'new-file':
      await createNewFile()
      break
    case 'rename':
      if (node) await renameNode(node)
      break
    case 'chmod':
      if (node) await changePermissions(node)
      break
    case 'chown':
      if (node) await changeOwnership(node)
      break
    case 'props':
      if (node) await showProperties(node)
      break
    case 'delete':
      if (node && props.sessionId) {
        const ok = await confirmDialog({
          title: '确认删除',
          message: `确定要彻底删除 ${node.name} 吗？此操作无法撤销。`,
          detail: currentGroup.value.danger
            ? `当前处于高风险分组「${currentGroup.value.label}」，请确认路径无误：\n${node.path}`
            : node.path,
          variant: 'danger',
          confirmText: '删除',
          cancelText: '取消'
        })
        if (ok) {
          try {
            await sftpRemoveFileApi(props.sessionId, node.path)
            emit('toast', `已成功删除 ${node.name}`, 'success')
            await refreshCurrent()
          } catch (err: any) {
            emit('toast', `删除失败: ${err?.message || err}`, 'error')
          }
        }
      }
      break
    case 'refresh':
      await refreshCurrent()
      break
    case 'upload':
      triggerUpload()
      break
  }
}

const downloadSelectedNode = () => {
  const node = selectedNode.value
  if (!node) return
  const sftpItem: SftpFileItem = {
    name: node.name,
    isDir: node.isDir,
    size: node.size,
    sizeBytes: node.sizeBytes,
    permissions: node.permissions,
    owner: node.owner,
    group: node.group,
    modifiedTime: node.modifiedTime,
    editable: node.editable,
    isHidden: node.isHidden
  }
  emit('download-file', sftpItem, node.path)
}

watch(
  () => props.sessionId,
  (sid) => {
    if (!sid) return
    // 新会话 initialPath 为 ~；已浏览过的会话带回绝对路径，避免折叠侧边栏后回到根目录
    loadInitialPath(props.initialPath || '~')
  },
  { immediate: true }
)

// 批量上传/下载时每个文件完成都会推一次 refreshSignal：合并成一次刷新，
// 避免同一目录被重复读取，也让失效会话的重连重试不被重复请求掩盖。
let refreshTimer: ReturnType<typeof setTimeout> | null = null

watch(
  () => props.refreshSignal,
  () => {
    if (!props.sessionId || !pathReady.value) return
    if (refreshTimer) clearTimeout(refreshTimer)
    refreshTimer = setTimeout(() => {
      refreshTimer = null
      if (props.sessionId && pathReady.value) void refreshCurrent()
    }, 250)
  }
)

onUnmounted(() => {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = null
})

const syncing = ref(false)

const toggleAutoFollow = () => {
  const newVal = !props.autoFollow
  emit('update:autoFollow', newVal)
  emit('toast', newVal ? '已开启终端目录自动跟随' : '已关闭终端目录自动跟随', 'info')
}

const triggerManualSync = () => {
  syncing.value = true
  emit('manual-sync')
  setTimeout(() => {
    syncing.value = false
  }, 600)
}

defineExpose({
  jumpToPath,
  getCurrentPath: () => currentPath.value,
  refresh: refreshCurrent
})
</script>
