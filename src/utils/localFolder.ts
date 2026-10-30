import type { LocalFolderItem } from '@/types'

/** 目录清单上限。再多会让资产页和设置文件都难维护。 */
export const LOCAL_FOLDER_LIMIT = 100

/**
 * 用户主目录的保留 id。
 * 不写入清单，这样「在用户主目录打开」不占 100 条名额，也不会被删目录误伤。
 */
export const LOCAL_HOME_FOLDER_ID = 'local-home'

/** 路径比较是否忽略大小写。只看当前系统，不做真实路径归一。 */
export function isWindowsPathHost(): boolean {
  if (typeof navigator === 'undefined') return false
  const platform = navigator.platform || ''
  const ua = navigator.userAgent || ''
  return /win/i.test(platform) || /windows/i.test(ua)
}

/**
 * 只做字符串比较键：首尾空白、末尾分隔符，以及 `/` 与 `\`。
 * 根路径保留。不 canonicalize，因此符号链接和 8.3 短名不会被当成同一条。
 */
export function pathCompareKey(input: string, caseInsensitive = isWindowsPathHost()): string {
  let value = input.trim().replace(/[\\/]+/g, '/')
  if (value.length > 1) value = value.replace(/\/+$/, '')
  if (!value) value = '/'
  return caseInsensitive ? value.toLowerCase() : value
}

export function sameLocalPath(a: string, b: string, caseInsensitive = isWindowsPathHost()): boolean {
  return pathCompareKey(a, caseInsensitive) === pathCompareKey(b, caseInsensitive)
}

/** 空显示名不能落成空标题；盘符根和 `/` 不能被截成空字符串。 */
export function folderLabelFromPath(path: string): string {
  const trimmed = path.trim()
  if (!trimmed) return '本地目录'
  const unified = trimmed.replace(/[\\/]+/g, '/')
  const stripped = unified.length > 1 ? unified.replace(/\/+$/, '') : unified
  if (!stripped || stripped === '/') return '/'
  if (/^[a-zA-Z]:$/.test(stripped)) return `${stripped[0].toUpperCase()}:\\`
  const parts = stripped.split('/').filter(Boolean)
  return parts[parts.length - 1] || stripped
}

function newFolderId(index: number): string {
  const rand = Math.random().toString(36).slice(2, 8)
  return `lf-${index}-${Date.now().toString(36)}-${rand}`
}

/**
 * 丢弃缺路径、空路径和非对象项；补 id 和空显示名；按比较键去重并保留先出现的一条。
 * 调用方要先判断字段是否存在：类型错误的字段应得到空列表，而不是在这里回退去读 cwd。
 */
export function normalizeLocalFolderList(items: unknown, caseInsensitive = isWindowsPathHost()): LocalFolderItem[] {
  if (!Array.isArray(items)) return []
  const seenPaths = new Set<string>()
  const seenIds = new Set<string>()
  const result: LocalFolderItem[] = []
  for (let i = 0; i < items.length; i++) {
    const item = items[i]
    if (!item || typeof item !== 'object') continue
    const record = item as Record<string, unknown>
    if (typeof record.path !== 'string') continue
    const path = record.path.trim()
    if (!path) continue
    const key = pathCompareKey(path, caseInsensitive)
    if (seenPaths.has(key)) continue
    seenPaths.add(key)
    let id = typeof record.id === 'string' ? record.id.trim() : ''
    if (!id || id === LOCAL_HOME_FOLDER_ID || seenIds.has(id)) id = newFolderId(i)
    seenIds.add(id)
    const rawLabel = typeof record.label === 'string' ? record.label.trim() : ''
    result.push({
      id,
      label: rawLabel || folderLabelFromPath(path),
      path
    })
    if (result.length >= LOCAL_FOLDER_LIMIT) break
  }
  return result
}

/**
 * 迁移只看原始 JSON 有没有 `localFolders` 字段。
 * 没有字段且旧 cwd 非空时收成一条；字段已是数组（包括空数组）或类型错误时都不再从 cwd 补。
 */
export function localFoldersFromSettings(raw: unknown, caseInsensitive = isWindowsPathHost()): LocalFolderItem[] {
  if (!raw || typeof raw !== 'object') return []
  const source = raw as Record<string, unknown>
  if (Object.prototype.hasOwnProperty.call(source, 'localFolders')) {
    return normalizeLocalFolderList(source.localFolders, caseInsensitive)
  }
  const shell = source.localShell
  const cwd = shell && typeof shell === 'object' && typeof (shell as Record<string, unknown>).cwd === 'string'
    ? ((shell as Record<string, unknown>).cwd as string).trim()
    : ''
  if (!cwd) return []
  return normalizeLocalFolderList([{ path: cwd, label: '' }], caseInsensitive)
}

/** 本次读入是否把旧的单一工作目录收进了清单。为 true 时应立即落盘，避免下次启动再插一条。 */
export function settingsMigratedLocalCwd(raw: unknown): boolean {
  if (!raw || typeof raw !== 'object') return false
  const source = raw as Record<string, unknown>
  if (Object.prototype.hasOwnProperty.call(source, 'localFolders')) return false
  const shell = source.localShell
  const cwd = shell && typeof shell === 'object' && typeof (shell as Record<string, unknown>).cwd === 'string'
    ? ((shell as Record<string, unknown>).cwd as string).trim()
    : ''
  return cwd.length > 0
}
