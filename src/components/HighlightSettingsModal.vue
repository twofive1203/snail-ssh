<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-6"
    @click.self="emit('close')"
  >
    <div
      class="w-full max-w-3xl h-[82vh] bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl flex flex-col overflow-hidden"
      @click.stop
    >
      <!-- 顶栏 -->
      <div class="px-5 py-3 border-b border-surface-border flex items-center gap-2.5 bg-surface-card/30 shrink-0">
        <Highlighter class="w-4 h-4 text-amber-300" />
        <h2 class="text-sm font-bold text-white">关键字高亮设置</h2>
        <span class="text-[10px] text-surface-subtle">改动即时生效并自动保存</span>
        <span class="flex-1"></span>
        <button @click="emit('close')" class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white" title="关闭">
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- 内容 -->
      <div class="flex-1 overflow-y-auto custom-scrollbar px-5 py-4 space-y-4">
        <!-- 全局开关 -->
        <div class="grid grid-cols-2 gap-3">
          <label class="flex items-center gap-2 rounded-lg border border-surface-border bg-surface-card/30 px-3 py-2 text-xs text-slate-200 cursor-pointer">
            <input v-model="highlightSettings.enabled" type="checkbox" class="accent-emerald-500 w-3.5 h-3.5" />
            启用关键字高亮
          </label>
          <label
            class="flex items-center gap-2 rounded-lg border border-surface-border bg-surface-card/30 px-3 py-2 text-xs text-slate-200 cursor-pointer"
            title="在终端右侧显示错误行位置，会占用约 6px 宽度，可能遮挡最右一列"
          >
            <input v-model="highlightSettings.overviewRuler" type="checkbox" class="accent-emerald-500 w-3.5 h-3.5" />
            右侧错误概览条
          </label>
        </div>

        <!-- 配色预设 -->
        <div class="space-y-2">
          <div class="flex items-center gap-2">
            <h3 class="text-[11px] font-semibold text-slate-300">配色预设</h3>
            <span class="text-[10px] text-surface-subtle">只换颜色，保留你改过的正则</span>
          </div>
          <div class="flex flex-wrap items-center gap-1.5">
            <button
              v-for="key in PRESET_KEYS"
              :key="key"
              @click="applyHighlightPreset(key)"
              class="px-2.5 py-1 rounded-lg text-[11px] border transition-colors"
              :class="highlightSettings.preset === key
                ? 'bg-brand-500/20 text-emerald-300 border-brand-500/50'
                : 'bg-surface-card/40 text-slate-300 border-surface-border hover:border-slate-500'"
            >
              {{ HIGHLIGHT_PRESETS[key].label }}
            </button>
            <span
              v-if="highlightSettings.preset === 'custom'"
              class="px-2.5 py-1 rounded-lg text-[11px] border bg-surface-card/40 text-slate-400 border-dashed border-surface-border"
            >
              自定义
            </span>
          </div>
        </div>

        <!-- 实时预览 -->
        <div class="space-y-1.5">
          <h3 class="text-[11px] font-semibold text-slate-300">实时预览</h3>
          <div class="rounded-lg border border-surface-border bg-[#0c0e14] p-3 font-mono text-[11px] leading-6 overflow-x-auto custom-scrollbar">
            <div
              v-for="(row, index) in previewRows"
              :key="index"
              :style="{ color: row.foreground, backgroundColor: row.background }"
            >
              <span
                v-for="(segment, segIndex) in row.segments"
                :key="segIndex"
                :style="{ color: segment.foreground, backgroundColor: segment.background }"
              >{{ segment.text }}</span>
            </div>
          </div>
        </div>

        <!-- 规则列表 -->
        <div class="space-y-2">
          <div class="flex items-center gap-2">
            <h3 class="text-[11px] font-semibold text-slate-300">高亮规则</h3>
            <span class="text-[10px] text-surface-subtle">按列表顺序决定优先级</span>
          </div>

          <div
            v-for="(rule, index) in highlightSettings.rules"
            :key="rule.id"
            class="rounded-xl border border-surface-border bg-surface-card/30 p-2.5 space-y-2"
          >
            <div class="flex items-center gap-2">
              <input v-model="rule.enabled" type="checkbox" class="accent-emerald-500 w-3.5 h-3.5" @change="markPresetCustom()" />
              <span
                class="text-[10px] font-semibold px-1.5 py-0.5 rounded border"
                :style="{ color: rule.foregroundColor, backgroundColor: rule.backgroundColor, borderColor: rule.foregroundColor + '55' }"
              >
                {{ levelLabel(rule.level) }}
              </span>
              <input
                v-model="rule.name"
                class="bg-transparent border-b border-transparent hover:border-surface-border focus:border-brand-500 text-xs text-white focus:outline-none px-1 py-0.5 w-28"
                @input="markPresetCustom()"
              />
              <span class="flex-1"></span>
              <button
                @click="removeRule(index)"
                class="p-1 rounded hover:bg-surface-elevated text-surface-subtle hover:text-rose-400 transition-colors"
                title="删除规则"
              >
                <Trash2 class="w-3.5 h-3.5" />
              </button>
            </div>

            <div class="flex items-center gap-2">
              <input
                v-model="rule.pattern"
                spellcheck="false"
                placeholder="正则表达式，例如 \bERROR\b"
                class="flex-1 min-w-0 bg-surface-base border rounded px-2 py-1 text-[11px] font-mono text-slate-200 focus:outline-none focus:border-brand-500 transition-colors"
                :class="patternError(rule) ? 'border-rose-500' : 'border-surface-border'"
                @input="markPresetCustom()"
              />
              <button
                @click="toggleCase(rule)"
                title="区分大小写"
                class="px-1.5 py-1 rounded text-[10px] font-mono border transition-colors"
                :class="rule.caseSensitive ? 'bg-brand-500/20 text-emerald-300 border-brand-500/40' : 'text-surface-subtle border-surface-border hover:text-white'"
              >Aa</button>
              <button
                @click="toggleWholeLine(rule)"
                title="整行着色（不改动已有前景色）"
                class="px-1.5 py-1 rounded text-[10px] border transition-colors"
                :class="rule.wholeLine ? 'bg-brand-500/20 text-emerald-300 border-brand-500/40' : 'text-surface-subtle border-surface-border hover:text-white'"
              >整行</button>
              <label class="flex items-center gap-1 text-[10px] text-surface-subtle">
                字
                <input v-model="rule.foregroundColor" type="color" class="w-5 h-5 bg-transparent border border-surface-border rounded cursor-pointer" @input="markPresetCustom()" />
              </label>
              <label class="flex items-center gap-1 text-[10px] text-surface-subtle">
                底
                <input v-model="rule.backgroundColor" type="color" class="w-5 h-5 bg-transparent border border-surface-border rounded cursor-pointer" @input="markPresetCustom()" />
              </label>
              <select
                v-model="rule.level"
                class="bg-surface-base border border-surface-border rounded px-1 py-1 text-[10px] text-slate-200 focus:outline-none focus:border-brand-500"
                @change="markPresetCustom()"
              >
                <option v-for="level in HIGHLIGHT_LEVELS" :key="level.key" :value="level.key">{{ level.label }}</option>
              </select>
            </div>

            <p v-if="patternError(rule)" class="text-[10px] text-rose-400">{{ patternError(rule) }}</p>
          </div>
        </div>
      </div>

      <!-- 底栏 -->
      <div class="px-5 py-3 border-t border-surface-border bg-surface-card/20 flex items-center gap-2 shrink-0">
        <button
          @click="addRule"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-200 transition-colors"
        >
          <Plus class="w-3.5 h-3.5" />
          新增规则
        </button>
        <input ref="fileInput" type="file" accept="application/json,.json" class="hidden" @change="handleImport" />
        <button
          @click="fileInput?.click()"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-200 transition-colors"
        >
          <Upload class="w-3.5 h-3.5" />
          导入
        </button>
        <button
          @click="handleExport"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-xs text-slate-200 transition-colors"
        >
          <Download class="w-3.5 h-3.5" />
          导出
        </button>
        <span class="flex-1"></span>
        <button
          @click="handleReset"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg hover:bg-surface-elevated border border-transparent text-xs text-surface-subtle hover:text-amber-300 transition-colors"
        >
          <RotateCcw class="w-3.5 h-3.5" />
          恢复默认
        </button>
        <button
          @click="emit('close')"
          class="px-4 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-xs transition-all shadow-glow-emerald"
        >
          完成
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Download, Highlighter, Plus, RotateCcw, Trash2, Upload, X } from 'lucide-vue-next'
import { confirmDialog, dialogState } from '@/composables/useDialog'
import { findHighlightHits, type TerminalHighlightRule } from '@/composables/useTerminalHighlight'
import {
  applyHighlightPreset,
  compileHighlightRules,
  createCustomRule,
  HIGHLIGHT_LEVELS,
  HIGHLIGHT_PRESETS,
  highlightSettings,
  markPresetCustom,
  PRESET_KEYS,
  resetHighlightSettings,
  sanitizeSettings,
  type HighlightLevel,
  type HighlightRuleSetting
} from '@/composables/useHighlightSettings'

const props = defineProps<{ open: boolean }>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const fileInput = ref<HTMLInputElement | null>(null)

const levelLabel = (level: HighlightLevel): string =>
  HIGHLIGHT_LEVELS.find((item) => item.key === level)?.label ?? level

const patternError = (rule: HighlightRuleSetting): string => {
  const source = rule.pattern.trim()
  if (!source) return ''
  try {
    new RegExp(source, rule.caseSensitive ? '' : 'i')
    return ''
  } catch (error: any) {
    return `正则无效：${error?.message || error}`
  }
}

const toggleCase = (rule: HighlightRuleSetting) => {
  rule.caseSensitive = !rule.caseSensitive
  markPresetCustom()
}

const toggleWholeLine = (rule: HighlightRuleSetting) => {
  rule.wholeLine = !rule.wholeLine
  markPresetCustom()
}

const addRule = () => {
  highlightSettings.rules.push(createCustomRule())
  markPresetCustom()
}

const removeRule = (index: number) => {
  highlightSettings.rules.splice(index, 1)
}

// ---------- 实时预览 ----------

const PREVIEW_LINES = [
  '2026-01-01 12:00:00 INFO  server listening on 0.0.0.0:8080',
  '2026-01-01 12:00:01 WARN  config key "timeout" is deprecated',
  '2026-01-01 12:00:02 ERROR dial tcp 127.0.0.1:3306: connection refused',
  '2026-01-01 12:00:03 Done in 1.24s',
  '需要高亮的自定义关键字示例'
]

interface PreviewSegment {
  text: string
  foreground?: string
  background?: string
}

interface PreviewRow {
  segments: PreviewSegment[]
  foreground?: string
  background?: string
}

const compiledRules = computed<TerminalHighlightRule[]>(() => compileHighlightRules(highlightSettings))

const previewRows = computed<PreviewRow[]>(() =>
  PREVIEW_LINES.map((line) => {
    const hits = findHighlightHits(line, compiledRules.value)
    // 整行规则在预览里也铺满整行，和终端里的表现保持一致
    const wholeLineHit = hits.find((hit) => hit.rule.wholeLine)
    if (wholeLineHit) {
      return {
        segments: [{ text: line }],
        foreground: wholeLineHit.rule.foregroundColor,
        background: wholeLineHit.rule.backgroundColor
      }
    }
    if (hits.length === 0) return { segments: [{ text: line }] }

    const segments: PreviewSegment[] = []
    let cursor = 0
    for (const hit of hits) {
      if (hit.start > cursor) segments.push({ text: line.slice(cursor, hit.start) })
      segments.push({
        text: line.slice(hit.start, hit.end),
        foreground: hit.rule.foregroundColor,
        background: hit.rule.backgroundColor
      })
      cursor = hit.end
    }
    if (cursor < line.length) segments.push({ text: line.slice(cursor) })
    return { segments }
  })
)

// ---------- 导入 / 导出 / 恢复默认 ----------

const handleExport = () => {
  const payload = {
    enabled: highlightSettings.enabled,
    preset: highlightSettings.preset,
    overviewRuler: highlightSettings.overviewRuler,
    rules: highlightSettings.rules
  }
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `snail-ssh-highlight-${new Date().toISOString().slice(0, 10)}.json`
  anchor.click()
  URL.revokeObjectURL(url)
  emit('toast', '高亮规则已导出', 'success')
}

const handleImport = async (event: Event) => {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  try {
    const text = await file.text()
    const next = sanitizeSettings(JSON.parse(text))
    highlightSettings.enabled = next.enabled
    highlightSettings.preset = next.preset
    highlightSettings.overviewRuler = next.overviewRuler
    highlightSettings.rules.splice(0, highlightSettings.rules.length, ...next.rules)
    emit('toast', `已导入 ${next.rules.length} 条高亮规则`, 'success')
  } catch (error: any) {
    emit('toast', `导入失败：${error?.message || error}`, 'error')
  }
}

const handleReset = async () => {
  const confirmed = await confirmDialog({
    title: '恢复默认高亮规则',
    variant: 'warning',
    message: '将清除当前所有自定义规则与配色，恢复为内置默认配置。',
    confirmText: '恢复默认'
  })
  if (!confirmed) return
  resetHighlightSettings()
  emit('toast', '已恢复默认高亮规则', 'success')
}

// ---------- 键盘 ----------

const onKeydown = (event: KeyboardEvent) => {
  if (!props.open) return
  if (event.key !== 'Escape') return
  if (dialogState.open) return
  event.preventDefault()
  emit('close')
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>
