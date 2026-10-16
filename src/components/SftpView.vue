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
            class="sftp-row hover:bg-surface-card/60 transition-colors group cursor-pointer"
            :class="{ 'bg-brand-500/10': selectedFileNames.has(file.name) }"
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
                  @click="$emit('edit-file', currentPath + '/' + file.name)"
                  class="p-1 rounded hover:bg-surface-elevated text-brand-400 hover:text-brand-300"
                  title="在线实时编辑"
                >
                  <Edit3 class="w-3.5 h-3.5" />
                </button>
                <button
                  v-if="!file.isDir"
                  @click="handleDownload(file)"
                  class="p-1 rounded hover:bg-surface-elevated text-cyan-400 hover:text-cyan-300"
                  title="下载至本地"
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
      <!-- 磁盘容量指示条 -->
      <div class="flex items-center gap-2">
        <span class="text-[11px] font-mono text-slate-400">远端磁盘 /dev/vda1: 42.8 GB / 100 GB (42%)</span>
        <div class="w-24 h-1.5 bg-surface-highlight rounded-full overflow-hidden">
          <div class="h-full bg-brand-500 rounded-full" style="width: 42%;"></div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import type { HostItem, SftpFileItem } from '@/types'
import { sftpReadDirApi, sftpMkdirApi, sftpRemoveFileApi } from '@/api/tauri'
import {
  Folder,
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
}>()

const emit = defineEmits<{
  (e: 'select-host', host: HostItem): void
  (e: 'edit-file', fullPath: string): void
  (e: 'download-file', file: SftpFileItem, remotePath: string): void
  (e: 'upload-trigger', remoteDir: string): void
  (e: 'open-in-terminal', path: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const showHostDropdown = ref(false)
const currentPath = ref('/etc/nginx')
const files = ref<SftpFileItem[]>([])
const loading = ref(false)
const showHidden = ref(false)
const selectedFileNames = ref<Set<string>>(new Set())

const pathSegments = computed(() => {
  return currentPath.value.split('/').filter(Boolean)
})

const visibleFiles = computed(() => {
  return files.value.filter(f => showHidden.value || !f.isHidden)
})

const isAllSelected = computed(() => {
  const selectable = visibleFiles.value.filter(f => f.name !== '..')
  return selectable.length > 0 && selectable.every(f => selectedFileNames.value.has(f.name))
})

const loadFiles = async () => {
  loading.value = true
  try {
    const list = await sftpReadDirApi(props.activeSessionId || 'default', currentPath.value)
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
      const nextPath = currentPath.value === '/' ? `/${file.name}` : `${currentPath.value}/${file.name}`
      navigateTo(nextPath)
    }
  } else if (file.editable) {
    emit('edit-file', `${currentPath.value}/${file.name}`)
  }
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
  emit('toast', '已刷新目录', 'info')
}

const triggerUpload = () => {
  emit('upload-trigger', currentPath.value)
}

const openNewFolderPrompt = async () => {
  const name = prompt('请输入新建文件夹名称:', 'new_folder')
  if (name) {
    try {
      await sftpMkdirApi(props.activeSessionId || 'default', `${currentPath.value}/${name}`)
      emit('toast', `已新建文件夹: ${name}`, 'success')
      loadFiles()
    } catch (e: any) {
      emit('toast', `新建文件夹失败: ${e?.message || e}`, 'error')
    }
  }
}

const handleDownload = (file: SftpFileItem) => {
  emit('download-file', file, `${currentPath.value}/${file.name}`)
}

const handleDelete = async (file: SftpFileItem) => {
  if (confirm(`确定要删除 ${file.name} 吗？`)) {
    try {
      await sftpRemoveFileApi(props.activeSessionId || 'default', `${currentPath.value}/${file.name}`)
      emit('toast', `已删除 ${file.name}`, 'warning')
      loadFiles()
    } catch (e: any) {
      emit('toast', `删除失败: ${e?.message || e}`, 'error')
    }
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

onMounted(() => {
  loadFiles()
})
</script>
