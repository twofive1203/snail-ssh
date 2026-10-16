<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-6 transition-opacity"
  >
    <div
      class="w-full max-w-4xl h-[80vh] bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl flex flex-col overflow-hidden"
      @click.stop
    >
      <!-- 编辑器顶栏 -->
      <div class="h-12 border-b border-surface-border px-4 flex items-center justify-between bg-surface-card/50">
        <div class="flex items-center gap-3">
          <FileCode class="w-4 h-4 text-brand-400" />
          <div class="text-xs font-bold text-white font-mono flex items-center gap-2">
            <span>{{ filePath }}</span>
            <span class="text-[10px] px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 font-normal">远程实时编辑</span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <button
            @click="handleSave"
            :disabled="saving"
            class="flex items-center gap-1.5 px-3 py-1 rounded bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald disabled:opacity-50"
          >
            <Save class="w-3.5 h-3.5" />
            <span>{{ saving ? '同步中...' : '保存并远程同步' }}</span>
          </button>
          <button @click="$emit('close')" class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white">
            <X class="w-4 h-4" />
          </button>
        </div>
      </div>

      <!-- 编辑器文本区域 -->
      <div class="flex-1 flex overflow-hidden">
        <!-- 行号 -->
        <div class="w-12 py-3 bg-surface-base text-right pr-3 font-mono text-xs text-slate-600 select-none border-r border-surface-border leading-6 overflow-hidden">
          <div v-for="n in lineCount" :key="n">{{ n }}</div>
        </div>
        <!-- 代码正文 -->
        <textarea
          v-model="content"
          spellcheck="false"
          class="flex-1 p-3 bg-[#0a0d14] text-slate-200 font-mono text-xs leading-6 resize-none focus:outline-none custom-scrollbar"
        ></textarea>
      </div>

      <!-- 编辑器底部信息 -->
      <div class="h-7 bg-surface-card border-t border-surface-border px-4 flex items-center justify-between text-[11px] font-mono text-surface-subtle">
        <span>编码: UTF-8 | 换行: LF | 模式: Config / Code</span>
        <span>按 Ctrl+S 或点击右上角保存</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { FileCode, Save, X } from 'lucide-vue-next'
import { sftpReadFileApi, sftpWriteFileApi } from '@/api/tauri'

const props = defineProps<{
  open: boolean
  filePath: string
  sessionId?: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const content = ref('')
const saving = ref(false)

const lineCount = computed(() => {
  const count = content.value.split('\n').length
  return Math.max(count, 15)
})

watch(() => props.open, async (val) => {
  if (!val || !props.filePath) return
  if (!props.sessionId) {
    emit('toast', '尚未建立 SSH 会话，无法读取远程文件', 'warning')
    return
  }
  try {
    content.value = await sftpReadFileApi(props.sessionId, props.filePath)
  } catch (err: any) {
    emit('toast', `读取文件失败: ${err?.message || err}`, 'error')
  }
})

const handleSave = async () => {
  if (!props.sessionId) {
    emit('toast', '尚未建立 SSH 会话，无法写入远程文件', 'warning')
    return
  }
  saving.value = true
  try {
    await sftpWriteFileApi(props.sessionId, props.filePath, content.value)
    emit('toast', '文件已成功写入远程服务器！', 'success')
    emit('close')
  } catch (err: any) {
    emit('toast', `保存失败: ${err?.message || err}`, 'error')
  } finally {
    saving.value = false
  }
}
</script>
