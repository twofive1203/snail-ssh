<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden bg-surface-base">
    <!-- SFTP 顶部工具控制栏 -->
    <div class="h-12 border-b border-surface-border px-4 flex items-center justify-between shrink-0 bg-surface-card/30">
      <!-- 目标主机选择器与当前路径面包屑 -->
      <div class="flex items-center gap-3 flex-1 mr-4">
        <!-- 主机切换下拉 -->
        <div class="relative">
          <button
            @click="showHostDropdown = !showHostDropdown"
            class="flex items-center gap-2 px-2.5 py-1 rounded-lg bg-surface-card border border-surface-border hover:border-brand-500/40 text-xs font-mono text-slate-200"
          >
            <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
            <span>{{ currentHost?.name || '选择服务器' }}</span>
            <ChevronDown class="w-3 h-3 text-slate-400" />
          </button>

          <!-- 下拉菜单 -->
          <div
            v-if="showHostDropdown"
            class="absolute left-0 top-full mt-1 w-52 bg-surface-sidebar border border-surface-border rounded-xl shadow-xl z-30 py-1"
          >
            <div
              v-for="h in hosts"
              :key="h.id"
              @click="switchHost(h)"
              class="px-3 py-1.5 hover:bg-surface-elevated text-xs flex items-center justify-between cursor-pointer"
            >
              <span class="font-mono text-slate-200">{{ h.name }}</span>
              <span class="text-[10px] text-slate-500">{{ h.host }}</span>
            </div>
          </div>
        </div>

        <!-- 面包屑路径 -->
        <div class="flex-1 flex items-center bg-surface-card border border-surface-border rounded-lg px-2.5 py-1 text-xs font-mono">
          <Folder class="w-3.5 h-3.5 text-amber-400 mr-2 shrink-0" />
          <div class="flex items-center gap-1 text-slate-300 overflow-x-auto no-scrollbar flex-1">
            <span
              @click="navigateTo('/')"
              class="hover:text-white cursor-pointer px-1 py-0.5 rounded hover:bg-surface-elevated font-semibold"
            >
              /
            </span>
            <template v-for="(seg, idx) in pathSegments" :key="idx">
              <span class="text-slate-600">/</span>
              <span
                @click="navigateToSegment(idx)"
                :class="idx === pathSegments.length - 1 ? 'text-brand-400 font-bold' : 'hover:text-white text-slate-300'"
                class="cursor-pointer px-1.5 py-0.5 rounded hover:bg-surface-elevated truncate max-w-[120px]"
              >
                {{ seg }}
              </span>
            </template>
          </div>
          <button @click="copyPath" class="text-slate-500 hover:text-white p-0.5 rounded transition-colors ml-2" title="复制完整路径">
            <Copy class="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <!-- 操作按钮：上传、新建、刷新、隐藏文件、终端联动 -->
      <div class="flex items-center gap-2 text-xs">
        <button
          @click="triggerUpload"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold transition-all shadow-glow-emerald"
        >
          <Upload class="w-3.5 h-3.5 stroke-[2.5]" />
          <span>上传文件</span>
        </button>

        <button
          @click="openNewFolderPrompt"
          class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-slate-300 hover:text-white transition-colors"
        >
          <FolderPlus class="w-3.5 h-3.5" />
          <span>新建文件夹</span>
        </button>

        <button
          @click="refreshFiles"
          class="p-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-slate-300 hover:text-white transition-colors"
          title="刷新目录"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        </button>

        <button
          @click="showHidden = !showHidden"
          :class="showHidden ? 'text-brand-400 bg-brand-500/10' : 'text-slate-300 bg-surface-card'"
          class="p-1.5 rounded-lg hover:bg-surface-elevated border border-surface-border transition-colors"
          title="显示/隐藏以点开头的文件"
        >
          <Eye class="w-3.5 h-3.5" />
        </button>

        <button
          @click="$emit('open-in-terminal', currentPath)"
          class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-slate-300 hover:text-white transition-colors"
          title="在此目录打开终端"
        >
          <Terminal class="w-3.5 h-3.5 text-emerald-400" />
          <span>在终端打开</span>
        </button>
      </div>
    </div>

    <!-- 多选操作栏 -->
    <div
      v-if="selectedFileNames.size > 0"
      class="h-9 px-4 flex items-center justify-between bg-brand-500/10 border-b border-brand-500/20 text-xs shrink-0"
    >
      <span class="text-brand-300 font-medium">已选中 {{ selectedFileNames.size }} 项</span>
      <div class="flex items-center gap-2">
        <button
          @click="downloadSelected"
          class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card hover:bg-surface-elevated border border-surface-border text-cyan-400 transition-colors"
        >
          <Download class="w-3.5 h-3.5" />
          <span>下载选中</span>
        </button>
        <button
          @click="deleteSelected"
          class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-surface-card hover:bg-surface-elevated border border-surface-border text-rose-400 transition-colors"
        >
          <Trash2 class="w-3.5 h-3.5" />
          <span>删除选中</span>
        </button>
        <button
          @click="selectedFileNames.clear()"
          class="px-2.5 py-1 rounded-md text-surface-subtle hover:text-white transition-colors"
        >
          取消选择
        </button>
      </div>
    </div>

    <!-- SFTP 表格区域 -->
    <div class="flex-1 overflow-y-auto custom-scrollbar select-none">
      <table class="w-full text-left text-xs border-collapse">
        <thead class="sticky top-0 bg-surface-sidebar border-b border-surface-border text-[11px] font-mono text-slate-400 uppercase tracking-wider z-10">
          <tr>
            <th class="py-2.5 px-4 font-medium w-12 text-center">
              <input
                type="checkbox"
                :checked="isAllSelected"
                @change="toggleSelectAll"
                class="rounded bg-surface-card border-surface-border text-brand-500 focus:ring-0"
              />
            </th>
            <th class="py-2.5 px-4 font-medium">名称</th>
            <th class="py-2.5 px-4 font-medium w-28 text-right">大小</th>
            <th class="py-2.5 px-4 font-medium w-36 font-mono">权限 (chmod)</th>
            <th class="py-2.5 px-4 font-medium w-28">用户/组</th>
            <th class="py-2.5 px-4 font-medium w-40">修改时间</th>
            <th class="py-2.5 px-4 font-medium w-28 text-center">操作</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-surface-border/40 font-mono">
          <tr
            v-for="file in visibleFiles"
            :key="file.name"
            @click="handleFileClick(file)"
            @dblclick="handleFileDoubleClick(file)"
            class="sftp-row hover:bg-surface-card/60 transition-colors group cursor-pointer"
            :class="{ 'bg-brand-500/10': selectedFileNames.has(file.name) }"
            :title="file.name === '..' ? '返回上级目录' : file.isDir ? '双击进入目录' : file.editable ? '双击在线编辑' : '双击下载到本地'"
          >
            <td class="py-2.5 px-4 text-center" @click.stop>
              <input
                v-if="file.name !== '..'"
                type="checkbox"
                :checked="selectedFileNames.has(file.name)"
                @change="toggleSelectFile(file.name)"
                class="rounded bg-surface-card border-surface-border text-brand-500 focus:ring-0"
              />
            </td>
            <td class="py-2.5 px-4">
              <div class="flex items-center gap-2.5 font-medium" :class="file.isDir ? 'text-white' : 'text-slate-200'">
                <component :is="getFileIcon(file)" class="w-4 h-4 shrink-0" :class="getFileIconColor(file)" />
                <span :class="file.isDir ? 'hover:underline text-cyan-300 font-semibold' : 'text-slate-200'">{{ file.name }}</span>
              </div>
            </td>
            <td class="py-2.5 px-4 text-right text-slate-400 text-[11px]">{{ file.size }}</td>
            <td class="py-2.5 px-4 text-slate-400 text-[11px]">{{ file.permissions }}</td>
            <td class="py-2.5 px-4 text-slate-400 text-[11px]">{{ file.owner }}:{{ file.group }}</td>
            <td class="py-2.5 px-4 text-slate-400 text-[11px]">{{ file.modifiedTime }}</td>
            <td class="py-2.5 px-4 text-center" @click.stop>
              <div class="flex items-center justify-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  v-if="file.editable"
                  @click="$emit('edit-file', joinPath(currentPath, file.name))"
                  class="p-1 rounded hover:bg-surface-elevated text-brand-400 hover:text-brand-300"
                  title="在线实时编辑"
                >
                  <Edit3 class="w-3.5 h-3.5" />
                </button>
                <button
                  v-if="file.name !== '..'"
                  @click="handleDownload(file)"
                  class="p-1 rounded hover:bg-surface-elevated text-cyan-400 hover:text-cyan-300"
                  :title="file.isDir ? '递归下载整个目录' : '下载至本地'"
                >
                  <Download class="w-3.5 h-3.5" />
                </button>
                <button
                  v-if="file.name !== '..'"
                  @click="handleDelete(file)"
                  class="p-1 rounded hover:bg-surface-elevated text-rose-400 hover:text-rose-300"
                  title="删除"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </td>
          </tr>

          <!-- 首次加载提示 -->
          <tr v-if="loading && files.length === 0">
            <td colspan="7" class="py-16 text-center text-xs text-slate-500 font-mono">
              <RefreshCw class="w-4 h-4 animate-spin inline mr-2 text-brand-400" />
              正在读取远端目录...
            </td>
          </tr>

          <!-- 空目录状态：后端总会返回 ".."，因此用是否有真实条目判断 -->
          <tr v-else-if="!loading && !hasRealEntries">
            <td colspan="7" class="py-16 text-center font-mono">
              <FolderOpen class="w-6 h-6 mx-auto mb-2 text-slate-600" />
              <div class="text-xs text-slate-400">此目录为空</div>
              <div class="text-[11px] text-slate-600 mt-1">
                {{ showHidden ? '也没有隐藏文件' : '若目录内只有隐藏文件，可点击右上角的眼睛图标查看' }}
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 底部状态指示条 -->
    <div class="h-8 bg-surface-sidebar border-t border-surface-border px-4 flex items-center justify-between text-xs text-surface-subtle shrink-0">
      <div class="flex items-center gap-4">
        <span>{{ visibleFiles.length }} 项</span>
        <span class="text-slate-600">|</span>
        <span :class="selectedFileNames.size > 0 ? 'text-brand-400 font-semibold' : ''">
          {{ selectedFileNames.size > 0 ? `已选中 ${selectedFileNames.size} 项` : '未选中项目' }}
        </span>
      </div>
      <div class="flex items-center text-[11px] font-mono text-slate-500">
        <span v-if="loading" class="flex items-center gap-1.5 text-brand-400">
          <RefreshCw class="w-3 h-3 animate-spin" />
          正在同步远端目录...
        </span>
        <span v-else :class="activeSessionId ? 'text-emerald-400' : 'text-amber-400'">
          {{ activeSessionId ? 'SFTP 通道已就绪' : '未建立 SSH 会话' }}
        </span>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import type { HostItem, RemoteFileRef, SftpFileItem } from '@/types'
import { sftpReadDirApi, sftpMkdirApi, sftpRemoveFileApi, sftpHomeDirApi } from '@/api/tauri'
import { confirmDialog, promptDialog } from '@/composables/useDialog'
import {
  Folder,
  FolderOpen,
  ChevronDown,
  Copy,
  Upload,
  FolderPlus,
  RefreshCw,
  Eye,
  Terminal,
  Edit3,
  Download,
  Trash2,
  CornerLeftUp,
  FileCode,
  FileText,
  Archive,
  ShieldCheck,
  File
} from 'lucide-vue-next'

const props = defineProps<{
  hosts: HostItem[]
  currentHost?: HostItem
  activeSessionId?: string
  /** 传输完成后由父组件自增，触发目录刷新 */
  refreshSignal?: number
}>()

const emit = defineEmits<{
  (e: 'select-host', host: HostItem): void
  (e: 'edit-file', fullPath: string): void
  (e: 'download-file', file: SftpFileItem, remotePath: string): void
  (e: 'download-files', refs: RemoteFileRef[]): void
  (e: 'upload-trigger', remoteDir: string): void
  (e: 'open-in-terminal', path: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const showHostDropdown = ref(false)
// 进入视图后由 sftp_home_dir 解析为远端家目录
const currentPath = ref('/')
const files = ref<SftpFileItem[]>([])
const loading = ref(false)
const showHidden = ref(false)
const selectedFileNames = ref<Set<string>>(new Set())

const joinPath = (dir: string, name: string) => (dir === '/' ? `/${name}` : `${dir}/${name}`)

const pathSegments = computed(() => {
  return currentPath.value.split('/').filter(Boolean)
})

const visibleFiles = computed(() => {
  return files.value.filter(f => showHidden.value || !f.isHidden)
})

// 当前选中的非 ".." 项
const selectedFiles = computed(() =>
  visibleFiles.value.filter(f => f.name !== '..' && selectedFileNames.value.has(f.name))
)

// 除 ".." 外是否有真实条目，用于空目录空状态
const hasRealEntries = computed(() => visibleFiles.value.some(f => f.name !== '..'))

// 远端家目录（失败时保留当前路径）
const resolveHome = async () => {
  if (!props.activeSessionId) return
  try {
    const home = await sftpHomeDirApi(props.activeSessionId)
    if (home) currentPath.value = home
  } catch (err) {
    console.warn('sftp_home_dir failed:', err)
  }
}

const isAllSelected = computed(() => {
  const selectable = visibleFiles.value.filter(f => f.name !== '..')
  return selectable.length > 0 && selectable.every(f => selectedFileNames.value.has(f.name))
})

const loadFiles = async () => {
  // 没有会话时不发请求，避免出现"会话不存在"的报错
  if (!props.activeSessionId) {
    files.value = []
    selectedFileNames.value.clear()
    return
  }

  loading.value = true
  try {
    const list = await sftpReadDirApi(props.activeSessionId, currentPath.value)
    files.value = list
    selectedFileNames.value.clear()
  } catch (err: any) {
    emit('toast', `加载目录失败: ${err?.message || err}`, 'error')
  } finally {
    loading.value = false
  }
}

const navigateTo = (path: string) => {
  currentPath.value = path
  loadFiles()
}

const navigateToSegment = (idx: number) => {
  const target = '/' + pathSegments.value.slice(0, idx + 1).join('/')
  navigateTo(target)
}

const handleFileClick = (file: SftpFileItem) => {
  if (file.isDir) {
    if (file.name === '..') {
      const segs = pathSegments.value.slice(0, -1)
      navigateTo('/' + segs.join('/'))
    } else {
      navigateTo(joinPath(currentPath.value, file.name))
    }
  } else if (file.editable) {
    emit('edit-file', joinPath(currentPath.value, file.name))
  }
  // 不可编辑文件单击不做任何事，避免误触下载；改用双击
}

// 双击：目录进入、可编辑文件打开编辑器、其余文件直接下载
const handleFileDoubleClick = (file: SftpFileItem) => {
  if (file.isDir || file.editable) return
  handleDownload(file)
}

const switchHost = (host: HostItem) => {
  showHostDropdown.value = false
  emit('select-host', host)
}

const copyPath = () => {
  navigator.clipboard?.writeText(currentPath.value)
  emit('toast', `已复制路径: ${currentPath.value}`, 'success')
}

const refreshFiles = () => {
  loadFiles()
}

const triggerUpload = () => {
  emit('upload-trigger', currentPath.value)
}

const openNewFolderPrompt = async () => {
  if (!props.activeSessionId) {
    emit('toast', '尚未建立 SSH 会话，无法创建目录', 'warning')
    return
  }
  const name = await promptDialog({
    title: '新建文件夹',
    message: '将在当前目录下创建文件夹：',
    detail: currentPath.value,
    input: { label: '文件夹名称', placeholder: 'new_folder', value: '', required: true },
    confirmText: '创建'
  })
  if (!name) return

  if (name.includes('/')) {
    emit('toast', '文件夹名称不能包含 "/"', 'warning')
    return
  }

  try {
    await sftpMkdirApi(props.activeSessionId, joinPath(currentPath.value, name))
    emit('toast', `已新建文件夹: ${name}`, 'success')
    loadFiles()
  } catch (e: any) {
    emit('toast', `新建文件夹失败: ${e?.message || e}`, 'error')
  }
}

const handleDownload = (file: SftpFileItem) => {
  emit('download-file', file, joinPath(currentPath.value, file.name))
}

// 批量下载：文件与目录均支持（目录由后端递归下载）
const downloadSelected = () => {
  const refs = selectedFiles.value.map(f => ({ file: f, remotePath: joinPath(currentPath.value, f.name) }))
  if (refs.length === 0) {
    emit('toast', '请先选择要下载的文件或目录', 'warning')
    return
  }
  emit('download-files', refs)
}

// 批量删除：后端递归删除目录
const deleteSelected = async () => {
  if (!props.activeSessionId) {
    emit('toast', '尚未建立 SSH 会话，无法删除', 'warning')
    return
  }
  const targets = selectedFiles.value
  if (targets.length === 0) return
  const label = targets.length === 1 ? targets[0].name : `${targets.length} 项`
  const hasDir = targets.some(t => t.isDir)
  const confirmed = await confirmDialog({
    title: '删除远端文件',
    variant: 'danger',
    message: `确定要删除 ${label} 吗？此操作不可撤销。`,
    detail: hasDir
      ? `其中包含目录，目录内的所有内容都会被递归删除。\n\n${targets.map(t => t.name + (t.isDir ? '/' : '')).join('\n')}`
      : currentPath.value,
    confirmText: '删除',
    cancelText: '取消'
  })
  if (!confirmed) return

  const sessionId = props.activeSessionId
  let failed = 0
  for (const file of targets) {
    try {
      await sftpRemoveFileApi(sessionId, joinPath(currentPath.value, file.name))
    } catch (e: any) {
      failed++
      emit('toast', `删除 ${file.name} 失败: ${e?.message || e}`, 'error')
    }
  }
  selectedFileNames.value.clear()
  await loadFiles()
  if (failed === 0) emit('toast', `已删除 ${label}`, 'success')
  else emit('toast', `${label} 中有 ${failed} 项删除失败`, 'error')
}

const handleDelete = async (file: SftpFileItem) => {
  if (!props.activeSessionId) {
    emit('toast', '尚未建立 SSH 会话，无法删除', 'warning')
    return
  }
  const confirmed = await confirmDialog({
    title: file.isDir ? '删除远端目录' : '删除远端文件',
    variant: 'danger',
    message: `确定要删除 ${file.name} 吗？此操作不可撤销。`,
    detail: file.isDir
      ? `目录内的所有内容都会被递归删除。\n\n${joinPath(currentPath.value, file.name)}`
      : joinPath(currentPath.value, file.name),
    confirmText: '删除',
    cancelText: '取消'
  })
  if (!confirmed) return

  try {
    await sftpRemoveFileApi(props.activeSessionId, joinPath(currentPath.value, file.name))
    emit('toast', `已删除 ${file.name}`, 'success')
    await loadFiles()
  } catch (e: any) {
    emit('toast', `删除失败: ${e?.message || e}`, 'error')
  }
}

const toggleSelectAll = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked
  if (checked) {
    visibleFiles.value.forEach(f => {
      if (f.name !== '..') selectedFileNames.value.add(f.name)
    })
  } else {
    selectedFileNames.value.clear()
  }
}

const toggleSelectFile = (name: string) => {
  if (selectedFileNames.value.has(name)) {
    selectedFileNames.value.delete(name)
  } else {
    selectedFileNames.value.add(name)
  }
}

const getFileIcon = (file: SftpFileItem) => {
  if (file.name === '..') return CornerLeftUp
  if (file.isDir) return Folder
  if (file.name.endsWith('.conf') || file.name.endsWith('.yaml') || file.name.endsWith('.json')) return FileCode
  if (file.name.endsWith('.log')) return FileText
  if (file.name.endsWith('.tar.gz') || file.name.endsWith('.zip')) return Archive
  if (file.name.endsWith('.crt') || file.name.endsWith('.key')) return ShieldCheck
  return File
}

const getFileIconColor = (file: SftpFileItem) => {
  if (file.name === '..') return 'text-slate-400'
  if (file.isDir) return 'text-amber-400 fill-amber-400/20'
  if (file.name.endsWith('.conf') || file.name.endsWith('.yaml')) return 'text-brand-400'
  if (file.name.endsWith('.log')) return 'text-blue-400'
  if (file.name.endsWith('.tar.gz')) return 'text-purple-400'
  return 'text-slate-400'
}

onMounted(async () => {
  if (props.activeSessionId) await resolveHome()
  loadFiles()
})

// 主机/会话切换后重新解析家目录并加载
watch(
  () => [props.activeSessionId, props.currentHost?.id].join('|'),
  async () => {
    currentPath.value = '/'
    if (props.activeSessionId) await resolveHome()
    loadFiles()
  }
)

// 传输完成后刷新当前目录，让新上传/下载的文件立刻可见
watch(
  () => props.refreshSignal,
  (val, old) => {
    if (val === undefined || old === undefined) return
    if (!props.activeSessionId) return
    loadFiles()
  }
)
</script>
