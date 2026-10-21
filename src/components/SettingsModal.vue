<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4 transition-opacity"
    @click="$emit('close')"
  >
    <div
      class="w-full max-w-3xl h-[78vh] bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden flex flex-col"
      @click.stop
    >
      <!-- 顶栏 -->
      <div class="px-5 py-3.5 border-b border-surface-border flex items-center justify-between bg-surface-card/30 shrink-0">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-lg bg-brand-500/10 border border-brand-500/20 flex items-center justify-center text-brand-400">
            <Settings class="w-4 h-4" />
          </div>
          <div>
            <h2 class="text-sm font-bold text-white">设置中心</h2>
            <p class="text-[11px] text-surface-subtle">
              配置保存在 <span class="font-mono">~/.snail-ssh/settings.json</span>，改动即时生效
            </p>
          </div>
        </div>
        <button @click="$emit('close')" class="p-1.5 rounded-lg hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors">
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex-1 flex overflow-hidden">
        <!-- 左侧分区导航 -->
        <nav class="w-40 border-r border-surface-border bg-surface-card/20 p-2 space-y-0.5 shrink-0 overflow-y-auto custom-scrollbar">
          <button
            v-for="tab in tabs"
            :key="tab.key"
            @click="activeTab = tab.key"
            class="w-full flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs transition-colors text-left"
            :class="activeTab === tab.key
              ? 'bg-brand-500/15 text-emerald-400 font-medium'
              : 'text-surface-subtle hover:text-slate-200 hover:bg-surface-card'"
          >
            <component :is="tab.icon" class="w-3.5 h-3.5 shrink-0" />
            <span class="truncate">{{ tab.label }}</span>
          </button>
        </nav>

        <!-- 右侧内容 -->
        <div class="flex-1 overflow-y-auto custom-scrollbar p-5 space-y-5">
          <!-- ============ 终端外观 ============ -->
          <template v-if="activeTab === 'terminal'">
            <section class="space-y-3">
              <h3 class="text-xs font-semibold text-slate-200">字体与字号</h3>
              <div class="space-y-1.5">
                <label class="text-[11px] text-slate-400">字体族 (Font Family)</label>
                <input
                  v-model="settings.terminal.fontFamily"
                  type="text"
                  spellcheck="false"
                  class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 transition-colors font-mono"
                />
                <p class="text-[10px] text-slate-500">
                  可填多个字体，按顺序回退；本机未安装的字体由系统替换。
                </p>
              </div>
              <div class="grid grid-cols-2 gap-4">
                <div class="space-y-1.5">
                  <label class="text-[11px] text-slate-400">字号 ({{ settings.terminal.fontSize }}px)</label>
                  <input
                    v-model.number="settings.terminal.fontSize"
                    type="range"
                    min="9"
                    max="28"
                    step="1"
                    class="w-full accent-emerald-500"
                  />
                </div>
                <div class="space-y-1.5">
                  <label class="text-[11px] text-slate-400">行高 ({{ settings.terminal.lineHeight.toFixed(2) }})</label>
                  <input
                    v-model.number="settings.terminal.lineHeight"
                    type="range"
                    min="1"
                    max="2.4"
                    step="0.05"
                    class="w-full accent-emerald-500"
                  />
                </div>
              </div>
            </section>

            <section class="space-y-3 pt-1">
              <h3 class="text-xs font-semibold text-slate-200">光标</h3>
              <div class="grid grid-cols-2 gap-4">
                <div class="space-y-1.5">
                  <label class="text-[11px] text-slate-400">形状</label>
                  <select
                    v-model="settings.terminal.cursorStyle"
                    class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500"
                  >
                    <option value="block">方块 (Block)</option>
                    <option value="underline">下划线 (Underline)</option>
                    <option value="bar">竖线 (Bar)</option>
                  </select>
                </div>
                <label class="flex items-center gap-2 pt-5 text-xs text-slate-300 cursor-pointer">
                  <input v-model="settings.terminal.cursorBlink" type="checkbox" class="accent-emerald-500" />
                  <span>光标闪烁</span>
                </label>
              </div>
            </section>

            <section class="space-y-2 pt-1">
              <h3 class="text-xs font-semibold text-slate-200">配色主题</h3>
              <div class="grid grid-cols-2 gap-2">
                <button
                  v-for="preset in TERMINAL_THEMES"
                  :key="preset.id"
                  @click="settings.terminal.themeId = preset.id"
                  class="flex items-center gap-2.5 p-2 rounded-lg border transition-all text-left"
                  :class="settings.terminal.themeId === preset.id
                    ? 'border-brand-500 bg-brand-500/10'
                    : 'border-surface-border bg-surface-card hover:border-brand-500/40'"
                >
                  <span
                    class="w-9 h-9 rounded-md border border-black/30 shrink-0 flex items-end justify-center gap-0.5 pb-1"
                    :style="{ background: preset.theme.background }"
                  >
                    <span class="w-1 h-2.5 rounded-sm" :style="{ background: preset.theme.red }"></span>
                    <span class="w-1 h-3.5 rounded-sm" :style="{ background: preset.theme.green }"></span>
                    <span class="w-1 h-2 rounded-sm" :style="{ background: preset.theme.yellow }"></span>
                    <span class="w-1 h-3 rounded-sm" :style="{ background: preset.theme.blue }"></span>
                  </span>
                  <span class="text-[11px] text-slate-300 leading-tight">{{ preset.label }}</span>
                </button>
              </div>
            </section>

            <section class="space-y-2 pt-1 border-t border-surface-border/60">
              <label class="flex items-start gap-2.5 cursor-pointer">
                <input v-model="settings.commandBarEnabled" type="checkbox" class="accent-emerald-500 mt-0.5" />
                <span>
                  <span class="text-xs text-slate-200 block">显示独立命令输入框</span>
                  <span class="text-[10px] text-slate-500 block leading-relaxed">
                    终端底部的输入条，适合粘贴长命令、写多行脚本后再一次性发送；
                    <span class="font-mono">vim</span> / <span class="font-mono">htop</span> 等交互式程序仍请直接在终端里操作。
                  </span>
                </span>
              </label>
              <label class="flex items-start gap-2.5 cursor-pointer">
                <input v-model="settings.historySuggestEnabled" type="checkbox" class="accent-emerald-500 mt-0.5" />
                <span>
                  <span class="text-xs text-slate-200 block">显示历史补全</span>
                  <span class="text-[10px] text-slate-500 block leading-relaxed">
                    开启后终端底部固定留出一行，输入时在该行显示历史候选，不再把终端内容挤上挤下。
                    关闭后不显示，也不拦截 <span class="font-mono">Tab</span> / 方向键。
                  </span>
                </span>
              </label>
              <label class="flex items-start gap-2.5 cursor-pointer">
                <input v-model="settings.remoteMonitorEnabled" type="checkbox" class="accent-emerald-500 mt-0.5" />
                <span>
                  <span class="text-xs text-slate-200 block">新建会话默认展开远程监控条</span>
                  <span class="text-[10px] text-slate-500 block leading-relaxed">
                    开启后连接主机时自动在工作区底部显示 CPU / 内存 / 网速 / 磁盘指标栏（每 2.5 秒采样一次）。
                    只影响之后新建的会话；已打开的会话可用文件树工具栏的监控按钮或信息栏右侧红点单独开关。
                  </span>
                </span>
              </label>
            </section>
          </template>

          <!-- ============ 命令片段 ============ -->
          <template v-else-if="activeTab === 'snippets'">
            <section class="space-y-2">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-xs font-semibold text-slate-200">常用指令片段</h3>
                  <p class="text-[10px] text-slate-500">显示在终端工具栏，点击即发送到当前会话</p>
                </div>
                <button
                  @click="addSnippet"
                  class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-[11px] transition-all"
                >
                  <Plus class="w-3 h-3" />
                  <span>新增</span>
                </button>
              </div>

              <div v-if="settings.snippets.length === 0" class="text-[11px] text-slate-500 py-6 text-center border border-dashed border-surface-border rounded-lg">
                还没有片段，点击「新增」添加第一条
              </div>

              <div v-for="(s, i) in settings.snippets" :key="s.id" class="flex items-center gap-2">
                <input
                  v-model="s.label"
                  type="text"
                  placeholder="显示名称"
                  class="w-40 shrink-0 bg-surface-card border border-surface-border rounded-lg px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-brand-500 font-mono"
                />
                <input
                  v-model="s.cmd"
                  type="text"
                  placeholder="发送到终端的命令（可含换行）"
                  class="flex-1 bg-surface-card border border-surface-border rounded-lg px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-brand-500 font-mono"
                />
                <button
                  @click="moveSnippet(i, -1)"
                  :disabled="i === 0"
                  class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                  title="上移"
                >
                  <ChevronUp class="w-3.5 h-3.5" />
                </button>
                <button
                  @click="moveSnippet(i, 1)"
                  :disabled="i === settings.snippets.length - 1"
                  class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                  title="下移"
                >
                  <ChevronDown class="w-3.5 h-3.5" />
                </button>
                <button
                  @click="settings.snippets.splice(i, 1)"
                  class="p-1 rounded hover:bg-rose-500/15 text-slate-400 hover:text-rose-400 transition-colors"
                  title="删除"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>

              <p class="text-[10px] text-slate-500 leading-relaxed pt-1">
                命令末尾是否带换行由你自己决定：带 <span class="font-mono">\n</span> 会立即执行，不带则只填进命令行等待确认。
              </p>
            </section>
          </template>

          <!-- ============ 目录书签 ============ -->
          <template v-else-if="activeTab === 'bookmarks'">
            <section class="space-y-2">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-xs font-semibold text-slate-200">SFTP 常用目录</h3>
                  <p class="text-[10px] text-slate-500">文件工作区书签下拉中显示，支持 <span class="font-mono">~</span> 与绝对路径</p>
                </div>
                <button
                  @click="addBookmark"
                  class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-[11px] transition-all"
                >
                  <Plus class="w-3 h-3" />
                  <span>新增</span>
                </button>
              </div>

              <div v-if="settings.bookmarks.length === 0" class="text-[11px] text-slate-500 py-6 text-center border border-dashed border-surface-border rounded-lg">
                还没有书签
              </div>

              <div v-for="b in settings.bookmarks" :key="b.id" class="flex items-center gap-2">
                <input
                  v-model="b.label"
                  type="text"
                  placeholder="名称"
                  class="w-40 shrink-0 bg-surface-card border border-surface-border rounded-lg px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-brand-500"
                />
                <input
                  v-model="b.path"
                  type="text"
                  placeholder="/var/log"
                  class="flex-1 bg-surface-card border border-surface-border rounded-lg px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-brand-500 font-mono"
                />
                <button
                  @click="settings.bookmarks = settings.bookmarks.filter(x => x.id !== b.id)"
                  class="p-1 rounded hover:bg-rose-500/15 text-slate-400 hover:text-rose-400 transition-colors"
                  title="删除"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </section>
          </template>

          <!-- ============ 主机分组 ============ -->
          <template v-else-if="activeTab === 'groups'">
            <section class="space-y-2">
              <div class="flex items-center justify-between">
                <div>
                  <h3 class="text-xs font-semibold text-slate-200">主机分组</h3>
                  <p class="text-[10px] text-slate-500">
                    分组 id 与主机配置里的 <span class="font-mono">group</span> 字段对应；改名只影响显示，不影响已有主机
                  </p>
                </div>
                <button
                  @click="addGroup"
                  class="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-[11px] transition-all"
                >
                  <Plus class="w-3 h-3" />
                  <span>新增分组</span>
                </button>
              </div>

              <div v-for="g in settings.groups" :key="g.id" class="p-2.5 rounded-lg bg-surface-card/60 border border-surface-border space-y-2">
                <div class="flex items-center gap-2">
                  <span class="w-2.5 h-2.5 rounded-full shrink-0" :class="groupColorOf(g.color).dot"></span>
                  <input
                    v-model="g.label"
                    type="text"
                    placeholder="分组显示名"
                    class="flex-1 bg-surface-card border border-surface-border rounded-lg px-2 py-1.5 text-[11px] text-white focus:outline-none focus:border-brand-500"
                  />
                  <span class="text-[10px] font-mono text-slate-500 px-1.5 py-0.5 rounded bg-surface-highlight shrink-0">
                    {{ g.id }}
                  </span>
                  <label class="flex items-center gap-1.5 text-[10px] text-slate-400 shrink-0 cursor-pointer" title="该分组下的主机会在发送危险命令、批量删除前额外确认">
                    <input v-model="g.danger" type="checkbox" class="accent-rose-500" />
                    <span>高风险</span>
                  </label>
                  <button
                    @click="removeGroup(g.id)"
                    :disabled="settings.groups.length <= 1"
                    class="p-1 rounded hover:bg-rose-500/15 text-slate-400 hover:text-rose-400 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                    title="删除分组（组内主机不会被删除，会回退到「未分组」样式）"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </button>
                </div>
                <div class="flex items-center gap-1.5 pl-4">
                  <button
                    v-for="(palette, key) in GROUP_COLORS"
                    :key="key"
                    @click="g.color = key"
                    class="w-5 h-5 rounded-full border-2 transition-all"
                    :class="[palette.dot, g.color === key ? 'border-white scale-110' : 'border-transparent hover:scale-105']"
                    :title="palette.label"
                  ></button>
                </div>
              </div>
            </section>
          </template>

          <!-- ============ 快捷键 ============ -->
          <template v-else>
            <section class="space-y-2">
              <h3 class="text-xs font-semibold text-slate-200">全局快捷键</h3>
              <div class="rounded-lg border border-surface-border overflow-hidden">
                <div
                  v-for="(item, i) in shortcutList"
                  :key="item.keys"
                  class="flex items-center justify-between px-3 py-2 text-[11px]"
                  :class="i % 2 === 0 ? 'bg-surface-card/40' : ''"
                >
                  <span class="text-slate-300">{{ item.label }}</span>
                  <kbd class="font-mono text-[10px] px-1.5 py-0.5 rounded bg-surface-highlight text-slate-300 border border-surface-border">
                    {{ item.keys }}
                  </kbd>
                </div>
              </div>
              <p class="text-[10px] text-slate-500 leading-relaxed">
                快捷键暂不支持自定义（后续版本提供）。终端聚焦时，<span class="font-mono">Ctrl+K</span> 等全局组合键会透传给远端，避免打断
                <span class="font-mono">readline</span> 操作。
              </p>
            </section>

            <section class="space-y-2 pt-3 border-t border-surface-border/60">
              <h3 class="text-xs font-semibold text-slate-200">配置存储位置</h3>
              <ul class="text-[11px] text-slate-400 space-y-1 font-mono leading-relaxed">
                <li>~/.snail-ssh/settings.json <span class="text-slate-600 font-sans">— 本页所有设置</span></li>
                <li>~/.snail-ssh/hosts.json <span class="text-slate-600 font-sans">— 主机资产（凭证在系统钥匙串）</span></li>
                <li>~/.snail-ssh/history.json <span class="text-slate-600 font-sans">— 命令历史</span></li>
                <li>~/.snail-ssh/logs/ <span class="text-slate-600 font-sans">— 会话日志</span></li>
              </ul>
            </section>
          </template>
        </div>
      </div>

      <!-- 底栏 -->
      <div class="px-5 py-3 border-t border-surface-border flex items-center justify-between bg-surface-card/20 shrink-0">
        <button
          @click="handleReset"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-card hover:bg-surface-elevated border border-surface-border text-[11px] text-slate-300 hover:text-white transition-colors"
        >
          <RotateCcw class="w-3.5 h-3.5" />
          <span>恢复默认设置</span>
        </button>
        <button
          @click="$emit('close')"
          class="px-4 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-[11px] transition-all"
        >
          完成
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { Settings, X, Plus, Trash2, ChevronUp, ChevronDown, RotateCcw, TerminalSquare, Zap, Bookmark, Layers, Keyboard } from 'lucide-vue-next'
import {
  settings,
  TERMINAL_THEMES,
  GROUP_COLORS,
  groupColorOf,
  resetSettings,
  flushSettings
} from '@/composables/useAppSettings'
import { confirmDialog, dialogState } from '@/composables/useDialog'

const props = defineProps<{ open: boolean; initialTab?: string }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

const tabs = [
  { key: 'terminal', label: '终端外观', icon: TerminalSquare },
  { key: 'snippets', label: '命令片段', icon: Zap },
  { key: 'bookmarks', label: '目录书签', icon: Bookmark },
  { key: 'groups', label: '主机分组', icon: Layers },
  { key: 'shortcuts', label: '快捷键', icon: Keyboard }
] as const

type TabKey = typeof tabs[number]['key']
const activeTab = ref<TabKey>('terminal')

const shortcutList = [
  { label: '全局命令面板 / 快速搜索', keys: 'Ctrl + K' },
  { label: '新建服务器配置', keys: 'Ctrl + N' },
  { label: '快速连接（临时会话）', keys: 'Ctrl + Shift + N' },
  { label: '展开 / 折叠传输任务抽屉', keys: 'Ctrl + T' },
  { label: '打开设置中心', keys: 'Ctrl + ,' },
  { label: '切换终端标签（下一个 / 上一个）', keys: 'Ctrl + Tab / Ctrl + Shift + Tab' },
  { label: '切换到第 1~9 个终端标签', keys: 'Ctrl + 1 … 9' },
  { label: '唤出 / 收起命令输入框', keys: 'Ctrl + Enter' },
  { label: '终端内查找', keys: 'Ctrl + F' },
  { label: '复制选中内容', keys: 'Ctrl + Shift + C / Ctrl + Insert' },
  { label: '粘贴', keys: 'Ctrl + Shift + V / Shift + Insert' },
  { label: '退出弹窗 / 关闭搜索条', keys: 'ESC' }
]

const uid = (prefix: string) => `${prefix}-${Date.now()}-${Math.floor(Math.random() * 1000)}`

const addSnippet = () => {
  settings.snippets.push({ id: uid('snip'), label: '', cmd: '' })
}

const moveSnippet = (index: number, delta: number) => {
  const target = index + delta
  if (target < 0 || target >= settings.snippets.length) return
  const list = settings.snippets
  const [item] = list.splice(index, 1)
  list.splice(target, 0, item)
}

const addBookmark = () => {
  settings.bookmarks.push({ id: uid('bm'), label: '', path: '' })
}

const addGroup = () => {
  // id 参与 hosts.json 的 group 字段匹配，必须唯一且尽量语义化
  let index = settings.groups.length + 1
  let id = `group-${index}`
  while (settings.groups.some(g => g.id === id)) {
    index++
    id = `group-${index}`
  }
  settings.groups.push({ id, label: `新分组 ${index}`, color: 'slate', danger: false })
}

const removeGroup = async (id: string) => {
  if (settings.groups.length <= 1) return
  const group = settings.groups.find(g => g.id === id)
  const ok = await confirmDialog({
    title: '删除分组',
    variant: 'warning',
    message: `确定删除分组「${group?.label || id}」吗？`,
    detail: '该分组下的主机不会被删除，但会失去分组归属显示，需要重新编辑主机选择其它分组。',
    confirmText: '删除',
    cancelText: '取消'
  })
  if (!ok) return
  settings.groups = settings.groups.filter(g => g.id !== id)
}

const handleReset = async () => {
  const ok = await confirmDialog({
    title: '恢复默认设置',
    variant: 'danger',
    message: '确定把终端外观、片段、书签、分组全部恢复为默认值吗？',
    detail: '该操作会覆盖 ~/.snail-ssh/settings.json，不会影响主机资产与命令历史。',
    confirmText: '恢复默认',
    cancelText: '取消'
  })
  if (!ok) return
  resetSettings()
  emit('toast', '已恢复默认设置', 'success')
}

const onKeydown = (e: KeyboardEvent) => {
  if (!props.open || dialogState.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    void flushSettings()
    emit('close')
  }
}

watch(() => props.open, (val) => {
  if (val) {
    // 侧边栏入口可指定要打开的分区
    const target = tabs.find(t => t.key === props.initialTab)
    activeTab.value = (target?.key as TabKey) || activeTab.value
  } else {
    void flushSettings()
  }
})

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  void flushSettings()
})
</script>
