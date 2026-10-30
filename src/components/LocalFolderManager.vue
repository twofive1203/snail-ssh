<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden">
    <div class="h-14 px-6 border-b border-surface-border flex items-center justify-between shrink-0 bg-surface-card/20 gap-4">
      <div class="min-w-0">
        <h1 class="text-sm font-bold text-white flex items-center gap-2">
          <span>本地目录</span>
          <span class="text-xs px-2 py-0.5 rounded-full bg-surface-card border border-surface-border text-surface-subtle font-mono font-normal">
            {{ settings.localFolders.length }} / {{ LOCAL_FOLDER_LIMIT }}
          </span>
        </h1>
        <p class="text-[11px] text-surface-subtle truncate">点击一条目录才打开终端。已有标签会聚焦，不会再开一个工作区</p>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        <div class="relative w-56">
          <Search class="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="搜索名称或路径..."
            class="w-full bg-surface-card border border-surface-border rounded-lg pl-9 pr-3 py-1.5 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-brand-500/60 focus:ring-1 focus:ring-brand-500/40 transition-all font-mono"
          />
        </div>
        <button
          @click="emit('open-home')"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-300 hover:text-white transition-all"
          title="在用户主目录打开一个本地终端。再次点击只聚焦，不新开进程"
        >
          <House class="w-3.5 h-3.5 text-sky-300" />
          <span>在用户主目录打开</span>
        </button>
        <button
          v-if="desktop"
          @click="addFromPicker"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-300 hover:text-white transition-all"
          title="用系统目录选择器添加"
        >
          <FolderPlus class="w-3.5 h-3.5" />
          <span>添加目录</span>
        </button>
        <button
          @click="addFromText"
          class="flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all"
          title="粘贴一条本地路径"
        >
          <Plus class="w-4 h-4 stroke-[2.5]" />
          <span>{{ desktop ? '粘贴路径' : '添加路径' }}</span>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <div v-if="settings.localFolders.length === 0" class="flex flex-col items-center justify-center p-16 text-center">
        <div class="w-14 h-14 rounded-2xl bg-surface-card border border-surface-border flex items-center justify-center text-slate-500 mb-3">
          <FolderOpen class="w-7 h-7" />
        </div>
        <div class="text-sm font-semibold text-slate-300">还没有本地目录</div>
        <div class="text-xs text-slate-500 mt-1 max-w-md">
          {{ desktop
            ? '添加一个目录后再打开终端。也可以直接在用户主目录打开，这不会写入列表。'
            : '浏览器预览没有目录选择器，粘贴路径只会更新列表，不会启动本机进程。' }}
        </div>
      </div>

      <div v-else-if="filteredFolders.length === 0" class="flex flex-col items-center justify-center p-16 text-center">
        <div class="text-sm font-semibold text-slate-300">没有匹配的目录</div>
        <div class="text-xs text-slate-500 mt-1">试试名称或路径里的另一段文字</div>
        <button
          @click="searchQuery = ''"
          class="mt-4 px-3.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-brand-400"
        >
          清空搜索
        </button>
      </div>

      <div v-else class="space-y-2">
        <div
          v-for="folder in filteredFolders"
          :key="folder.id"
          class="rounded-xl border border-surface-border bg-surface-card/40"
        >
          <form v-if="editingId === folder.id" class="p-3 space-y-2" @submit.prevent="saveEdit(folder)">
            <div class="grid grid-cols-1 md:grid-cols-[180px_minmax(0,1fr)] gap-2">
              <input
                v-model="draftLabel"
                type="text"
                spellcheck="false"
                placeholder="显示名，留空则用路径最后一段"
                class="bg-surface-base border border-surface-border rounded-lg px-3 py-1.5 text-xs text-white focus:outline-none focus:border-brand-500"
              />
              <div class="flex items-center gap-2 min-w-0">
                <input
                  v-model="draftPath"
                  type="text"
                  spellcheck="false"
                  placeholder="目录路径"
                  class="flex-1 min-w-0 bg-surface-base border border-surface-border rounded-lg px-3 py-1.5 text-xs text-white font-mono focus:outline-none focus:border-brand-500"
                />
                <button
                  v-if="desktop"
                  type="button"
                  @click="browseDraft"
                  class="px-2.5 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-slate-300 hover:text-white"
                >
                  选择
                </button>
              </div>
            </div>
            <div class="flex items-center justify-end gap-2">
              <button type="button" @click="editingId = null" class="px-3 py-1.5 rounded-lg text-xs text-slate-400 hover:text-white">取消</button>
              <button type="submit" class="px-3 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 text-xs font-semibold">保存</button>
            </div>
          </form>

          <div
            v-else
            class="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-surface-card/80 transition-colors"
            @click="emit('open-folder', folder)"
          >
            <Folder class="w-4 h-4 text-sky-400 shrink-0" />
            <div class="min-w-0 flex-1">
              <div class="text-xs font-medium text-slate-100 truncate">{{ folder.label }}</div>
              <div class="text-[11px] font-mono text-slate-500 truncate" :title="folder.path">{{ folder.path }}</div>
            </div>
            <button
              @click.stop="emit('open-folder', folder)"
              class="px-2.5 py-1 rounded-md text-[11px] text-emerald-300 hover:bg-emerald-500/10"
            >
              打开
            </button>
            <button
              @click.stop="startEdit(folder)"
              class="p-1.5 rounded text-surface-subtle hover:text-white hover:bg-surface-highlight"
              title="修改名称和路径"
            >
              <Pencil class="w-3.5 h-3.5" />
            </button>
            <button
              @click.stop="removeFolder(folder)"
              class="p-1.5 rounded text-surface-subtle hover:text-rose-400 hover:bg-surface-highlight"
              title="从列表删除。已打开的终端不会关闭"
            >
              <Trash2 class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { Folder, FolderOpen, FolderPlus, House, Pencil, Plus, Search, Trash2 } from 'lucide-vue-next'
import { flushSettings, settings } from '@/composables/useAppSettings'
import { confirmDialog, promptDialog } from '@/composables/useDialog'
import { isTauri, pickDirectory } from '@/api/tauri'
import type { LocalFolderItem } from '@/types'
import { folderLabelFromPath, LOCAL_FOLDER_LIMIT, sameLocalPath } from '@/utils/localFolder'

const emit = defineEmits<{
  (e: 'open-folder', folder: LocalFolderItem): void
  (e: 'open-home'): void
  (e: 'toast', text: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const desktop = isTauri()
const searchQuery = ref('')
const editingId = ref<string | null>(null)
const draftLabel = ref('')
const draftPath = ref('')

const filteredFolders = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return settings.localFolders
  return settings.localFolders.filter(folder =>
    folder.label.toLowerCase().includes(q) || folder.path.toLowerCase().includes(q)
  )
})

const newId = () => `lf-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const duplicateOf = (path: string, ignoreId?: string) =>
  settings.localFolders.find(folder => folder.id !== ignoreId && sameLocalPath(folder.path, path))

const commit = async (next: LocalFolderItem[], message: string) => {
  settings.localFolders = next
  await flushSettings()
  emit('toast', message, 'success')
}

const addPath = async (path: string) => {
  const trimmed = path.trim()
  if (!trimmed) {
    emit('toast', '路径不能为空', 'warning')
    return
  }
  if (settings.localFolders.length >= LOCAL_FOLDER_LIMIT) {
    emit('toast', `本地目录最多 ${LOCAL_FOLDER_LIMIT} 条`, 'warning')
    return
  }
  const existing = duplicateOf(trimmed)
  if (existing) {
    emit('toast', `已有相同路径：${existing.label}`, 'warning')
    return
  }
  const item: LocalFolderItem = {
    id: newId(),
    label: folderLabelFromPath(trimmed),
    path: trimmed
  }
  await commit([...settings.localFolders, item], `已添加 ${item.label}`)
}

const addFromPicker = async () => {
  try {
    const picked = await pickDirectory()
    if (!picked) return
    await addPath(picked)
  } catch (e: any) {
    emit('toast', `选择目录失败: ${e?.message || e}`, 'error')
  }
}

const addFromText = async () => {
  const path = await promptDialog({
    title: '添加本地目录',
    message: '粘贴要打开的目录路径。不会检查这个路径现在是否存在。',
    input: { label: '路径', placeholder: '例如 D:\\work', required: true },
    confirmText: '添加'
  })
  if (!path) return
  await addPath(path)
}

const startEdit = (folder: LocalFolderItem) => {
  editingId.value = folder.id
  draftLabel.value = folder.label
  draftPath.value = folder.path
}

const browseDraft = async () => {
  try {
    const picked = await pickDirectory()
    if (picked) draftPath.value = picked
  } catch (e: any) {
    emit('toast', `选择目录失败: ${e?.message || e}`, 'error')
  }
}

const saveEdit = async (folder: LocalFolderItem) => {
  const path = draftPath.value.trim()
  if (!path) {
    emit('toast', '路径不能为空', 'warning')
    return
  }
  const existing = duplicateOf(path, folder.id)
  if (existing) {
    emit('toast', `已有相同路径：${existing.label}`, 'warning')
    return
  }
  const label = draftLabel.value.trim() || folderLabelFromPath(path)
  editingId.value = null
  await commit(
    settings.localFolders.map(item => item.id === folder.id ? { ...item, label, path } : item),
    '已更新目录'
  )
}

const removeFolder = async (folder: LocalFolderItem) => {
  const confirmed = await confirmDialog({
    title: '删除本地目录',
    variant: 'danger',
    message: `确定从列表中删除「${folder.label}」吗？`,
    detail: '已打开的终端不会关闭。这个目录从列表删除后，重连会使用打开时记下的路径。',
    confirmText: '删除',
    cancelText: '取消'
  })
  if (!confirmed) return
  if (editingId.value === folder.id) editingId.value = null
  await commit(
    settings.localFolders.filter(item => item.id !== folder.id),
    `已从列表删除 ${folder.label}`
  )
}
</script>
