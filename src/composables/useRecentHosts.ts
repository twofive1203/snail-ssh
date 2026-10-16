import { ref } from 'vue'
import type { HostItem } from '@/types'

/**
 * 最近连接记录（MVP 版，存 localStorage，不落 ~/.snail-ssh/）
 * 理由：这是纯 UI 辅助信息，丢失可接受；等引入「设置中心」后再统一迁到后端。
 */
export interface RecentHostEntry {
  hostId: string
  name: string
  host: string
  port: number
  username: string
  lastUsedAt: number
}

const STORAGE_KEY = 'snail_ssh_recent'
/** 落盘上限，读取展示时只取前 N 条 */
const MAX_STORE = 20
export const MAX_RECENT_DISPLAY = 5

const isValidEntry = (v: any): v is RecentHostEntry =>
  !!v && typeof v.hostId === 'string' && typeof v.name === 'string' && typeof v.host === 'string'

const loadFromStorage = (): RecentHostEntry[] => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(isValidEntry).slice(0, MAX_STORE)
  } catch {
    return []
  }
}

// 模块级单例：所有调用方共享同一份响应式数据
const recents = ref<RecentHostEntry[]>(loadFromStorage())

const persist = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(recents.value.slice(0, MAX_STORE)))
  } catch {
    // 隐私模式 / 配额满时静默降级，不影响主流程
  }
}

/** 记录一次使用（按最近使用时间倒序，去重） */
const touchRecent = (host: HostItem) => {
  const entry: RecentHostEntry = {
    hostId: host.id,
    name: host.name,
    host: host.host,
    port: host.port,
    username: host.username,
    lastUsedAt: Date.now()
  }
  recents.value = [entry, ...recents.value.filter(r => r.hostId !== host.id)].slice(0, MAX_STORE)
  persist()
}

const removeRecent = (hostId: string) => {
  recents.value = recents.value.filter(r => r.hostId !== hostId)
  persist()
}

const clearRecents = () => {
  recents.value = []
  persist()
}

export function useRecentHosts() {
  return { recents, touchRecent, removeRecent, clearRecents }
}
