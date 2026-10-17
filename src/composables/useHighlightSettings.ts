import { reactive, watch } from 'vue'
import type { TerminalHighlightRule } from '@/composables/useTerminalHighlight'

/**
 * 关键字高亮的用户配置。
 *
 * 设计约束：
 * - 规则以「字符串正则」持久化，运行前才编译成 RegExp；导入的 JSON 一律走 sanitizeSettings，
 *   避免非法正则或 #RRGGBB 之外的颜色让 xterm 直接抛错。
 * - 配置是全局单例（跨所有终端实例共享），TerminalView 只负责监听变化后应用到自己的会话。
 * - enabled 与旧版 `snail_ssh_highlight_enabled` 兼容：新配置不存在时用旧开关初始化。
 */

export type HighlightLevel = 'error' | 'warn' | 'info' | 'debug' | 'success' | 'custom'

export type HighlightPresetKey = 'default' | 'dracula' | 'nord' | 'one-dark' | 'high-contrast'

export interface HighlightRuleSetting {
  id: string
  level: HighlightLevel
  name: string
  /** 正则源码（不含首尾斜杠） */
  pattern: string
  caseSensitive: boolean
  wholeLine: boolean
  foregroundColor: string
  backgroundColor: string
  enabled: boolean
}

export interface HighlightSettings {
  enabled: boolean
  preset: HighlightPresetKey | 'custom'
  /** 右侧错误概览条（会占用终端右侧约 6px，可能遮挡最右一列，默认关闭） */
  overviewRuler: boolean
  rules: HighlightRuleSetting[]
}

interface PaletteColor {
  fg: string
  bg: string
}

export const HIGHLIGHT_LEVELS: Array<{ key: HighlightLevel; label: string }> = [
  { key: 'error', label: '错误' },
  { key: 'warn', label: '警告' },
  { key: 'info', label: '信息' },
  { key: 'debug', label: '调试' },
  { key: 'success', label: '成功' },
  { key: 'custom', label: '自定义' }
]

const LEVEL_KEYS = HIGHLIGHT_LEVELS.map((item) => item.key)

export const HIGHLIGHT_PRESETS: Record<HighlightPresetKey, { label: string; colors: Record<HighlightLevel, PaletteColor> }> = {
  default: {
    label: '默认',
    colors: {
      error: { fg: '#fecaca', bg: '#7f1d1d' },
      warn: { fg: '#fde68a', bg: '#451a03' },
      info: { fg: '#a5f3fc', bg: '#083344' },
      debug: { fg: '#cbd5e1', bg: '#1e293b' },
      success: { fg: '#bbf7d0', bg: '#052e16' },
      custom: { fg: '#e9d5ff', bg: '#3b0764' }
    }
  },
  dracula: {
    label: 'Dracula',
    colors: {
      error: { fg: '#ff5555', bg: '#3a1414' },
      warn: { fg: '#f1fa8c', bg: '#3a3410' },
      info: { fg: '#8be9fd', bg: '#0f3340' },
      debug: { fg: '#9aa0b5', bg: '#1e2029' },
      success: { fg: '#50fa7b', bg: '#0d3319' },
      custom: { fg: '#ff79c6', bg: '#3a1030' }
    }
  },
  nord: {
    label: 'Nord',
    colors: {
      error: { fg: '#bf616a', bg: '#331c1f' },
      warn: { fg: '#ebcb8b', bg: '#332b1a' },
      info: { fg: '#88c0d0', bg: '#1a2c33' },
      debug: { fg: '#8f97a8', bg: '#22272f' },
      success: { fg: '#a3be8c', bg: '#1e2a1a' },
      custom: { fg: '#b48ead', bg: '#2a2230' }
    }
  },
  'one-dark': {
    label: 'One Dark',
    colors: {
      error: { fg: '#e06c75', bg: '#33191c' },
      warn: { fg: '#e5c07b', bg: '#332a19' },
      info: { fg: '#56b6c2', bg: '#16282b' },
      debug: { fg: '#8b93a1', bg: '#21252b' },
      success: { fg: '#98c379', bg: '#1d2a19' },
      custom: { fg: '#c678dd', bg: '#2a1a30' }
    }
  },
  'high-contrast': {
    label: '高对比',
    colors: {
      error: { fg: '#ffffff', bg: '#b91c1c' },
      warn: { fg: '#111827', bg: '#fbbf24' },
      info: { fg: '#ffffff', bg: '#1d4ed8' },
      debug: { fg: '#e5e7eb', bg: '#374151' },
      success: { fg: '#052e16', bg: '#4ade80' },
      custom: { fg: '#111827', bg: '#c084fc' }
    }
  }
}

export const PRESET_KEYS = Object.keys(HIGHLIGHT_PRESETS) as HighlightPresetKey[]

const STORAGE_KEY = 'snail_ssh_highlight_settings_v1'
const LEGACY_ENABLED_KEY = 'snail_ssh_highlight_enabled'

let ruleSeq = 0
const nextRuleId = (): string => `rule-${Date.now().toString(36)}-${(ruleSeq++).toString(36)}`

const levelLabel = (level: HighlightLevel): string =>
  HIGHLIGHT_LEVELS.find((item) => item.key === level)?.label ?? '自定义'

const makeRule = (
  level: HighlightLevel,
  name: string,
  pattern: string,
  color: PaletteColor,
  overrides: Partial<HighlightRuleSetting> = {}
): HighlightRuleSetting => ({
  id: nextRuleId(),
  level,
  name,
  pattern,
  caseSensitive: false,
  wholeLine: false,
  foregroundColor: color.fg,
  backgroundColor: color.bg,
  enabled: true,
  ...overrides
})

/** 新建一条自定义规则（供设置面板「新增规则」使用） */
export function createCustomRule(): HighlightRuleSetting {
  return makeRule('custom', '新规则', '', HIGHLIGHT_PRESETS.default.colors.custom)
}

export function createDefaultSettings(): HighlightSettings {
  const colors = HIGHLIGHT_PRESETS.default.colors
  return {
    enabled: true,
    preset: 'default',
    overviewRuler: false,
    rules: [
      // 默认只开 error / warn / success，info / debug / 自定义保持关闭，避免一上来满屏着色
      makeRule('error', '错误', '\\b(ERROR|FATAL|FAIL(?:ED|URE)?|PANIC|Exception|Traceback)\\b', colors.error, { wholeLine: true }),
      makeRule('warn', '警告', '\\b(WARN(?:ING)?|DEPRECAT(?:ED|ION))\\b', colors.warn),
      makeRule('info', '信息', '\\b(INFO|NOTICE)\\b', colors.info, { enabled: false }),
      makeRule('debug', '调试', '\\b(DEBUG|TRACE)\\b', colors.debug, { enabled: false }),
      makeRule('success', '成功', '\\b(SUCCESS|PASSED|Done)\\b', colors.success),
      makeRule('custom', '自定义关键字', '', colors.custom, { enabled: false })
    ]
  }
}

/** 只接受 #RRGGBB / #RGB；xterm 的 decoration 只认 #RRGGBB */
const normalizeColor = (value: unknown): string | undefined => {
  if (typeof value !== 'string') return undefined
  const text = value.trim()
  if (/^#[0-9a-fA-F]{6}$/.test(text)) return text.toLowerCase()
  if (/^#[0-9a-fA-F]{3}$/.test(text)) {
    return `#${text[1]}${text[1]}${text[2]}${text[2]}${text[3]}${text[3]}`.toLowerCase()
  }
  return undefined
}

const isLevel = (value: unknown): value is HighlightLevel =>
  typeof value === 'string' && (LEVEL_KEYS as string[]).includes(value)

/** 把任意来源（localStorage / 导入 JSON）的配置收敛成安全可用的结构 */
export function sanitizeSettings(raw: unknown): HighlightSettings {
  const fallback = createDefaultSettings()
  if (!raw || typeof raw !== 'object') return fallback

  const input = raw as Record<string, unknown>
  const rules: HighlightRuleSetting[] = []
  if (Array.isArray(input.rules)) {
    for (const item of input.rules) {
      if (!item || typeof item !== 'object') continue
      const rule = item as Record<string, unknown>
      const level = isLevel(rule.level) ? rule.level : 'custom'
      const colors = HIGHLIGHT_PRESETS.default.colors[level]
      rules.push({
        id: typeof rule.id === 'string' && rule.id ? rule.id : nextRuleId(),
        level,
        name: typeof rule.name === 'string' && rule.name ? rule.name : levelLabel(level),
        pattern: typeof rule.pattern === 'string' ? rule.pattern : '',
        caseSensitive: rule.caseSensitive === true,
        wholeLine: rule.wholeLine === true,
        foregroundColor: normalizeColor(rule.foregroundColor) ?? colors.fg,
        backgroundColor: normalizeColor(rule.backgroundColor) ?? colors.bg,
        enabled: rule.enabled !== false
      })
    }
  }

  const preset = typeof input.preset === 'string' && (PRESET_KEYS as string[]).includes(input.preset)
    ? (input.preset as HighlightPresetKey)
    : 'custom'

  return {
    enabled: input.enabled !== false,
    preset,
    overviewRuler: input.overviewRuler === true,
    rules: rules.length > 0 ? rules : fallback.rules
  }
}

function loadSettings(): HighlightSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return sanitizeSettings(JSON.parse(raw))

    // 首次升级：沿用旧版开关状态
    const settings = createDefaultSettings()
    if (localStorage.getItem(LEGACY_ENABLED_KEY) === '0') settings.enabled = false
    return settings
  } catch {
    return createDefaultSettings()
  }
}

/** 全局单例配置：所有终端实例共享 */
export const highlightSettings = reactive<HighlightSettings>(loadSettings())

const persist = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      enabled: highlightSettings.enabled,
      preset: highlightSettings.preset,
      overviewRuler: highlightSettings.overviewRuler,
      rules: highlightSettings.rules
    }))
  } catch {
    // localStorage 不可用时仅本次会话生效
  }
}

// 模块级深监听：设置面板里任何字段改动都会自动落盘
watch(highlightSettings, persist, { deep: true })

/** 应用配色预设：只替换颜色，保留用户改过的正则与开关 */
export function applyHighlightPreset(key: HighlightPresetKey): void {
  const preset = HIGHLIGHT_PRESETS[key]
  for (const rule of highlightSettings.rules) {
    const color = preset.colors[rule.level]
    if (!color) continue
    rule.foregroundColor = color.fg
    rule.backgroundColor = color.bg
  }
  highlightSettings.preset = key
}

/** 用户手动改动规则后，预设不再代表当前配色 */
export function markPresetCustom(): void {
  if (highlightSettings.preset !== 'custom') highlightSettings.preset = 'custom'
}

export function resetHighlightSettings(): void {
  const defaults = createDefaultSettings()
  highlightSettings.enabled = defaults.enabled
  highlightSettings.preset = defaults.preset
  highlightSettings.overviewRuler = defaults.overviewRuler
  highlightSettings.rules.splice(0, highlightSettings.rules.length, ...defaults.rules)
}

/** 把配置编译成终端可直接消费的规则；非法正则和空正则在这里被过滤掉 */
export function compileHighlightRules(settings: HighlightSettings): TerminalHighlightRule[] {
  const rules: TerminalHighlightRule[] = []
  for (const rule of settings.rules) {
    if (!rule.enabled) continue
    const source = rule.pattern.trim()
    if (!source) continue

    let pattern: RegExp
    try {
      pattern = new RegExp(source, rule.caseSensitive ? '' : 'i')
    } catch {
      // 设置面板会提示非法正则，这里静默跳过，不影响其它规则
      continue
    }

    rules.push({
      key: rule.id,
      level: rule.level,
      pattern,
      foregroundColor: rule.foregroundColor,
      backgroundColor: rule.backgroundColor,
      wholeLine: rule.wholeLine
    })
  }
  return rules
}
