import { reactive, watch } from 'vue'
import type { TerminalHighlightRule } from '@/composables/useTerminalHighlight'

/**
 * 终端语法高亮（Syntax Highlighting）用户配置。
 *
 * 对标 MobaXterm 语法高亮机制：
 * - 支持「关键词词表（keywords）」与「正则表达式（regex）」双模式：
 *   关键词模式允许直接输入逗号/空格分隔的词，自动编译为词边界正则（兼顾英文词边界与中文短语）。
 * - 支持细粒度语法分类：错误、警告、成功、信息、调试、网络/地址、日期/时间、数值/单位、布尔/开关、路径/注释、自定义。
 * - 支持独立控制每个规则是否启用背景色（backgroundEnabled）：默认词级规则采用清爽的纯前景色着色，
 *   避免满屏深色背景块；需要强提醒的规则（如错误）可单独开启背景色或整行铺底。
 * - 内置多套场景规则模板（MobaXterm 经典全量 / 日志告警聚焦 / 网络与连接排障 / 配置与脚本），一键切换。
 */

export type HighlightLevel =
  | 'error'
  | 'warn'
  | 'success'
  | 'info'
  | 'debug'
  | 'network'
  | 'datetime'
  | 'number'
  | 'boolean'
  | 'path'
  | 'custom'

export type HighlightMatchMode = 'keywords' | 'regex'

export type HighlightPresetKey = 'default' | 'dracula' | 'nord' | 'one-dark' | 'high-contrast'

export type HighlightSceneKey = 'mobaxterm' | 'log-alert' | 'network-ops' | 'shell-config'

export interface HighlightRuleSetting {
  id: string
  level: HighlightLevel
  name: string
  /** 匹配模式：'keywords' 为逗号/空格分隔词表，'regex' 为原生正则表达式 */
  matchMode: HighlightMatchMode
  /** keywords 模式下为词表；regex 模式下为正则源码（不含首尾斜杠） */
  pattern: string
  caseSensitive: boolean
  wholeLine: boolean
  foregroundColor: string
  backgroundColor: string
  /** 是否渲染背景底色（wholeLine 开启时始终使用背景色） */
  backgroundEnabled: boolean
  enabled: boolean
}

export interface HighlightSettings {
  schemaVersion: number
  enabled: boolean
  scene: HighlightSceneKey | 'custom'
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
  { key: 'error', label: '错误/异常' },
  { key: 'warn', label: '警告/等待' },
  { key: 'success', label: '成功/就绪' },
  { key: 'info', label: '信息/状态' },
  { key: 'network', label: '网络/地址' },
  { key: 'datetime', label: '日期/时间' },
  { key: 'number', label: '数值/单位' },
  { key: 'boolean', label: '布尔/开关' },
  { key: 'path', label: '路径/注释' },
  { key: 'debug', label: '调试/追踪' },
  { key: 'custom', label: '自定义' }
]

const LEVEL_KEYS = HIGHLIGHT_LEVELS.map((item) => item.key)

export const HIGHLIGHT_SCENES: Record<HighlightSceneKey, { label: string; desc: string }> = {
  mobaxterm: {
    label: 'MobaXterm 经典全量',
    desc: '全要素纯字色高亮 + 错误词重点标红，覆盖状态词、IP/MAC、时间戳、数值单位、布尔值与注释'
  },
  'log-alert': {
    label: '日志告警聚焦',
    desc: '仅高亮 Error（整行铺底）、Warn、Success、Info，适合紧盯服务日志报错'
  },
  'network-ops': {
    label: '网络与系统排障',
    desc: '强化网卡/连接状态（UP/DOWN/ESTABLISHED/LISTEN）、IP/MAC/CIDR、丢包延迟与协议关键字'
  },
  'shell-config': {
    label: '配置与脚本审阅',
    desc: '突出配置文件注释、路径、布尔开关（yes/no/true/false）、容量数值与异常提示'
  }
}

export const SCENE_KEYS = Object.keys(HIGHLIGHT_SCENES) as HighlightSceneKey[]

export const HIGHLIGHT_PRESETS: Record<HighlightPresetKey, { label: string; colors: Record<HighlightLevel, PaletteColor> }> = {
  default: {
    label: 'Moba 暗夜',
    colors: {
      error: { fg: '#ff6b6b', bg: '#451218' },
      warn: { fg: '#fbbf24', bg: '#451a03' },
      success: { fg: '#4ade80', bg: '#052e16' },
      info: { fg: '#38bdf8', bg: '#083344' },
      network: { fg: '#22d3ee', bg: '#083344' },
      datetime: { fg: '#a78bfa', bg: '#2e1065' },
      number: { fg: '#fb923c', bg: '#431407' },
      boolean: { fg: '#f472b6', bg: '#500724' },
      path: { fg: '#818cf8', bg: '#1e1b4b' },
      debug: { fg: '#94a3b8', bg: '#1e293b' },
      custom: { fg: '#e879f9', bg: '#3b0764' }
    }
  },
  dracula: {
    label: 'Dracula',
    colors: {
      error: { fg: '#ff5555', bg: '#3a1414' },
      warn: { fg: '#f1fa8c', bg: '#3a3410' },
      success: { fg: '#50fa7b', bg: '#0d3319' },
      info: { fg: '#8be9fd', bg: '#0f3340' },
      network: { fg: '#8be9fd', bg: '#0f3340' },
      datetime: { fg: '#bd93f9', bg: '#241b35' },
      number: { fg: '#ffb86c', bg: '#382510' },
      boolean: { fg: '#ff79c6', bg: '#3a1030' },
      path: { fg: '#6272a4', bg: '#191a24' },
      debug: { fg: '#9aa0b5', bg: '#1e2029' },
      custom: { fg: '#ff79c6', bg: '#3a1030' }
    }
  },
  nord: {
    label: 'Nord',
    colors: {
      error: { fg: '#bf616a', bg: '#331c1f' },
      warn: { fg: '#ebcb8b', bg: '#332b1a' },
      success: { fg: '#a3be8c', bg: '#1e2a1a' },
      info: { fg: '#88c0d0', bg: '#1a2c33' },
      network: { fg: '#8fbcbb', bg: '#1a2c2b' },
      datetime: { fg: '#b48ead', bg: '#2a2230' },
      number: { fg: '#d08770', bg: '#33221c' },
      boolean: { fg: '#b48ead', bg: '#2a2230' },
      path: { fg: '#81a1c1', bg: '#1c2633' },
      debug: { fg: '#8f97a8', bg: '#22272f' },
      custom: { fg: '#b48ead', bg: '#2a2230' }
    }
  },
  'one-dark': {
    label: 'One Dark',
    colors: {
      error: { fg: '#e06c75', bg: '#33191c' },
      warn: { fg: '#e5c07b', bg: '#332a19' },
      success: { fg: '#98c379', bg: '#1d2a19' },
      info: { fg: '#61afef', bg: '#162635' },
      network: { fg: '#56b6c2', bg: '#16282b' },
      datetime: { fg: '#c678dd', bg: '#2a1a30' },
      number: { fg: '#d19a66', bg: '#302216' },
      boolean: { fg: '#e06c75', bg: '#33191c' },
      path: { fg: '#7f848e', bg: '#1e2127' },
      debug: { fg: '#8b93a1', bg: '#21252b' },
      custom: { fg: '#c678dd', bg: '#2a1a30' }
    }
  },
  'high-contrast': {
    label: '高对比',
    colors: {
      error: { fg: '#ffffff', bg: '#b91c1c' },
      warn: { fg: '#fde047', bg: '#713f12' },
      success: { fg: '#4ade80', bg: '#052e16' },
      info: { fg: '#38bdf8', bg: '#1e3a8a' },
      network: { fg: '#67e8f9', bg: '#164e63' },
      datetime: { fg: '#c4b5fd', bg: '#4c1d95' },
      number: { fg: '#fdba74', bg: '#7c2d12' },
      boolean: { fg: '#f9a8d4', bg: '#831843' },
      path: { fg: '#93c5fd', bg: '#1e3a8a' },
      debug: { fg: '#e5e7eb', bg: '#374151' },
      custom: { fg: '#f0abfc', bg: '#701a75' }
    }
  }
}

export const PRESET_KEYS = Object.keys(HIGHLIGHT_PRESETS) as HighlightPresetKey[]

const CURRENT_SCHEMA_VERSION = 2
const STORAGE_KEY = 'snail_ssh_highlight_settings_v1'
const LEGACY_ENABLED_KEY = 'snail_ssh_highlight_enabled'

let ruleSeq = 0
const nextRuleId = (): string => `rule-${Date.now().toString(36)}-${(ruleSeq++).toString(36)}`

const levelLabel = (level: HighlightLevel): string =>
  HIGHLIGHT_LEVELS.find((item) => item.key === level)?.label ?? '自定义'

const makeRule = (
  level: HighlightLevel,
  name: string,
  matchMode: HighlightMatchMode,
  pattern: string,
  color: PaletteColor,
  overrides: Partial<HighlightRuleSetting> = {}
): HighlightRuleSetting => ({
  id: nextRuleId(),
  level,
  name,
  matchMode,
  pattern,
  caseSensitive: false,
  wholeLine: false,
  foregroundColor: color.fg,
  backgroundColor: color.bg,
  backgroundEnabled: false,
  enabled: true,
  ...overrides
})

/** 新建一条自定义规则（默认使用易上手的关键词模式） */
export function createCustomRule(): HighlightRuleSetting {
  return makeRule('custom', '新规则', 'keywords', '', HIGHLIGHT_PRESETS.default.colors.custom)
}

/**
 * 构建指定场景与调色板下的内置规则列表
 */
export function createSceneRules(
  scene: HighlightSceneKey,
  presetKey: HighlightPresetKey = 'default'
): HighlightRuleSetting[] {
  const colors = HIGHLIGHT_PRESETS[presetKey]?.colors ?? HIGHLIGHT_PRESETS.default.colors

  const baseErrorKeywords =
    'ERROR, ERR, FATAL, FAIL, FAILED, FAILURE, PANIC, CRITICAL, CRIT, EMERGENCY, EMERG, ALERT, Exception, Traceback, DENIED, REFUSED, REJECTED, UNREACHABLE, TIMEOUT, TIMED OUT, DEAD, KILLED, CRASH, CRASHED, CORRUPT, CORRUPTED, INVALID, FORBIDDEN, UNAUTHORIZED, SEGFAULT, OOM, ABORTED, DISCONNECTED, OFFLINE, FAULT'
  const baseWarnKeywords =
    'WARN, WARNING, DEPRECATED, DEPRECATION, RETRY, RETRYING, PENDING, WAITING, SLOW, OVERFLOW, EXPIRED, SKIP, SKIPPED, CANCEL, CANCELLED, CANCELED, BLOCKED, LOCKED, BUSY, DEGRADED, UNSTABLE, DROPPED, LOST'
  const baseSuccessKeywords =
    'SUCCESS, SUCCESSFUL, SUCCEEDED, PASSED, PASS, OK, DONE, COMPLETE, COMPLETED, ACTIVE, RUNNING, ENABLED, CONNECTED, ONLINE, READY, LISTENING, ACCEPTED, ESTABLISHED, HEALTHY, REACHABLE, VALID, VERIFIED, SYNCHRONIZED'
  const baseInfoKeywords =
    'INFO, NOTICE, STARTING, STARTED, STOPPING, STOPPED, RELOADING, RELOADED, RESTARTING, RESTARTED, CREATING, CREATED, UPDATING, UPDATED, DELETING, DELETED, CONNECTING, CLOSING, CLOSED, INITIALIZING, LOADED, SAVED'

  const networkRegex =
    '\\b(?:(?:25[0-5]|2[0-4]\\d|1?\\d?\\d)(?:\\.(?:25[0-5]|2[0-4]\\d|1?\\d?\\d)){3}(?::\\d{1,5}|\\/\\d{1,2})?|(?:[0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}|(?:TCP|UDP|ICMP|HTTP\\/[12](?:\\.[01])?|HTTPS|SSH|TLSv1\\.[23]|SSL|DNS|ARP|VLAN|BGP|OSPF))\\b'
  const datetimeRegex =
    '\\b(?:\\d{4}[-/]\\d{2}[-/]\\d{2}(?:[T\\s]\\d{2}:\\d{2}:\\d{2}(?:\\.\\d{1,6})?(?:Z|[+-]\\d{2}:?\\d{2})?)?|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\\s+\\d{1,2}\\s+\\d{2}:\\d{2}:\\d{2}|\\d{2}:\\d{2}:\\d{2}(?:\\.\\d{1,6})?)\\b'
  const numberRegex =
    '\\b(?:0x[0-9a-fA-F]+|\\d+(?:\\.\\d+)?(?:%|(?:\\s*(?:[KMGTPE]i?B?(?:\\/s)?|bps|[KMGT]bps|ms|us|ns|s|m|h|d))\\b)?)\\b'
  const booleanKeywords = 'true, false, yes, no, on, off, null, none, nil, NaN, undefined'
  const pathCommentRegex =
    '(?:(?:^|(?<=\\s))#[^\\r\\n]*|(?:~|(?<![A-Za-z0-9_.:\\/])\\/[A-Za-z0-9_.-]+)(?:\\/[A-Za-z0-9_.:@-]+)+)'
  const debugKeywords = 'DEBUG, TRACE, VERBOSE, DBG'

  if (scene === 'log-alert') {
    return [
      makeRule('error', '错误告警', 'keywords', baseErrorKeywords, colors.error, {
        wholeLine: true,
        backgroundEnabled: true
      }),
      makeRule('warn', '警告提示', 'keywords', baseWarnKeywords, colors.warn, {
        backgroundEnabled: true
      }),
      makeRule('success', '成功状态', 'keywords', baseSuccessKeywords, colors.success),
      makeRule('info', '信息日志', 'keywords', baseInfoKeywords, colors.info),
      makeRule('datetime', '日志时间', 'regex', datetimeRegex, colors.datetime),
      makeRule('network', 'IP 与端口', 'regex', networkRegex, colors.network, { enabled: false }),
      makeRule('debug', '调试追踪', 'keywords', debugKeywords, colors.debug, { enabled: false })
    ]
  }

  if (scene === 'network-ops') {
    return [
      makeRule(
        'error',
        '链路异常/拒绝',
        'keywords',
        `${baseErrorKeywords}, DOWN, NO-CARRIER, UNK, DUP, LOSS, RETRANS, RESET, TIME_WAIT, CLOSE_WAIT, FIN_WAIT1, FIN_WAIT2`,
        colors.error,
        { backgroundEnabled: true }
      ),
      makeRule(
        'success',
        '链路连通/正常',
        'keywords',
        `${baseSuccessKeywords}, UP, LOWER_UP, BROADCAST, MULTICAST, FULL, DUPLEX, SYNCHRONIZED`,
        colors.success
      ),
      makeRule(
        'warn',
        '状态迁移/拥塞',
        'keywords',
        `${baseWarnKeywords}, SYN_SENT, SYN_RECV, CLOSING, LAST_ACK, PROMISC, HALF`,
        colors.warn
      ),
      makeRule('network', 'IP / MAC / 协议', 'regex', networkRegex, colors.network),
      makeRule('number', '带宽/延迟/包数', 'regex', numberRegex, colors.number),
      makeRule('datetime', '时间戳', 'regex', datetimeRegex, colors.datetime),
      makeRule('info', '操作事件', 'keywords', baseInfoKeywords, colors.info),
      makeRule('path', '接口/配置路径', 'regex', pathCommentRegex, colors.path)
    ]
  }

  if (scene === 'shell-config') {
    return [
      makeRule('path', '注释与文件路径', 'regex', pathCommentRegex, colors.path),
      makeRule('boolean', '布尔与开关项', 'keywords', booleanKeywords, colors.boolean),
      makeRule('number', '数值与容量单位', 'regex', numberRegex, colors.number),
      makeRule('network', '地址与端口', 'regex', networkRegex, colors.network),
      makeRule('error', '错误与非法项', 'keywords', baseErrorKeywords, colors.error, { backgroundEnabled: true }),
      makeRule('warn', '警告与废弃项', 'keywords', baseWarnKeywords, colors.warn),
      makeRule('success', '成功与启用项', 'keywords', baseSuccessKeywords, colors.success),
      makeRule('datetime', '日期与时间', 'regex', datetimeRegex, colors.datetime)
    ]
  }

  // 默认：mobaxterm 经典全量
  return [
    makeRule('error', '错误与拒绝', 'keywords', `${baseErrorKeywords}, DOWN`, colors.error, {
      backgroundEnabled: true
    }),
    makeRule('warn', '警告与阻塞', 'keywords', baseWarnKeywords, colors.warn),
    makeRule('success', '成功与运行', 'keywords', `${baseSuccessKeywords}, UP, OPEN`, colors.success),
    makeRule('info', '信息与生命周期', 'keywords', baseInfoKeywords, colors.info),
    makeRule('network', 'IP / MAC / 协议', 'regex', networkRegex, colors.network),
    makeRule('datetime', '日期与时间戳', 'regex', datetimeRegex, colors.datetime),
    makeRule('number', '数值与容量单位', 'regex', numberRegex, colors.number),
    makeRule('boolean', '布尔与空值', 'keywords', booleanKeywords, colors.boolean),
    makeRule('path', '路径与配置注释', 'regex', pathCommentRegex, colors.path),
    makeRule('debug', '调试与追踪', 'keywords', debugKeywords, colors.debug, { enabled: false })
  ]
}

export function createDefaultSettings(): HighlightSettings {
  return {
    schemaVersion: CURRENT_SCHEMA_VERSION,
    enabled: true,
    scene: 'mobaxterm',
    preset: 'default',
    overviewRuler: false,
    rules: createSceneRules('mobaxterm', 'default')
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

const isMatchMode = (value: unknown): value is HighlightMatchMode =>
  value === 'keywords' || value === 'regex'

const isAsciiWordChar = (ch: string): boolean => /^[A-Za-z0-9_]$/.test(ch)

const escapeRegex = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/**
 * 把「关键词词表」转换为正则表达式源码：
 * - 支持用逗号、分号、换行或竖线分隔短语（如 `TIMED OUT`），若无分隔符则按空格分词；
 * - 按长度降序排列，保证 `FAILED` 优先于 `FAIL`、`WARNING` 优先于 `WARN`；
 * - 英文/数字词首尾自动加 `\b` 防止误伤词中子串（如 `OK` 不匹配 `BOOK`），中文或符号不加 `\b`（避免 JS 正则 `\b` 对中文失效）。
 */
export function keywordsToRegexSource(raw: string): string {
  const trimmed = raw.trim()
  if (!trimmed) return ''

  const hasExplicitDelimiter = /[,;\r\n|]/.test(trimmed)
  const parts = hasExplicitDelimiter
    ? trimmed.split(/[,;\r\n|]+/)
    : trimmed.split(/\s+/)

  const tokens = Array.from(
    new Set(
      parts
        .map((item) => item.trim())
        .filter(Boolean)
    )
  ).sort((a, b) => b.length - a.length)

  if (tokens.length === 0) return ''

  const branches = tokens.map((token) => {
    const escaped = escapeRegex(token).replace(/\s+/g, '\\s+')
    const prefix = isAsciiWordChar(token[0]) ? '\\b' : ''
    const suffix = isAsciiWordChar(token[token.length - 1]) ? '\\b' : ''
    return `${prefix}${escaped}${suffix}`
  })

  return `(?:${branches.join('|')})`
}

/** 把任意来源（localStorage / 导入 JSON）的配置收敛成安全可用的结构 */
export function sanitizeSettings(raw: unknown): HighlightSettings {
  const fallback = createDefaultSettings()
  if (!raw || typeof raw !== 'object') return fallback

  const input = raw as Record<string, unknown>
  const preset = typeof input.preset === 'string' && (PRESET_KEYS as string[]).includes(input.preset)
    ? (input.preset as HighlightPresetKey)
    : 'custom'
  const palette = preset !== 'custom' ? HIGHLIGHT_PRESETS[preset].colors : HIGHLIGHT_PRESETS.default.colors

  const rules: HighlightRuleSetting[] = []
  if (Array.isArray(input.rules)) {
    for (const item of input.rules) {
      if (!item || typeof item !== 'object') continue
      const rule = item as Record<string, unknown>
      const level = isLevel(rule.level) ? rule.level : 'custom'
      const colors = palette[level] ?? HIGHLIGHT_PRESETS.default.colors.custom
      const wholeLine = rule.wholeLine === true
      const backgroundEnabled =
        typeof rule.backgroundEnabled === 'boolean'
          ? rule.backgroundEnabled
          : wholeLine || level === 'error'

      rules.push({
        id: typeof rule.id === 'string' && rule.id ? rule.id : nextRuleId(),
        level,
        name: typeof rule.name === 'string' && rule.name ? rule.name : levelLabel(level),
        matchMode: isMatchMode(rule.matchMode) ? rule.matchMode : 'regex',
        pattern: typeof rule.pattern === 'string' ? rule.pattern : '',
        caseSensitive: rule.caseSensitive === true,
        wholeLine,
        foregroundColor: normalizeColor(rule.foregroundColor) ?? colors.fg,
        backgroundColor: normalizeColor(rule.backgroundColor) ?? colors.bg,
        backgroundEnabled,
        enabled: rule.enabled !== false
      })
    }
  }

  const scene = typeof input.scene === 'string' && (SCENE_KEYS as string[]).includes(input.scene)
    ? (input.scene as HighlightSceneKey)
    : 'custom'

  return {
    schemaVersion: CURRENT_SCHEMA_VERSION,
    enabled: input.enabled !== false,
    scene,
    preset,
    overviewRuler: input.overviewRuler === true,
    rules: rules.length > 0 ? rules : fallback.rules
  }
}

function loadSettings(): HighlightSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as Record<string, unknown>
      // 从旧版 v1（仅有 6 条日志规则）平滑升级到 v2（MobaXterm 全量语法规则），并保留用户自定义规则与开关
      if (!parsed || typeof parsed !== 'object' || parsed.schemaVersion !== CURRENT_SCHEMA_VERSION) {
        const legacy = sanitizeSettings(parsed)
        const presetKey = legacy.preset !== 'custom' ? legacy.preset : 'default'
        const upgraded = createDefaultSettings()
        upgraded.enabled = legacy.enabled
        upgraded.overviewRuler = legacy.overviewRuler
        upgraded.preset = legacy.preset
        upgraded.rules = createSceneRules('mobaxterm', presetKey)
        const userCustomRules = legacy.rules.filter(
          (r) => r.level === 'custom' && r.pattern.trim().length > 0
        )
        if (userCustomRules.length > 0) {
          upgraded.rules.push(...userCustomRules)
        }
        return upgraded
      }
      return sanitizeSettings(parsed)
    }

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
      schemaVersion: highlightSettings.schemaVersion,
      enabled: highlightSettings.enabled,
      scene: highlightSettings.scene,
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

/** 立即保存一次升级后的默认配置，确保 localStorage 同步为最新结构 */
persist()

/** 应用配色预设：只替换颜色，保留当前规则、模式与开关 */
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

/** 应用场景规则模板：按所选场景重建内置规则，并保留当前配色预设与非空自定义规则 */
export function applyHighlightScene(sceneKey: HighlightSceneKey): void {
  const presetKey: HighlightPresetKey =
    highlightSettings.preset !== 'custom' ? highlightSettings.preset : 'default'
  const nextRules = createSceneRules(sceneKey, presetKey)
  const customRules = highlightSettings.rules.filter(
    (r) => r.level === 'custom' && r.pattern.trim().length > 0
  )
  highlightSettings.rules.splice(0, highlightSettings.rules.length, ...nextRules, ...customRules)
  highlightSettings.scene = sceneKey
}

/** 用户手动改动规则后，标记为自定义状态 */
export function markPresetCustom(): void {
  if (highlightSettings.preset !== 'custom') highlightSettings.preset = 'custom'
}

export function markSceneCustom(): void {
  if (highlightSettings.scene !== 'custom') highlightSettings.scene = 'custom'
}

export function resetHighlightSettings(): void {
  const defaults = createDefaultSettings()
  highlightSettings.schemaVersion = defaults.schemaVersion
  highlightSettings.enabled = defaults.enabled
  highlightSettings.scene = defaults.scene
  highlightSettings.preset = defaults.preset
  highlightSettings.overviewRuler = defaults.overviewRuler
  highlightSettings.rules.splice(0, highlightSettings.rules.length, ...defaults.rules)
}

/** 校验单条规则的表达式是否合法，合法返回空串，非法返回错误原因 */
export function validateRulePattern(rule: Pick<HighlightRuleSetting, 'matchMode' | 'pattern' | 'caseSensitive'>): string {
  const raw = rule.pattern.trim()
  if (!raw) return ''
  const source = rule.matchMode === 'keywords' ? keywordsToRegexSource(raw) : raw
  if (!source) return ''
  try {
    new RegExp(source, rule.caseSensitive ? '' : 'i')
    return ''
  } catch (error: any) {
    return `正则无效：${error?.message || error}`
  }
}

/** 把配置编译成终端可直接消费的规则；非法正则和空规则在这里被过滤掉 */
export function compileHighlightRules(settings: HighlightSettings): TerminalHighlightRule[] {
  const rules: TerminalHighlightRule[] = []
  for (const rule of settings.rules) {
    if (!rule.enabled) continue
    const raw = rule.pattern.trim()
    if (!raw) continue
    const source = rule.matchMode === 'keywords' ? keywordsToRegexSource(raw) : raw
    if (!source) continue

    let pattern: RegExp
    try {
      pattern = new RegExp(source, rule.caseSensitive ? '' : 'i')
    } catch {
      // 设置面板会提示非法正则，这里静默跳过，不影响其它规则
      continue
    }

    const useBg = rule.wholeLine || rule.backgroundEnabled

    rules.push({
      key: rule.id,
      level: rule.level,
      pattern,
      foregroundColor: rule.foregroundColor,
      backgroundColor: useBg ? rule.backgroundColor : undefined,
      wholeLine: rule.wholeLine
    })
  }
  return rules
}
