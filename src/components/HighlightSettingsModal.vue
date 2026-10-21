<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-6"
    @click.self="emit('close')"
  >
    <div
      class="w-full max-w-4xl h-[86vh] bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl flex flex-col overflow-hidden"
      @click.stop
    >
      <!-- 顶栏 -->
      <div class="px-5 py-3 border-b border-surface-border flex items-center gap-2.5 bg-surface-card/30 shrink-0">
        <Highlighter class="w-4 h-4 text-amber-300" />
        <h2 class="text-sm font-bold text-white">终端语法与关键字高亮</h2>
        <span class="text-[10px] text-surface-subtle">对标 MobaXterm 实时流语法着色 · 改动即时生效并自动保存</span>
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
            <span>启用终端语法高亮</span>
          </label>
          <label
            class="flex items-center gap-2 rounded-lg border border-surface-border bg-surface-card/30 px-3 py-2 text-xs text-slate-200 cursor-pointer"
            title="在终端右侧显示错误行位置，会占用约 6px 宽度，可能遮挡最右一列"
          >
            <input v-model="highlightSettings.overviewRuler" type="checkbox" class="accent-emerald-500 w-3.5 h-3.5" />
            <span>右侧错误概览条（Error 行位置标记）</span>
          </label>
        </div>

        <!-- 场景规则模板 + 配色预设 -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3 rounded-xl border border-surface-border bg-surface-card/20 p-3">
          <!-- 场景规则模板 -->
          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <h3 class="text-[11px] font-semibold text-slate-300">场景规则集（Syntax Profile）</h3>
              <span class="text-[10px] text-surface-subtle">一键切换不同运维场景规则</span>
            </div>
            <div class="flex flex-wrap items-center gap-1.5">
              <button
                v-for="sceneKey in SCENE_KEYS"
                :key="sceneKey"
                @click="handleSceneSelect(sceneKey)"
                class="px-2.5 py-1 rounded-lg text-[11px] border transition-colors"
                :class="highlightSettings.scene === sceneKey
                  ? 'bg-brand-500/20 text-emerald-300 border-brand-500/50 font-medium'
                  : 'bg-surface-card/40 text-slate-300 border-surface-border hover:border-slate-500'"
              >
                {{ HIGHLIGHT_SCENES[sceneKey].label }}
              </button>
              <span
                v-if="highlightSettings.scene === 'custom'"
                class="px-2 py-1 rounded-lg text-[10px] border bg-surface-card/40 text-slate-400 border-dashed border-surface-border"
              >
                已自定义规则
              </span>
            </div>
            <p class="text-[10px] text-slate-500 leading-relaxed">
              {{ currentSceneDesc }}
            </p>
          </div>

          <!-- 配色预设 -->
          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <h3 class="text-[11px] font-semibold text-slate-300">配色主题预设</h3>
              <span class="text-[10px] text-surface-subtle">只换颜色，保留你的词表与正则</span>
            </div>
            <div class="flex flex-wrap items-center gap-1.5">
              <button
                v-for="key in PRESET_KEYS"
                :key="key"
                @click="applyHighlightPreset(key)"
                class="px-2.5 py-1 rounded-lg text-[11px] border transition-colors"
                :class="highlightSettings.preset === key
                  ? 'bg-brand-500/20 text-emerald-300 border-brand-500/50 font-medium'
                  : 'bg-surface-card/40 text-slate-300 border-surface-border hover:border-slate-500'"
              >
                {{ HIGHLIGHT_PRESETS[key].label }}
              </button>
              <span
                v-if="highlightSettings.preset === 'custom'"
                class="px-2 py-1 rounded-lg text-[10px] border bg-surface-card/40 text-slate-400 border-dashed border-surface-border"
              >
                自定义配色
              </span>
            </div>
            <p class="text-[10px] text-slate-500 leading-relaxed">
              默认采用纯前景色着色（清爽无色块），可对需要强提醒的规则单独勾选「底色」或「整行」。
            </p>
          </div>
        </div>

        <!-- 实时预览 -->
        <div class="space-y-1.5">
          <div class="flex items-center justify-between">
            <h3 class="text-[11px] font-semibold text-slate-300">实时终端预览</h3>
            <span class="text-[10px] text-surface-subtle">同行支持多语法元素叠加着色（整行底色不会吞掉同行 IP / 时间 / 关键字）</span>
          </div>
          <div class="rounded-lg border border-surface-border bg-[#0c0e14] px-3.5 py-2.5 font-mono text-[11px] leading-6 overflow-x-auto custom-scrollbar text-slate-300">
            <div
              v-for="(row, index) in previewRows"
              :key="index"
              class="px-1 rounded-sm whitespace-pre"
              :style="{ color: row.foreground, backgroundColor: row.background }"
            >
              <span
                v-for="(segment, segIndex) in row.segments"
                :key="segIndex"
                :class="segment.background ? 'px-0.5 rounded-[2px]' : ''"
                :style="{ color: segment.foreground, backgroundColor: segment.background }"
              >{{ segment.text }}</span>
            </div>
          </div>
        </div>

        <!-- 规则列表 -->
        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <h3 class="text-[11px] font-semibold text-slate-300">高亮规则明细 ({{ highlightSettings.rules.length }})</h3>
              <span class="text-[10px] text-surface-subtle">列表越靠前优先级越高 · 支持「关键词词表」与「正则表达式」两种模式</span>
            </div>
            <button
              @click="addRule"
              class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-brand-500/15 hover:bg-brand-500/25 border border-brand-500/40 text-[11px] text-emerald-300 transition-colors"
            >
              <Plus class="w-3.5 h-3.5" />
              新增规则
            </button>
          </div>

          <div
            v-for="(rule, index) in highlightSettings.rules"
            :key="rule.id"
            class="rounded-xl border border-surface-border bg-surface-card/30 p-2.5 space-y-2 transition-colors"
            :class="!rule.enabled ? 'opacity-60' : ''"
          >
            <!-- 第一行：启用、预览色标、名称、分类、模式切换、排序与删除 -->
            <div class="flex items-center gap-2 flex-wrap">
              <input
                v-model="rule.enabled"
                type="checkbox"
                class="accent-emerald-500 w-3.5 h-3.5 cursor-pointer"
                @change="markSceneCustom()"
              />
              <span
                class="text-[10px] font-mono font-semibold px-1.5 py-0.5 rounded border select-none"
                :style="{
                  color: rule.foregroundColor,
                  backgroundColor: (rule.backgroundEnabled || rule.wholeLine) ? rule.backgroundColor : 'transparent',
                  borderColor: rule.foregroundColor + '55'
                }"
              >
                {{ levelLabel(rule.level) }}
              </span>
              <input
                v-model="rule.name"
                placeholder="规则名称"
                class="bg-transparent border-b border-transparent hover:border-surface-border focus:border-brand-500 text-xs text-white focus:outline-none px-1 py-0.5 w-32"
                @input="markSceneCustom()"
              />
              <select
                v-model="rule.level"
                class="bg-surface-base border border-surface-border rounded px-1.5 py-0.5 text-[10px] text-slate-200 focus:outline-none focus:border-brand-500"
                title="规则语义分类"
                @change="onLevelChange(rule)"
              >
                <option v-for="level in HIGHLIGHT_LEVELS" :key="level.key" :value="level.key">{{ level.label }}</option>
              </select>

              <!-- 匹配模式切换：关键词词表 vs 正则表达式 -->
              <div class="inline-flex rounded border border-surface-border bg-surface-base p-0.5 text-[10px]">
                <button
                  type="button"
                  @click="setMatchMode(rule, 'keywords')"
                  class="px-1.5 py-0.5 rounded-sm transition-colors"
                  :class="rule.matchMode === 'keywords' ? 'bg-brand-500/20 text-emerald-300 font-medium' : 'text-surface-subtle hover:text-slate-200'"
                  title="关键词模式：用逗号或空格分隔多个词，自动按词边界匹配"
                >
                  关键词
                </button>
                <button
                  type="button"
                  @click="setMatchMode(rule, 'regex')"
                  class="px-1.5 py-0.5 rounded-sm font-mono transition-colors"
                  :class="rule.matchMode === 'regex' ? 'bg-brand-500/20 text-emerald-300 font-medium' : 'text-surface-subtle hover:text-slate-200'"
                  title="正则表达式模式（专家模式）：直接编写原生正则"
                >
                  /.*/
                </button>
              </div>

              <span class="flex-1"></span>

              <!-- 样式控制组 -->
              <button
                @click="toggleCase(rule)"
                title="区分大小写"
                class="px-1.5 py-0.5 rounded text-[10px] font-mono border transition-colors"
                :class="rule.caseSensitive ? 'bg-brand-500/20 text-emerald-300 border-brand-500/40' : 'text-surface-subtle border-surface-border hover:text-white'"
              >Aa</button>

              <label class="flex items-center gap-1 text-[10px] text-slate-300 cursor-pointer" title="文字前景色">
                <span>字色</span>
                <input
                  v-model="rule.foregroundColor"
                  type="color"
                  class="w-5 h-5 bg-transparent border border-surface-border rounded cursor-pointer"
                  @input="markPresetCustom()"
                />
              </label>

              <div class="flex items-center gap-1 text-[10px] text-slate-300" title="勾选后启用背景色块；不勾选则只改变文字颜色">
                <label class="flex items-center gap-1 cursor-pointer select-none">
                  <input
                    v-model="rule.backgroundEnabled"
                    type="checkbox"
                    class="accent-emerald-500 w-3 h-3 cursor-pointer"
                    @change="markSceneCustom()"
                  />
                  <span>底色</span>
                </label>
                <input
                  v-model="rule.backgroundColor"
                  type="color"
                  :disabled="!rule.backgroundEnabled && !rule.wholeLine"
                  class="w-5 h-5 bg-transparent border border-surface-border rounded cursor-pointer disabled:opacity-35"
                  @input="markPresetCustom()"
                />
              </div>

              <button
                @click="toggleWholeLine(rule)"
                title="整行铺底着色（同行内其它词级高亮仍会叠加显示）"
                class="px-1.5 py-0.5 rounded text-[10px] border transition-colors"
                :class="rule.wholeLine ? 'bg-brand-500/20 text-emerald-300 border-brand-500/40' : 'text-surface-subtle border-surface-border hover:text-white'"
              >整行</button>

              <!-- 优先级上下移动与删除 -->
              <div class="flex items-center gap-0.5 ml-1">
                <button
                  @click="moveRule(index, -1)"
                  :disabled="index === 0"
                  class="p-1 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                  title="上移（提高优先级）"
                >
                  <ArrowUp class="w-3.5 h-3.5" />
                </button>
                <button
                  @click="moveRule(index, 1)"
                  :disabled="index === highlightSettings.rules.length - 1"
                  class="p-1 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
                  title="下移（降低优先级）"
                >
                  <ArrowDown class="w-3.5 h-3.5" />
                </button>
                <button
                  @click="removeRule(index)"
                  class="p-1 rounded hover:bg-surface-elevated text-surface-subtle hover:text-rose-400 transition-colors"
                  title="删除规则"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <!-- 第二行：词表或正则输入框 -->
            <div class="flex items-center gap-2">
              <input
                v-model="rule.pattern"
                spellcheck="false"
                :placeholder="rule.matchMode === 'keywords'
                  ? '输入要高亮的关键词，用逗号分隔（例如：ERROR, FATAL, DENIED, TIMED OUT）'
                  : '输入正则表达式源码（例如：\\b\\d{1,3}(?:\\.\\d{1,3}){3}\\b）'"
                class="flex-1 min-w-0 bg-surface-base border rounded px-2.5 py-1 text-[11px] font-mono text-slate-200 placeholder:text-slate-600 focus:outline-none focus:border-brand-500 transition-colors"
                :class="patternError(rule) ? 'border-rose-500' : 'border-surface-border'"
                @input="markSceneCustom()"
              />
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
          恢复默认 (MobaXterm 全量)
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
import { ArrowDown, ArrowUp, Download, Highlighter, Plus, RotateCcw, Trash2, Upload, X } from 'lucide-vue-next'
import { confirmDialog, dialogState } from '@/composables/useDialog'
import { findHighlightHits, type TerminalHighlightRule } from '@/composables/useTerminalHighlight'
import {
  applyHighlightPreset,
  applyHighlightScene,
  compileHighlightRules,
  createCustomRule,
  HIGHLIGHT_LEVELS,
  HIGHLIGHT_PRESETS,
  HIGHLIGHT_SCENES,
  highlightSettings,
  markPresetCustom,
  markSceneCustom,
  PRESET_KEYS,
  resetHighlightSettings,
   sanitizeSettings,
  SCENE_KEYS,
  validateRulePattern,
  type HighlightLevel,
  type HighlightMatchMode,
  type HighlightRuleSetting,
  type HighlightSceneKey
} from '@/composables/useHighlightSettings'

const props = defineProps<{ open: boolean }>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const fileInput = ref<HTMLInputElement | null>(null)

const levelLabel = (level: HighlightLevel): string =>
  HIGHLIGHT_LEVELS.find((item) => item.key === level)?.label ?? level

const currentSceneDesc = computed(() => {
  if (highlightSettings.scene === 'custom') {
    return '当前规则已在预设基础上手动调整；点击上方任意场景可切换为对应模板（保留你的非空自定义规则）。'
  }
  return HIGHLIGHT_SCENES[highlightSettings.scene]?.desc ?? ''
})

const patternError = (rule: HighlightRuleSetting): string => validateRulePattern(rule)

const handleSceneSelect = (sceneKey: HighlightSceneKey) => {
  applyHighlightScene(sceneKey)
  emit('toast', `已切换至「${HIGHLIGHT_SCENES[sceneKey].label}」规则集`, 'info')
}

const setMatchMode = (rule: HighlightRuleSetting, mode: HighlightMatchMode) => {
  if (rule.matchMode === mode) return
  rule.matchMode = mode
  markSceneCustom()
}

const onLevelChange = (rule: HighlightRuleSetting) => {
  const presetKey = highlightSettings.preset !== 'custom' ? highlightSettings.preset : 'default'
  const palette = HIGHLIGHT_PRESETS[presetKey].colors[rule.level]
  if (palette) {
    rule.foregroundColor = palette.fg
    rule.backgroundColor = palette.bg
  }
  markSceneCustom()
}

const toggleCase = (rule: HighlightRuleSetting) => {
  rule.caseSensitive = !rule.caseSensitive
  markSceneCustom()
}

const toggleWholeLine = (rule: HighlightRuleSetting) => {
  rule.wholeLine = !rule.wholeLine
  if (rule.wholeLine) rule.backgroundEnabled = true
  markSceneCustom()
}

const addRule = () => {
  highlightSettings.rules.unshift(createCustomRule())
  markSceneCustom()
}

const removeRule = (index: number) => {
  highlightSettings.rules.splice(index, 1)
  markSceneCustom()
}

const moveRule = (index: number, delta: -1 | 1) => {
  const target = index + delta
  if (target < 0 || target >= highlightSettings.rules.length) return
  const [item] = highlightSettings.rules.splice(index, 1)
  highlightSettings.rules.splice(target, 0, item)
  markSceneCustom()
}

// ---------- 实时预览（多语法元素同行叠加） ----------

const PREVIEW_LINES = [
  '2026-10-21 15:01:25 INFO  [sshd] Accepted publickey for root from 192.168.1.108:52412 ssh2',
  '2026-10-21 15:01:26 WARN  [nginx] upstream 10.0.0.45:8080 response slow (1.42s), retry=true, usage=86.5%',
  '2026-10-21 15:01:28 ERROR [mysql] dial tcp 127.0.0.1:3306: connection refused (timeout after 3000ms)',
  'eth0: <BROADCAST,UP,RUNNING> mtu 1500 ether 00:16:3e:2b:8c:91 RX 15.4GiB (0 dropped) status: active',
  '# /etc/ssh/sshd_config: PermitRootLogin yes | PasswordAuthentication no | Done in 24ms'
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
    if (!highlightSettings.enabled) {
      return { segments: [{ text: line }] }
    }
    const allHits = findHighlightHits(line, compiledRules.value)
    const wholeLineHit = allHits.find((hit) => hit.wholeLine)
    const wordHits = allHits.filter((hit) => !hit.wholeLine)

    const rowBg = wholeLineHit?.rule.backgroundColor
    const rowFg = undefined

    if (wordHits.length === 0) {
      return {
        segments: [{ text: line }],
        foreground: wholeLineHit?.rule.foregroundColor,
        background: rowBg
      }
    }

    const segments: PreviewSegment[] = []
    let cursor = 0
    for (const hit of wordHits) {
      if (hit.start > cursor) {
        segments.push({ text: line.slice(cursor, hit.start) })
      }
      segments.push({
        text: line.slice(hit.start, hit.end),
        foreground: hit.rule.foregroundColor,
        background: hit.rule.backgroundColor
      })
      cursor = hit.end
    }
    if (cursor < line.length) {
      segments.push({ text: line.slice(cursor) })
    }
    return {
      segments,
      foreground: rowFg,
      background: rowBg
    }
  })
)

// ---------- 导入 / 导出 / 恢复默认 ----------

const handleExport = () => {
  const payload = {
    schemaVersion: highlightSettings.schemaVersion,
    enabled: highlightSettings.enabled,
    scene: highlightSettings.scene,
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
    highlightSettings.schemaVersion = next.schemaVersion
    highlightSettings.enabled = next.enabled
    highlightSettings.scene = next.scene
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
    message: '将清除当前所有自定义规则与配色，恢复为内置「MobaXterm 经典全量」默认配置。',
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
