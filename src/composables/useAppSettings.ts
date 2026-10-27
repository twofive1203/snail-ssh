import { reactive, ref, watch } from 'vue'
import type { AppSettings, BookmarkItem, HostGroupItem, SnippetItem, TerminalSettings } from '@/types'
import { getSettingsApi, isTauri, saveSettingsApi } from '@/api/tauri'

/**
 * 应用设置单例。
 *
 * 为什么是单例而不是 provide/inject：终端外观、片段、分组颜色需要同时被
 * TerminalView（字体/主题）、Sidebar（分组）、FileTree（书签）、HostModal（分组下拉）
 * 读取，跨多层组件传 props 会让每个中间组件都变成搬运工。
 *
 * 落盘位置：桌面端 `~/.snail-ssh/settings.json`（后端 settings.rs）；
 * 浏览器预览降级到 localStorage，保证 `pnpm dev` 下仍可调试。
 */

/** 内置终端配色预设（仅覆盖 xterm 需要的 16 色 + 背景/前景） */
export interface TerminalThemePreset {
  id: string
  label: string
  theme: {
    background: string
    foreground: string
    cursor: string
    selectionBackground: string
    black: string
    red: string
    green: string
    yellow: string
    blue: string
    magenta: string
    cyan: string
    white: string
  }
}

export const TERMINAL_THEMES: TerminalThemePreset[] = [
  {
    id: 'snail-dark',
    label: 'SnailSSH 暗夜（默认）',
    theme: {
      background: '#0c0e14',
      foreground: '#e2e8f0',
      cursor: '#10b981',
      selectionBackground: 'rgba(16, 185, 129, 0.3)',
      black: '#1e293b',
      red: '#ef4444',
      green: '#10b981',
      yellow: '#f59e0b',
      blue: '#06b6d4',
      magenta: '#d946ef',
      cyan: '#38bdf8',
      white: '#f8fafc'
    }
  },
  {
    id: 'dracula',
    label: 'Dracula',
    theme: {
      background: '#282a36',
      foreground: '#f8f8f2',
      cursor: '#f8f8f2',
      selectionBackground: 'rgba(68, 71, 90, 0.8)',
      black: '#21222c',
      red: '#ff5555',
      green: '#50fa7b',
      yellow: '#f1fa8c',
      blue: '#bd93f9',
      magenta: '#ff79c6',
      cyan: '#8be9fd',
      white: '#f8f8f2'
    }
  },
  {
    id: 'nord',
    label: 'Nord',
    theme: {
      background: '#2e3440',
      foreground: '#d8dee9',
      cursor: '#d8dee9',
      selectionBackground: 'rgba(136, 192, 208, 0.3)',
      black: '#3b4252',
      red: '#bf616a',
      green: '#a3be8c',
      yellow: '#ebcb8b',
      blue: '#81a1c1',
      magenta: '#b48ead',
      cyan: '#88c0d0',
      white: '#e5e9f0'
    }
  },
  {
    id: 'one-dark',
    label: 'One Dark',
    theme: {
      background: '#282c34',
      foreground: '#abb2bf',
      cursor: '#528bff',
      selectionBackground: 'rgba(82, 139, 255, 0.3)',
      black: '#1e2127',
      red: '#e06c75',
      green: '#98c379',
      yellow: '#e5c07b',
      blue: '#61afef',
      magenta: '#c678dd',
      cyan: '#56b6c2',
      white: '#abb2bf'
    }
  },
  {
    id: 'solarized-dark',
    label: 'Solarized Dark',
    theme: {
      background: '#002b36',
      foreground: '#93a1a1',
      cursor: '#93a1a1',
      selectionBackground: 'rgba(7, 54, 66, 0.9)',
      black: '#073642',
      red: '#dc322f',
      green: '#859900',
      yellow: '#b58900',
      blue: '#268bd2',
      magenta: '#d33682',
      cyan: '#2aa198',
      white: '#eee8d5'
    }
  },
  {
    id: 'high-contrast',
    label: '高对比（弱视友好）',
    theme: {
      background: '#000000',
      foreground: '#ffffff',
      cursor: '#ffff00',
      selectionBackground: 'rgba(255, 255, 0, 0.35)',
      black: '#000000',
      red: '#ff5555',
      green: '#55ff55',
      yellow: '#ffff55',
      blue: '#5555ff',
      magenta: '#ff55ff',
      cyan: '#55ffff',
      white: '#ffffff'
    }
  }
]

/** 分组颜色调色板：key 存进配置，Tailwind 类名与十六进制色在组件里映射 */
export const GROUP_COLORS: Record<string, { label: string; dot: string; text: string; border: string; bg: string; hex: string }> = {
  rose: { label: '玫红（生产）', dot: 'bg-rose-500', text: 'text-rose-400', border: 'border-rose-500/40', bg: 'bg-rose-500/10', hex: '#f43f5e' },
  amber: { label: '琥珀（预发）', dot: 'bg-amber-500', text: 'text-amber-400', border: 'border-amber-500/40', bg: 'bg-amber-500/10', hex: '#f59e0b' },
  blue: { label: '蓝色（测试）', dot: 'bg-blue-500', text: 'text-blue-400', border: 'border-blue-500/40', bg: 'bg-blue-500/10', hex: '#3b82f6' },
  emerald: { label: '翠绿（开发）', dot: 'bg-emerald-500', text: 'text-emerald-400', border: 'border-emerald-500/40', bg: 'bg-emerald-500/10', hex: '#10b981' },
  violet: { label: '紫罗兰（个人）', dot: 'bg-violet-500', text: 'text-violet-400', border: 'border-violet-500/40', bg: 'bg-violet-500/10', hex: '#8b5cf6' },
  slate: { label: '中性灰', dot: 'bg-slate-500', text: 'text-slate-400', border: 'border-slate-500/40', bg: 'bg-slate-500/10', hex: '#64748b' }
}

export const groupColorOf = (color?: string) => GROUP_COLORS[color || 'slate'] || GROUP_COLORS.slate

const DEFAULT_SNIPPETS: SnippetItem[] = [
  { id: 'snip-docker-ps', label: 'docker ps', cmd: 'docker ps\n' },
  { id: 'snip-free', label: 'free -h', cmd: 'free -h\n' },
  { id: 'snip-df', label: 'df -h', cmd: 'df -h\n' },
  { id: 'snip-nginx', label: 'systemctl status nginx', cmd: 'systemctl status nginx\n' },
  { id: 'snip-netstat', label: 'netstat -tulpn', cmd: 'netstat -tulpn\n' },
  { id: 'snip-htop', label: 'htop', cmd: 'htop\n' }
]

const DEFAULT_BOOKMARKS: BookmarkItem[] = [
  { id: 'bm-home', label: '家目录 ~', path: '~' },
  { id: 'bm-root', label: '根目录 /', path: '/' },
  { id: 'bm-log', label: '日志 /var/log', path: '/var/log' },
  { id: 'bm-etc', label: '配置 /etc', path: '/etc' },
  { id: 'bm-tmp', label: '临时 /tmp', path: '/tmp' }
]

const DEFAULT_GROUPS: HostGroupItem[] = [
  { id: 'production', label: '生产集群 (Prod)', color: 'rose', danger: true },
  { id: 'staging', label: '测试环境 (Stage)', color: 'amber' },
  { id: 'cloud', label: '个人 VPS & 实验机', color: 'blue' }
]

const DEFAULT_TERMINAL: TerminalSettings = {
  fontFamily: '"JetBrains Mono", "Fira Code", Consolas, monospace',
  fontSize: 13,
  lineHeight: 1.3,
  cursorStyle: 'block',
  cursorBlink: true,
  themeId: 'snail-dark'
}

export const defaultSettings = (): AppSettings => ({
  terminal: { ...DEFAULT_TERMINAL },
  snippets: DEFAULT_SNIPPETS.map(s => ({ ...s })),
  bookmarks: DEFAULT_BOOKMARKS.map(b => ({ ...b })),
  groups: DEFAULT_GROUPS.map(g => ({ ...g })),
  commandBarEnabled: true,
  historySuggestEnabled: true,
  remoteMonitorEnabled: true,
  selectToCopy: true,
  pasteOnRightClick: true
})

/** 旧版选中即复制只写在 localStorage；settings 缺字段时继承，避免升级把用户关过的开关又打开 */
const LEGACY_SELECT_COPY_KEY = 'snail_ssh_select_to_copy'
const legacySelectToCopy = (): boolean => {
  try {
    const raw = localStorage.getItem(LEGACY_SELECT_COPY_KEY)
    if (raw === '0') return false
    if (raw === '1') return true
  } catch {
    // localStorage 不可用时用默认值
  }
  return true
}

const STORAGE_KEY = 'snail_ssh_app_settings_v1'
const SAVE_DEBOUNCE_MS = 400

/** 把任意来源（磁盘 / localStorage / 旧版本）收敛成完整可用的设置结构 */
export const normalizeSettings = (raw: any): AppSettings => {
  const base = defaultSettings()
  if (!raw || typeof raw !== 'object') return base

  const terminal = raw.terminal && typeof raw.terminal === 'object' ? raw.terminal : {}
  const fontSize = Number(terminal.fontSize)
  const lineHeight = Number(terminal.lineHeight)
  return {
    terminal: {
      fontFamily: typeof terminal.fontFamily === 'string' && terminal.fontFamily.trim() ? terminal.fontFamily : base.terminal.fontFamily,
      // 字号/行高来自用户输入或旧配置，夹在可用区间内，避免 0 号字把终端撑坏
      fontSize: Number.isFinite(fontSize) ? Math.min(28, Math.max(9, Math.round(fontSize))) : base.terminal.fontSize,
      lineHeight: Number.isFinite(lineHeight) ? Math.min(2.4, Math.max(1, lineHeight)) : base.terminal.lineHeight,
      cursorStyle: ['block', 'underline', 'bar'].includes(terminal.cursorStyle) ? terminal.cursorStyle : base.terminal.cursorStyle,
      cursorBlink: typeof terminal.cursorBlink === 'boolean' ? terminal.cursorBlink : base.terminal.cursorBlink,
      themeId: TERMINAL_THEMES.some(t => t.id === terminal.themeId) ? terminal.themeId : base.terminal.themeId
    },
    snippets: Array.isArray(raw.snippets)
      ? raw.snippets
          .filter((s: any) => s && typeof s.label === 'string' && typeof s.cmd === 'string' && s.label.trim())
          .map((s: any, i: number) => ({
            id: typeof s.id === 'string' && s.id ? s.id : `snip-${i}-${Date.now()}`,
            label: s.label.trim(),
            cmd: s.cmd
          }))
      : base.snippets,
    bookmarks: Array.isArray(raw.bookmarks)
      ? raw.bookmarks
          .filter((b: any) => b && typeof b.label === 'string' && typeof b.path === 'string' && b.path.trim())
          .map((b: any, i: number) => ({
            id: typeof b.id === 'string' && b.id ? b.id : `bm-${i}-${Date.now()}`,
            label: b.label.trim() || b.path.trim(),
            path: b.path.trim()
          }))
      : base.bookmarks,
    groups: Array.isArray(raw.groups) && raw.groups.length > 0
      ? raw.groups
          .filter((g: any) => g && typeof g.id === 'string' && g.id.trim())
          .map((g: any) => ({
            id: g.id.trim(),
            label: typeof g.label === 'string' && g.label.trim() ? g.label.trim() : g.id.trim(),
            color: GROUP_COLORS[g.color] ? g.color : 'slate',
            danger: !!g.danger
          }))
      : base.groups,
    commandBarEnabled: typeof raw.commandBarEnabled === 'boolean' ? raw.commandBarEnabled : base.commandBarEnabled,
    historySuggestEnabled: typeof raw.historySuggestEnabled === 'boolean' ? raw.historySuggestEnabled : base.historySuggestEnabled,
    remoteMonitorEnabled: typeof raw.remoteMonitorEnabled === 'boolean' ? raw.remoteMonitorEnabled : base.remoteMonitorEnabled,
    selectToCopy: typeof raw.selectToCopy === 'boolean' ? raw.selectToCopy : legacySelectToCopy(),
    pasteOnRightClick: typeof raw.pasteOnRightClick === 'boolean' ? raw.pasteOnRightClick : base.pasteOnRightClick
  }
}

export const settings = reactive<AppSettings>(defaultSettings())
/** 设置是否已从磁盘加载完成（避免首屏用默认值覆盖用户配置） */
export const settingsLoaded = ref(false)
/** 正在应用磁盘/默认值：此时不触发回写，否则每次启动都会重写一遍 settings.json */
let applying = false

const applySettings = (next: AppSettings) => {
  applying = true
  settings.terminal = next.terminal
  settings.snippets = next.snippets
  settings.bookmarks = next.bookmarks
  settings.groups = next.groups
  settings.commandBarEnabled = next.commandBarEnabled
  settings.historySuggestEnabled = next.historySuggestEnabled
  settings.remoteMonitorEnabled = next.remoteMonitorEnabled
  settings.selectToCopy = next.selectToCopy
  settings.pasteOnRightClick = next.pasteOnRightClick
  // 同步 watch 在下一个 tick 才跑，这里用微任务兜底恢复标志，避免漏掉用户紧接的修改
  void Promise.resolve().then(() => { applying = false })
}

export const loadSettings = async () => {
  let raw: any = null
  if (isTauri()) {
    try {
      raw = await getSettingsApi()
    } catch (e) {
      console.warn('读取 settings.json 失败，使用默认设置:', e)
    }
  }
  if (!raw) {
    try {
      const cached = localStorage.getItem(STORAGE_KEY)
      if (cached) raw = JSON.parse(cached)
    } catch {
      // 浏览器预览下 localStorage 不可用时直接用默认值
    }
  }
  applySettings(normalizeSettings(raw))
  settingsLoaded.value = true
}

let saveTimer: number | null = null

const persistNow = async () => {
  const snapshot = JSON.parse(JSON.stringify(settings)) as AppSettings
  if (isTauri()) {
    try {
      await saveSettingsApi(snapshot)
      return
    } catch (e) {
      console.warn('写入 settings.json 失败，回退到 localStorage:', e)
    }
  }
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot))
  } catch {
    // 存储不可用时仅本次会话生效
  }
}

/** 设置改动后防抖落盘：拖动字号滑块时不会每个像素都写一次文件 */
export const scheduleSaveSettings = () => {
  if (!settingsLoaded.value || applying) return
  if (saveTimer !== null) window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(() => {
    saveTimer = null
    void persistNow()
  }, SAVE_DEBOUNCE_MS)
}

/** 立即落盘（关闭设置弹窗 / 恢复默认值时使用） */
export const flushSettings = async () => {
  if (saveTimer !== null) {
    window.clearTimeout(saveTimer)
    saveTimer = null
  }
  await persistNow()
}

watch(settings, () => scheduleSaveSettings(), { deep: true })

export const resetSettings = () => {
  applySettings(defaultSettings())
  void flushSettings()
}

export const currentTerminalTheme = () =>
  TERMINAL_THEMES.find(t => t.id === settings.terminal.themeId) || TERMINAL_THEMES[0]

/** 按分组 id 取分组定义；未登记的分组回退到中性样式，保证旧 hosts.json 不炸 */
export const groupById = (id?: string): HostGroupItem =>
  settings.groups.find(g => g.id === id) || { id: id || 'unknown', label: id || '未分组', color: 'slate' }
