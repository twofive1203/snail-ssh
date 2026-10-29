import { computed, watch } from 'vue'
import type { HostItem, RecentHostRecord } from '@/types'
import { isRecentHostRecord, setRecentHosts, settings, settingsLoaded } from '@/composables/useAppSettings'

/**
 * 最近连接记录。
 *
 * 唯一数据源是 settings.recentHosts：桌面端随 settings.json 落在 ~/.snail-ssh/，
 * 浏览器预览由 useAppSettings 自动降级到 localStorage（这里不写特例分支）。
 * 早先单独存 localStorage 的 snail_ssh_recent 会随「清除站点数据」一起丢，也不跟用户换机迁移，
 * 与「所有配置都在 ~/.snail-ssh/」的承诺不符，因此改为并入设置。
 */

/** 旧版独立存储键，只用于下面的一次性迁移 */
const LEGACY_STORAGE_KEY = 'snail_ssh_recent'
/** 落盘上限，读取展示时只取前 N 条 */
const MAX_STORE = 20
export const MAX_RECENT_DISPLAY = 5

const recents = computed<RecentHostRecord[]>(() => settings.recentHosts ?? [])

/**
 * 一次性迁移：把老版本留在 localStorage 的记录搬进 settings。
 *
 * 为什么搬完就删旧键：迁移判据是「settings 里为空」，若旧键留着，用户清空最近连接后
 * 下次启动又会被旧数据「复活」，等于清不掉。删除旧键即宣告迁移结束，天然只做一次。
 * 迁移失败（localStorage 不可用 / JSON 损坏）只是丢掉一条辅助信息，绝不能影响启动。
 */
let migrationAttempted = false
const migrateLegacyRecents = () => {
  if (migrationAttempted || !settingsLoaded.value) return
  migrationAttempted = true
  try {
    // settings.json 已有记录说明用户已在新版本写过，旧数据更陈旧，直接丢弃
    if (recents.value.length > 0) {
      localStorage.removeItem(LEGACY_STORAGE_KEY)
      return
    }
    const raw = localStorage.getItem(LEGACY_STORAGE_KEY)
    if (!raw) return
    const parsed: unknown = JSON.parse(raw)
    const valid = Array.isArray(parsed) ? parsed.filter(isRecentHostRecord).slice(0, MAX_STORE) : []
    if (valid.length > 0) setRecentHosts(valid)
    localStorage.removeItem(LEGACY_STORAGE_KEY)
  } catch {
    // 本次会话不再重试，但保留旧键，下次启动还有机会（例如隐私模式下一时读不到）
  }
}

// settings.json 是异步读的，而本模块在 App setup 阶段就被调用；等加载完成再迁，避免用默认值误判「settings 为空」
watch(settingsLoaded, loaded => { if (loaded) migrateLegacyRecents() }, { immediate: true })

/** 记录一次使用（按最近使用时间倒序，去重） */
const touchRecent = (host: HostItem) => {
  const entry: RecentHostRecord = {
    hostId: host.id,
    name: host.name,
    host: host.host,
    port: host.port,
    username: host.username,
    lastUsedAt: Date.now()
  }
  setRecentHosts([entry, ...recents.value.filter(r => r.hostId !== host.id)].slice(0, MAX_STORE))
}

const removeRecent = (hostId: string) => {
  setRecentHosts(recents.value.filter(r => r.hostId !== hostId))
}

const clearRecents = () => {
  setRecentHosts([])
}

export function useRecentHosts() {
  return { recents, touchRecent, removeRecent, clearRecents }
}
