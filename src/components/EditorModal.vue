<template>
  <div
    v-if="open"
    data-snail-editor
    class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-6 transition-opacity"
  >
    <div
      class="w-full max-w-6xl h-[86vh] bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl flex flex-col overflow-hidden"
      @click.stop
    >
      <div class="h-12 border-b border-surface-border px-4 flex items-center justify-between bg-surface-card/50 gap-3">
        <div class="flex items-center gap-3 min-w-0">
          <FileCode class="w-4 h-4 text-brand-400 shrink-0" />
          <div class="text-xs font-bold text-white font-mono flex items-center gap-2 min-w-0">
            <span class="truncate" :title="filePath">{{ filePath }}</span>
            <span class="text-[10px] px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 font-normal shrink-0">远程实时编辑</span>
            <span
              v-if="languageLabel"
              class="text-[10px] px-1.5 py-0.2 rounded bg-brand-500/15 text-brand-300 font-normal shrink-0"
            >{{ languageLabel }}</span>
          </div>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <button
            @click="handleSave"
            :disabled="saving || loading || !!loadError"
            class="flex items-center gap-1.5 px-3 py-1 rounded bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald disabled:opacity-50"
            title="写入远程服务器，编辑器保持打开"
          >
            <Save class="w-3.5 h-3.5" />
            <span>{{ saving ? '同步中...' : '保存到远程' }}</span>
          </button>
          <button @click="handleClose" class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white" title="关闭编辑器">
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <div class="flex-1 relative overflow-hidden bg-[#0a0d14]">
        <div ref="hostRef" class="absolute inset-0"></div>
        <div
          v-if="loading"
          class="absolute inset-0 z-10 flex items-center justify-center text-xs font-mono text-slate-400 bg-[#0a0d14]"
        >
          正在读取远程文件…
        </div>
        <div
          v-else-if="loadError"
          class="absolute inset-0 z-10 flex items-center justify-center px-8 text-center text-xs font-mono text-rose-300 bg-[#0a0d14]"
        >
          {{ loadError }}
        </div>
      </div>

      <div class="h-7 bg-surface-card border-t border-surface-border px-4 flex items-center justify-between text-[11px] font-mono text-surface-subtle gap-3">
        <span class="flex items-center gap-2 min-w-0 truncate">
          <span>UTF-8</span>
          <span>换行: {{ lineEndingLabel }}</span>
          <span>Ln {{ cursor.line }}, Col {{ cursor.col }}</span>
          <span v-if="dirty" class="text-amber-400">● 未保存</span>
        </span>
        <span class="shrink-0">Ctrl+S 保存 · Ctrl+F 查找 · 行号旁箭头折叠</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, nextTick } from 'vue'
import { FileCode, Save, X } from 'lucide-vue-next'
import { sftpReadFileApi, sftpWriteFileApi } from '@/api/tauri'
import { confirmDialog, dialogState } from '@/composables/useDialog'
import type { EditorCursor, RemoteEditorHandle } from '@/components/editor/createRemoteEditor'

const props = defineProps<{
  open: boolean
  filePath: string
  sessionId?: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const saving = ref(false)
const loading = ref(false)
const loadError = ref('')
const hostRef = ref<HTMLElement | null>(null)
const docText = ref('')
const baseline = ref('')
const languageLabel = ref('')
const lineEndingLabel = ref('LF')
const cursor = ref<EditorCursor>({ line: 1, col: 1 })

let loadSeq = 0
let editor: RemoteEditorHandle | null = null

const dirty = computed(() => docText.value !== baseline.value)

const destroyEditor = () => {
  editor?.destroy()
  editor = null
  docText.value = ''
  baseline.value = ''
}

const loadFile = async () => {
  const seq = ++loadSeq
  destroyEditor()
  loading.value = true
  loadError.value = ''
  languageLabel.value = ''
  cursor.value = { line: 1, col: 1 }

  if (!props.sessionId) {
    loading.value = false
    loadError.value = '尚未建立 SSH 会话，无法读取远程文件'
    emit('toast', loadError.value, 'warning')
    return
  }

  try {
    const text = await sftpReadFileApi(props.sessionId, props.filePath)
    if (seq !== loadSeq || !props.open) return
    await nextTick()
    if (!hostRef.value || seq !== loadSeq) return
    const { createRemoteEditor } = await import('@/components/editor/createRemoteEditor')
    if (seq !== loadSeq || !props.open || !hostRef.value) return
    editor = createRemoteEditor({
      parent: hostRef.value,
      path: props.filePath,
      text,
      onChange: (value, nextCursor) => {
        docText.value = value
        cursor.value = nextCursor
      },
    })
    baseline.value = editor.getValue()
    docText.value = baseline.value
    cursor.value = editor.getCursor()
    languageLabel.value = editor.languageLabel
    lineEndingLabel.value = editor.lineEndingLabel
    loading.value = false
  } catch (err: any) {
    if (seq !== loadSeq) return
    destroyEditor()
    loading.value = false
    loadError.value = `读取文件失败: ${err?.message || err}`
    emit('toast', loadError.value, 'error')
  }
}

watch(() => [props.open, props.filePath, props.sessionId] as const, ([open]) => {
  if (!open) {
    loadSeq++
    destroyEditor()
    loading.value = false
    loadError.value = ''
    return
  }
  void loadFile()
})

const handleClose = async () => {
  if (saving.value) {
    emit('toast', '正在保存，请稍候再关闭', 'info')
    return
  }
  if (dirty.value) {
    const confirmed = await confirmDialog({
      title: '放弃未保存的修改？',
      variant: 'warning',
      message: `${props.filePath} 有尚未保存到远端的修改，关闭后将丢失。`,
      confirmText: '放弃修改',
      cancelText: '继续编辑'
    })
    if (!confirmed) return
  }
  emit('close')
}

const handleSave = async () => {
  if (!props.sessionId) {
    emit('toast', '尚未建立 SSH 会话，无法写入远程文件', 'warning')
    return
  }
  if (!editor || saving.value || loading.value || loadError.value) return
  const snapshot = editor.getValue()
  saving.value = true
  try {
    await sftpWriteFileApi(props.sessionId, props.filePath, snapshot)
    // 保存期间如果又改了内容，基线只记已写入的那一版，编辑器保持打开
    baseline.value = snapshot
    if (editor.getValue() === snapshot) docText.value = snapshot
    emit('toast', '文件已写入远程服务器', 'success')
  } catch (err: any) {
    emit('toast', `保存失败: ${err?.message || err}`, 'error')
  } finally {
    saving.value = false
  }
}

const onKeydown = (e: KeyboardEvent) => {
  if (!props.open || dialogState.open) return
  if (e.key === 'Escape') {
    if (e.defaultPrevented) return
    e.preventDefault()
    void handleClose()
    return
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault()
    void handleSave()
  }
}

window.addEventListener('keydown', onKeydown)
onBeforeUnmount(() => {
  loadSeq++
  destroyEditor()
  window.removeEventListener('keydown', onKeydown)
})
</script>
