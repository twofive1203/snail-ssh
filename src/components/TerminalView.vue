<template>
  <section class="flex-1 flex flex-col h-full overflow-hidden bg-[#0c0e14]">
    <!-- 终端标签页管理条 (Tab Bar) -->
    <div class="h-10 bg-surface-sidebar border-b border-surface-border flex items-center justify-between px-2 shrink-0 select-none">
      <div class="flex items-center gap-1 overflow-x-auto no-scrollbar flex-1 mr-2">
        <div
          v-for="(tab, idx) in tabs"
          :key="tab.id"
          @click="$emit('switch-tab', idx)"
          @contextmenu.prevent="openTabMenu($event, idx)"
          :class="idx === activeTabIndex ? 'bg-[#0c0e14] border-t-2 text-slate-200 border-x border-surface-border/50' : 'bg-surface-card/40 hover:bg-surface-card text-surface-subtle hover:text-slate-300 border-t-2 border-transparent'"
          :style="idx === activeTabIndex ? { borderTopColor: groupColorHex } : undefined"
          class="group flex items-center gap-2 px-3 py-1.5 rounded-t-lg text-xs cursor-pointer transition-all shrink-0"
        >
          <!-- 环境色点：生产红 / 预发黄 / 测试蓝，降低「在哪个环境敲命令」的认知成本 -->
          <span
            class="w-2 h-2 rounded-full shrink-0"
            :class="[tab.connected ? groupColorDot : 'bg-rose-500', tab.connected ? 'animate-pulse' : '']"
            :title="transport === 'local' ? (tab.sub || '本地终端') : currentGroup.label"
          ></span>
          <span class="font-mono font-medium">{{ tab.title }}</span>
          <span class="text-[10px] font-mono text-slate-500 hidden sm:inline">{{ tab.sub }}</span>
          <button
            @click.stop="$emit('close-tab', idx)"
            class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-all ml-1"
          >
            <X class="w-3 h-3" />
          </button>
        </div>

        <!-- 添加新终端 -->
        <button
          @click="$emit('add-tab')"
          class="p-1 rounded-md text-surface-subtle hover:text-white hover:bg-surface-card transition-colors ml-1"
          :title="transport === 'local' ? '打开新的本地终端' : '连接新的服务器终端'"
        >
          <Plus class="w-4 h-4" />
        </button>
      </div>

      <!-- 终端操作按钮组：展开文件树、查找、清屏 -->
      <div class="flex items-center gap-1 text-xs">
        <!-- 会话日志录制指示 -->
        <span
          v-if="currentLogging"
          class="flex items-center gap-1 px-1.5 py-0.5 rounded bg-rose-500/15 text-rose-400 border border-rose-500/30 text-[10px] font-mono shrink-0 select-none"
          :title="currentLogPath ? `正在记录会话日志: ${currentLogPath}` : '正在记录会话日志'"
        >
          <span class="w-1.5 h-1.5 rounded-full bg-rose-500 animate-pulse"></span>
          REC
        </span>

        <button
          v-if="sidebarCollapsed && transport !== 'local'"
          @click="$emit('toggle-sidebar')"
          class="flex items-center gap-1.5 px-2 py-1 rounded bg-brand-500/10 text-emerald-400 border border-brand-500/30 hover:bg-brand-500/20 text-xs font-medium transition-all"
          title="展开左侧文件工作区"
        >
          <FolderGit2 class="w-3.5 h-3.5" />
          <span class="hidden sm:inline">文件工作区</span>
        </button>

        <!-- 切换到全屏 SFTP 视图 -->
        <button
          v-if="transport !== 'local'"
          @click="$emit('jump-to-sftp')"
          class="flex items-center gap-1.5 px-2 py-1 rounded bg-surface-card hover:bg-surface-elevated border border-surface-border hover:border-cyan-500/40 text-cyan-400 text-xs font-medium transition-all"
          title="切换至独立全屏 SFTP 传输视图"
        >
          <FolderGit2 class="w-3.5 h-3.5" />
          <span class="hidden sm:inline">全屏 SFTP</span>
        </button>

        <span class="w-px h-4 bg-surface-border shrink-0"></span>

        <button
          @click="toggleSelectCopy"
          :title="settings.selectToCopy ? '关闭选中即复制' : '开启选中即复制（拖选后自动写入剪贴板）'"
          class="p-1.5 rounded hover:bg-surface-elevated transition-colors"
          :class="settings.selectToCopy ? 'text-emerald-300 bg-emerald-500/10' : 'text-surface-subtle hover:text-white'"
        >
          <TextSelect class="w-3.5 h-3.5" />
        </button>

        <button
          @click="toggleHighlight"
          :title="highlightEnabled ? '关闭语法高亮' : '开启语法高亮（MobaXterm 级实时语法着色）'"
          class="p-1.5 rounded hover:bg-surface-elevated transition-colors"
          :class="highlightEnabled ? 'text-amber-300 bg-amber-500/10' : 'text-surface-subtle hover:text-white'"
        >
          <Highlighter class="w-3.5 h-3.5" />
        </button>

        <!-- 错误行导航：仅在开启高亮时出现 -->
        <template v-if="highlightEnabled">
          <button
            @click="jumpToProblem(-1)"
            class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
            title="上一个错误行"
          >
            <ChevronUp class="w-3.5 h-3.5" />
          </button>
          <button
            @click="jumpToProblem(1)"
            class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
            title="下一个错误行"
          >
            <ChevronDown class="w-3.5 h-3.5" />
          </button>
        </template>

        <button
          @click="highlightSettingsOpen = true"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="终端语法高亮设置"
        >
          <SlidersHorizontal class="w-3.5 h-3.5" />
        </button>

        <button
          @click="toggleSearch"
          :disabled="!currentTab"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :class="searchOpen ? 'text-brand-300 bg-brand-500/10' : ''"
          title="在终端中查找 (Ctrl+F)"
        >
          <Search class="w-3.5 h-3.5" />
        </button>

        <button
          @click="clearCurrentTerminal"
          class="p-1.5 rounded hover:bg-surface-elevated text-surface-subtle hover:text-white transition-colors"
          title="清除屏幕 (Ctrl+L)"
        >
          <Eraser class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- 标签页右键菜单：复制会话 / 复制 IP / 标签管理 -->
    <ContextMenu
      :open="tabMenu.open"
      :x="tabMenu.x"
      :y="tabMenu.y"
      :items="tabMenuItems"
      @select="handleTabMenuSelect"
      @close="tabMenu.open = false"
    />

    <!-- 终端内右键菜单：复制 / 粘贴 / 全选 / 查找 / 清除屏幕。右键粘贴开启时需 Shift/Ctrl+右键 -->
    <ContextMenu
      :open="termMenu.open"
      :x="termMenu.x"
      :y="termMenu.y"
      :items="termMenuItems"
      @select="handleTermMenuSelect"
      @close="termMenu.open = false"
    />

    <!-- 常用指令代码片段工具栏 -->
    <div class="h-8 bg-surface-card/40 border-b border-surface-border/60 px-3 flex items-center justify-between text-xs shrink-0 select-none">
      <div v-if="transport !== 'local'" class="flex items-center gap-2 overflow-x-auto no-scrollbar py-1">
        <span class="text-[10px] font-mono text-slate-500 uppercase flex items-center gap-1">
          <Zap class="w-3 h-3 text-amber-400" />
          <span>常用指令:</span>
        </span>
        <button
          v-for="s in settings.snippets"
          :key="s.id"
          @click="sendSnippetCommand(s.cmd)"
          :disabled="!currentTab"
          class="px-2 py-0.5 rounded bg-surface-card hover:bg-surface-elevated text-[11px] font-mono text-slate-300 hover:text-brand-300 border border-surface-border transition-colors shrink-0 disabled:opacity-40 disabled:cursor-not-allowed"
          :title="s.cmd.trim()"
        >
          {{ s.label }}
        </button>
        <span v-if="settings.snippets.length === 0" class="text-[10px] text-slate-500 font-mono">
          暂无片段，可在设置中心添加
        </span>
      </div>
      <div v-else class="flex items-center gap-2 min-w-0 text-[11px] font-mono text-slate-400">
        <SquareTerminal class="w-3.5 h-3.5 text-sky-400 shrink-0" />
        <span class="text-slate-300">本地终端</span>
        <span class="text-slate-500 truncate">{{ currentTab?.sub || '自动探测' }}</span>
      </div>
      <div class="flex items-center gap-3 text-[11px] font-mono text-slate-500">
        <span>UTF-8</span>
        <span>xterm-256color</span>
        <span v-if="termSize">{{ termSize.cols }}x{{ termSize.rows }}</span>
      </div>
    </div>

    <!-- 真实 xterm.js 容器：每个标签页一个独立实例，切换不丢回显 -->
    <div
      class="flex-1 w-full h-full relative overflow-hidden bg-[#0c0e14] p-2"
    >
      <!-- 终端内搜索条 (Ctrl+F)：贴在容器右上角，不抢占终端焦点 -->
      <div
        v-if="searchOpen"
        class="absolute top-3 right-4 z-30 flex items-center gap-1 bg-surface-sidebar/95 backdrop-blur-xl border border-surface-border rounded-lg shadow-2xl pl-2 pr-1 py-1"
      >
        <Search class="w-3.5 h-3.5 text-slate-500 shrink-0" />
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          spellcheck="false"
          placeholder="在终端中查找"
          class="w-40 bg-transparent text-xs font-mono text-slate-200 placeholder:text-slate-600 outline-none"
          @keydown.enter.prevent="onSearchEnter"
          @keydown.esc.prevent.stop="closeSearch"
        />
        <span class="text-[10px] font-mono text-slate-500 min-w-[42px] text-center shrink-0">{{ searchResultText }}</span>
        <button
          @click="runSearch('prev')"
          :disabled="!searchQuery"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
          title="上一个 (Shift+Enter)"
        >
          <ChevronUp class="w-3.5 h-3.5" />
        </button>
        <button
          @click="runSearch('next')"
          :disabled="!searchQuery"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
          title="下一个 (Enter)"
        >
          <ChevronDown class="w-3.5 h-3.5" />
        </button>
        <span class="w-px h-4 bg-surface-border shrink-0"></span>
        <button
          @click="searchCaseSensitive = !searchCaseSensitive"
          class="p-1 rounded transition-colors"
          :class="searchCaseSensitive ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="区分大小写"
        >
          <CaseSensitive class="w-3.5 h-3.5" />
        </button>
        <button
          @click="searchWholeWord = !searchWholeWord"
          class="p-1 rounded transition-colors"
          :class="searchWholeWord ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="全字匹配"
        >
          <WholeWord class="w-3.5 h-3.5" />
        </button>
        <button
          @click="searchRegex = !searchRegex"
          class="p-1 rounded transition-colors"
          :class="searchRegex ? 'bg-brand-500/20 text-brand-300' : 'text-slate-400 hover:text-white hover:bg-surface-elevated'"
          title="正则表达式"
        >
          <Regex class="w-3.5 h-3.5" />
        </button>
        <span class="w-px h-4 bg-surface-border shrink-0"></span>
        <button
          @click="closeSearch"
          class="p-1 rounded hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors"
          title="关闭 (Esc)"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>

      <!-- 断线提示条：固定在顶部不遮挡历史回显，一键重建连接（重连后沿用同一个 xterm） -->
      <div
        v-if="disconnectedTab"
        class="absolute top-2 left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 pl-2.5 pr-1.5 py-1.5 rounded-lg bg-surface-sidebar/95 backdrop-blur-xl border border-rose-500/40 shadow-2xl"
      >
        <RefreshCw class="w-4 h-4 text-rose-400 shrink-0" />
        <div class="min-w-0">
          <p class="text-xs text-slate-200 font-medium leading-tight">{{ transport === 'local' ? '终端已退出' : '连接已断开' }}</p>
          <p class="text-[10px] font-mono text-slate-500 truncate max-w-[240px] leading-tight">
            {{ disconnectedTab.closeReason || (transport === 'local' ? '本地进程已结束' : '远端会话已结束') }}
          </p>
        </div>
        <button
          @click="$emit('reconnect-tab', activeTabIndex)"
          :disabled="disconnectedTab.reconnecting"
          class="px-2 py-1 rounded text-[11px] font-medium bg-brand-500/15 text-emerald-300 border border-brand-500/40 hover:bg-brand-500/25 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
          :title="transport === 'local' ? '保留当前回显，重新打开本地终端' : '保留当前回显，重新建立 SSH 会话'"
        >
          {{ disconnectedTab.reconnecting ? (transport === 'local' ? '正在打开...' : '重连中...') : (transport === 'local' ? '重新打开' : '重新连接') }}
        </button>
        <button
          @click="$emit('close-tab', activeTabIndex)"
          class="p-1 rounded text-slate-400 hover:text-white hover:bg-surface-elevated transition-colors shrink-0"
          title="关闭该终端"
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>

      <div
        v-if="tabs.length === 0"
        class="w-full h-full flex flex-col items-center justify-center gap-3 text-center"
      >
        <TerminalIcon class="w-10 h-10 text-slate-600" />
        <p class="text-sm text-slate-400">{{ transport === 'local' ? '当前没有本地终端' : '当前没有活动的终端会话' }}</p>
        <p class="text-xs text-slate-500 font-mono">{{ transport === 'local' ? '点上方加号再开一个本地终端' : '请在「服务器资产」中选择主机并点击「连接 SSH」，或用 Ctrl+K 快速直连' }}</p>
        <p class="text-[11px] text-slate-600 font-mono">小技巧：Ctrl+Enter 唤出命令输入框，Ctrl+, 打开设置中心</p>
      </div>

      <div
        v-for="tab in tabs"
        :key="tab.uiId"
        :ref="el => setContainer(tab.uiId, el)"
        class="w-full h-full"
        :style="{ display: tab.id === currentTabId ? 'block' : 'none' }"
        @contextmenu.prevent="onTerminalContextMenu($event)"
      ></div>

    </div>

    <!-- 历史补全固定占一行：候选显隐只换内容，不再改变终端高度、也不再 fit / 通知远端改窗口 -->
    <div
      v-if="settings.historySuggestEnabled"
      class="shrink-0 h-8 border-t border-surface-border/60 bg-[#0c0e14] px-3 flex items-center"
    >
      <CommandSuggestBar
        :candidates="suggestCandidates"
        :active-index="suggestIndex"
        :visible="suggestBarVisible"
        @accept="handleSuggestionAccept"
        @hover="handleSuggestionHover"
        @leave="handleSuggestionLeave"
      />
    </div>

    <!-- 独立命令输入框（对标 Xshell Compose Bar / FinalShell 命令输入框）
         Ctrl+Enter 唤出；Shift+Enter 换行，Enter 发送。只用于「写好再发」，
         交互式程序（vim/htop/top）仍需直接在终端里操作。 -->
    <div
      v-if="settings.commandBarEnabled"
      class="shrink-0 border-t border-surface-border bg-surface-sidebar/80"
    >
      <div v-if="!commandBarOpen" class="h-7 px-3 flex items-center justify-between text-[10px] font-mono text-slate-500 select-none">
        <button
          @click="openCommandBar"
          :disabled="!currentTab"
          class="flex items-center gap-1.5 hover:text-brand-300 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          title="打开独立命令输入框 (Ctrl+Enter)"
        >
          <SquareTerminal class="w-3 h-3" />
          <span>命令输入框</span>
          <kbd class="px-1 py-0.2 rounded bg-surface-highlight text-slate-400 border border-surface-border">Ctrl+Enter</kbd>
        </button>
        <span class="text-slate-600">Enter 发送 · Shift+Enter 换行 · Esc 收起</span>
      </div>

      <div v-else class="px-2 py-2 space-y-1.5">
        <div class="flex items-start gap-2">
          <div class="flex-1 relative">
            <textarea
              ref="commandInputRef"
              v-model="commandDraft"
              rows="1"
              spellcheck="false"
              placeholder="在此输入命令（支持多行草稿，Enter 发送 / Shift+Enter 换行）"
              class="w-full bg-[#0a0d14] border border-surface-border rounded-lg px-2.5 py-1.5 text-xs font-mono text-slate-200 placeholder:text-slate-600 focus:outline-none focus:border-brand-500/70 focus:ring-1 focus:ring-brand-500/30 resize-none leading-5 max-h-40 overflow-y-auto custom-scrollbar"
              :style="{ height: draftHeight + 'px' }"
              @keydown="onCommandBarKeydown"
              @input="autoGrowDraft"
            ></textarea>
            <!-- 历史候选：复用与终端内联想相同的词库，避免两套逻辑；
                 仅在按 ↑/↓ 浏览历史时展开，否则会一直挡住上方终端 -->
            <div
              v-if="historyIndex >= 0 && commandHistory.length > 0"
              class="absolute left-0 right-0 bottom-full mb-1 max-h-48 overflow-y-auto custom-scrollbar bg-surface-sidebar/98 backdrop-blur-xl border border-surface-border rounded-lg shadow-2xl z-30"
            >
              <button
                v-for="(item, idx) in commandHistory"
                :key="item.cmd"
                @mousedown.prevent="pickHistoryCommand(item.cmd)"
                class="w-full text-left px-2.5 py-1 text-[11px] font-mono truncate transition-colors"
                :class="idx === historyIndex ? 'bg-brand-500/15 text-brand-300' : 'text-slate-300 hover:bg-surface-elevated'"
              >
                {{ item.cmd }}
              </button>
            </div>
          </div>
          <button
            @click="sendCommandDraft"
            :disabled="!commandDraft.trim() || !currentTab"
            class="shrink-0 px-3 py-1.5 rounded-lg bg-brand-500 hover:bg-brand-600 text-slate-950 font-semibold text-[11px] transition-all disabled:opacity-40 disabled:cursor-not-allowed"
            title="发送到当前终端 (Enter)"
          >
            发送
          </button>
          <button
            @click="closeCommandBar"
            class="shrink-0 p-1.5 rounded-lg hover:bg-surface-elevated text-slate-400 hover:text-white transition-colors"
            title="收起 (Esc)"
          >
            <X class="w-3.5 h-3.5" />
          </button>
        </div>
        <div class="flex items-center justify-between text-[10px] font-mono text-slate-500 px-0.5">
          <span class="flex items-center gap-2">
            <span v-if="dangerousDraft" class="flex items-center gap-1 text-rose-400">
              <TriangleAlert class="w-3 h-3" />
              <span>危险命令：{{ dangerousDraft.label }}</span>
            </span>
            <span v-else-if="draftLineCount > 1" class="text-amber-400/80">
              多行草稿 · {{ draftLineCount }} 行，将一次性发送
            </span>
            <span v-else class="text-slate-600">发送目标: {{ currentTab?.sub || '未连接' }}</span>
          </span>
          <span class="text-slate-600">↑/↓ 浏览历史 · Shift+Enter 换行</span>
        </div>
      </div>
    </div>

    <!-- 终端状态栏 -->
    <div class="h-6 bg-surface-sidebar border-t border-surface-border px-3 flex items-center justify-between text-[11px] font-mono text-surface-subtle shrink-0">
      <div class="flex items-center gap-3">
        <span class="flex items-center gap-1.5" :class="currentTab?.connected ? 'text-emerald-400' : 'text-rose-400'">
          <span class="w-1.5 h-1.5 rounded-full" :class="currentTab?.connected ? 'bg-emerald-400' : 'bg-rose-400'"></span>
          <span>{{ currentTab?.connected ? 'SSH 协议就绪 (OpenSSH 2.0 / Pty)' : '会话未连接' }}</span>
        </span>
        <span>编码: UTF-8</span>
        <span :class="currentLogging ? 'text-rose-400' : ''">{{ currentLogging ? '日志: 记录中' : '日志: 已关闭' }}</span>
      </div>
      <div>
        <span>快捷键: Ctrl+C 中断 | Ctrl+L 清屏</span>
      </div>
    </div>

    <!-- 高亮规则设置（全局配置，改动即时生效） -->
    <HighlightSettingsModal
      :open="highlightSettingsOpen"
      @close="highlightSettingsOpen = false"
      @toast="emitToast"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { Terminal as Xterm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { SearchAddon } from '@xterm/addon-search'
import { Unicode11Addon } from '@xterm/addon-unicode11'
import {
  X, Plus, FolderGit2, Eraser, Zap, Terminal as TerminalIcon, SquareTerminal, TriangleAlert,
  CopyPlus, Globe, User, Terminal, PenLine, RefreshCw, XCircle, ChevronsRight,
  Search, ChevronUp, ChevronDown, CaseSensitive, WholeWord, Regex,
  Copy, ClipboardPaste, TextSelect, Highlighter, FolderOpen, CircleDot, CircleStop,
  SlidersHorizontal
} from 'lucide-vue-next'
import ContextMenu from '@/components/ContextMenu.vue'
import CommandSuggestBar from '@/components/CommandSuggestBar.vue'
import HighlightSettingsModal from '@/components/HighlightSettingsModal.vue'
import { dialogState, confirmDialog } from '@/composables/useDialog'
import { useTerminalHighlight, type TerminalHighlight } from '@/composables/useTerminalHighlight'
import { compileHighlightRules, highlightSettings } from '@/composables/useHighlightSettings'
import { settings, currentTerminalTheme, groupById, groupColorOf } from '@/composables/useAppSettings'
import { detectDangerousCommand } from '@/utils/commandSafety'
import type { CommandHistoryEntry, ContextMenuItem, TerminalTab } from '@/types'
import {
  writeSshSessionApi,
  resizeSshSessionApi,
  writeLocalSessionApi,
  resizeLocalSessionApi,
  onLocalStdout,
  onLocalClosed,
  takeLocalSessionBannerApi,
  setLocalSessionLoggingApi,
  getSshSessionCwdApi,
  setMockTerminalCwd,
  onSshStdout,
  onSshClosed,
  getCommandHistoryApi,
  pushCommandHistoryApi,
  importCommandHistoryApi,
  execSshCommandApi,
  takeSshSessionBannerApi,
  setSessionLoggingApi,
  getLogDirApi,
  revealInFileManagerApi,
  isTauri
} from '@/api/tauri'

const props = withDefaults(
  defineProps<{
    tabs: TerminalTab[]
    activeTabIndex: number
    sidebarCollapsed?: boolean
    /** 该终端所属的会话工作区是否为当前活动工作区（用于全局快捷键只在一个实例上生效） */
    active?: boolean
    /** 命令历史的主机键："user@host:port"，比 hostId 稳定，临时会话/重新导入资产后历史不丢 */
    hostKey?: string
    /** 所属分组 id：用于标签环境色与危险命令二次确认 */
    groupId?: string
    /** ssh 走原来的远端通道；local 走本机 PTY */
    transport?: 'ssh' | 'local'
  }>(),
  {
    sidebarCollapsed: false,
    active: false,
    hostKey: '',
    groupId: '',
    transport: 'ssh'
  }
)

const emit = defineEmits<{
  (e: 'switch-tab', index: number): void
  (e: 'close-tab', index: number): void
  (e: 'add-tab'): void
  (e: 'jump-to-sftp'): void
  (e: 'toggle-sidebar'): void
  (e: 'duplicate-tab', index: number): void
  (e: 'reconnect-tab', index: number): void
  (e: 'rename-tab', index: number): void
  (e: 'copy-tab-info', index: number, kind: 'ip' | 'login' | 'ssh-command'): void
  (e: 'open-tab-sftp', index: number): void
  (e: 'close-other-tabs', index: number): void
  (e: 'close-tabs-after', index: number): void
  (e: 'session-closed', tabId: string, reason: string): void
  (e: 'terminal-cwd-change', tabId: string, cwd: string): void
  (e: 'terminal-enter', tabId: string): void
  (e: 'toast', msg: string, type: 'info' | 'success' | 'warning' | 'error'): void
}>()

interface TerminalSession {
  term: Xterm
  fit: FitAddon
  search: SearchAddon
  highlight: TerminalHighlight
  container: HTMLDivElement
  inputBuffer: string
  /** 本地维护的当前输入行（仅用于命令历史/联想，绝不参与回显） */
  lineBuffer: string
  unlisten: Array<() => void>
  writeFailed: boolean
  /** 右键按下瞬间的选区快照（右键可能清空选区，需提前捕获） */
  rightClickSelection: string
  /** 远端 shell 是否已经自己打印过 Last login（用于避免客户端重复补打） */
  sawLastLogin: boolean
  /** 终端容器归属的标签 UI 标识（重连不换） */
  uiId: string
}

const containerMap = new Map<string, HTMLDivElement>()
const sessions = new Map<string, TerminalSession>()
/**
 * 容器按稳定的 `tab.uiId` 索引：重连只换后端会话 id，xterm 挂载的 DOM 不变，
 * 新旧会话可以复用同一个终端实例（历史回显不会丢）。
 */
const setContainer = (uiId: string, el: any) => {
  if (el) {
    containerMap.set(uiId, el as HTMLDivElement)
  } else {
    containerMap.delete(uiId)
  }
}
const termSize = ref<{ cols: number; rows: number }>({ cols: 80, rows: 24 })

// 关键字高亮配置是全局单例（useHighlightSettings），这里只读开关状态用于 UI
const highlightEnabled = computed(() => highlightSettings.enabled)
const highlightSettingsOpen = ref(false)


const currentTabId = computed(() => props.tabs[props.activeTabIndex]?.id ?? '')
const currentTab = computed(() => props.tabs[props.activeTabIndex])
const currentSession = computed(() => sessions.get(currentTabId.value))

/** 该标签是否是当前断线、需要展示重连入口的标签 */
const disconnectedTab = computed(() => {
  const tab = currentTab.value
  return tab && !tab.connected ? tab : null
})

// ==========================================
// 命令历史 / 联想补全
// ==========================================
/** 每主机历史缓存：候选计算只查内存，仅建连/提交/导入时才走后端 IPC */
const historyCache = new Map<string, CommandHistoryEntry[]>()
/** 已拉过远端 ~/.bash_history 的主机，避免每次开 Tab 重复拉取 */
const remoteHistoryBootstrapped = new Set<string>()
const REMOTE_HISTORY_KEY = 'snail_ssh_import_remote_history'

const suggestCandidates = ref<string[]>([])
const suggestIndex = ref(-1)
/** 用户用方向键或悬停明确选中后才允许 Enter 补全；仅打开候选不算选中，避免抢走正常回车执行 */
const suggestArmed = ref(false)
/** key：方向键选中，鼠标离开条不取消；hover：悬停选中，离开条后取消，避免手偶停在候选上把下一次 Enter 变成补全 */
const suggestPickSource = ref<'key' | 'hover' | ''>('')
const suggestTabId = ref('')

const suggestBarVisible = computed(() =>
  suggestTabId.value === currentTabId.value && suggestCandidates.value.length > 0
)

const isLocalTransport = computed(() => props.transport === 'local')

const hostKeyFor = (tab: TerminalTab): string => {
  if (isLocalTransport.value) return `local:${tab.shellKind || 'shell'}`
  return props.hostKey || tab.hostId
}

const writeTransport = (tabId: string, data: string) =>
  isLocalTransport.value ? writeLocalSessionApi(tabId, data) : writeSshSessionApi(tabId, data)

const resizeTransport = (tabId: string, cols: number, rows: number) =>
  isLocalTransport.value ? resizeLocalSessionApi(tabId, cols, rows) : resizeSshSessionApi(tabId, cols, rows)

const setLoggingTransport = (tabId: string, enabled: boolean, label: string) =>
  isLocalTransport.value
    ? setLocalSessionLoggingApi(tabId, enabled, label)
    : setSessionLoggingApi(tabId, enabled, label)

/** 候选条仅在可见时接管 ↑/↓ / Esc；Enter 只在已选中时补全，Tab 始终透传 */
const hasSuggestions = (tabId: string) => suggestTabId.value === tabId && suggestCandidates.value.length > 0

const resetSuggestionPick = () => {
  suggestIndex.value = -1
  suggestArmed.value = false
  suggestPickSource.value = ''
}

const closeSuggestions = () => {
  suggestCandidates.value = []
  resetSuggestionPick()
  suggestTabId.value = ''
}

/** 内置子命令词表（不做远端 --help 探测，避免拖慢输入） */
const SUBCOMMAND_WORDS: Record<string, string[]> = {
  docker: ['ps', 'ps -a', 'images', 'logs -f', 'exec -it', 'compose up -d', 'compose down', 'system prune -f'],
  systemctl: ['status', 'start', 'stop', 'restart', 'enable', 'disable', 'daemon-reload'],
  git: ['status', 'pull', 'push', 'log --oneline', 'diff', 'checkout', 'branch -a', 'commit -m "']
}

const staticCandidates = (input: string): string[] => {
  if (isLocalTransport.value) return []
  const matched = input.match(/^(\w+)\s+(\S*)$/)
  if (!matched) return []
  const words = SUBCOMMAND_WORDS[matched[1]]
  if (!words) return []
  const frag = matched[2].toLowerCase()
  return words
    .filter(w => w.toLowerCase().startsWith(frag) && `${matched[1]} ${w}` !== input.trim())
    .map(w => `${matched[1]} ${w}`)
}

const computeCandidates = (hostKey: string, input: string): string[] => {
  const needle = input.trim()
  if (needle.length < 2) return []
  const lowerNeedle = needle.toLowerCase()
  const prefix: string[] = []
  const substr: string[] = []
  for (const entry of historyCache.get(hostKey) || []) {
    if (entry.cmd === needle) continue
    const lower = entry.cmd.toLowerCase()
    if (lower.startsWith(lowerNeedle)) prefix.push(entry.cmd)
    else if (lower.includes(lowerNeedle)) substr.push(entry.cmd)
  }
  // 前缀匹配优先，其次子串匹配；内置动词放最前
  return Array.from(new Set([...staticCandidates(needle), ...prefix, ...substr])).slice(0, 8)
}

const updateSuggestions = (tab: TerminalTab, lineBuffer: string) => {
  if (!settings.historySuggestEnabled) {
    if (suggestTabId.value === tab.id) closeSuggestions()
    return
  }
  const candidates = computeCandidates(hostKeyFor(tab), lineBuffer)
  if (candidates.length === 0) {
    if (suggestTabId.value === tab.id) closeSuggestions()
    return
  }
  suggestCandidates.value = candidates
  // 输入变了就作废上一次选中：未选中时 Enter 仍执行当前输入
  resetSuggestionPick()
  suggestTabId.value = tab.id
}

const refreshHistory = async (hostKey: string) => {
  try {
    historyCache.set(hostKey, await getCommandHistoryApi(hostKey))
  } catch (e) {
    console.warn('加载命令历史失败:', e)
  }
}

const recordCommand = async (tab: TerminalTab, cmd: string) => {
  const key = hostKeyFor(tab)
  try {
    historyCache.set(key, await pushCommandHistoryApi(key, cmd))
  } catch {
    // 历史记录失败不影响终端使用
  }
}

/** 会话建立后拉取远端 ~/.bash_history 作为词库（localStorage 置 '0' 可关闭） */
const bootstrapRemoteHistory = async (tab: TerminalTab, sessionId: string) => {
  if (!isTauri()) return
  try {
    if (localStorage.getItem(REMOTE_HISTORY_KEY) === '0') return
  } catch {
    // localStorage 不可用时按默认开启处理
  }
  const key = hostKeyFor(tab)
  if (remoteHistoryBootstrapped.has(key)) return
  remoteHistoryBootstrapped.add(key)
  try {
    const raw = await execSshCommandApi(sessionId, 'cat ~/.bash_history 2>/dev/null | tail -n 200')
    const cmds = raw.split(/\r?\n/).map((l: string) => l.replace(/\u0000/g, '')).filter((l: string) => l.trim())
    if (cmds.length > 0) {
      await importCommandHistoryApi(key, cmds)
      await refreshHistory(key)
    }
  } catch {
    // 远端历史拉取失败不影响会话
  }
}

/** 用退格抹掉远端已回显的输入，再写入完整候选（不依赖远端 shell 的补全能力） */
const applySuggestion = (cmd: string) => {
  const tabId = suggestTabId.value
  const session = sessions.get(tabId)
  if (!session || !cmd) return
  writeToSession(tabId, '\x7f'.repeat(session.lineBuffer.length) + cmd)
  session.lineBuffer = cmd
  closeSuggestions()
}

const handleSuggestionAccept = (cmd: string) => applySuggestion(cmd)
const handleSuggestionHover = (index: number) => {
  suggestIndex.value = index
  suggestArmed.value = true
  // 方向键已选中时，悬停只改高亮，不把选中降级成「鼠标一离开就消失」
  if (suggestPickSource.value !== 'key') suggestPickSource.value = 'hover'
}
const handleSuggestionLeave = () => {
  if (suggestPickSource.value !== 'hover') return
  resetSuggestionPick()
}

const moveSuggestion = (delta: number) => {
  const total = suggestCandidates.value.length
  if (total === 0) return
  suggestPickSource.value = 'key'
  suggestArmed.value = true
  // 还没选中时，第一下方向键只选中端点，不要从默认 0 再跳一格
  if (suggestIndex.value < 0) {
    suggestIndex.value = delta > 0 ? 0 : total - 1
    return
  }
  suggestIndex.value = (suggestIndex.value + delta + total) % total
}

/** 终端控制序列（方向键/功能键/鼠标/bracketed paste/OSC 标题等）不参与输入行缓冲 */
const stripTerminalSequences = (input: string): string =>
  input
    .replace(/\x1b\][\s\S]*?(?:\x07|\x1b\\)/g, '')
    .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '')
    .replace(/\x1bO[0-9A-Za-z]/g, '')
    .replace(/\x1b[@-Z\\-_]/g, '')

const handleLineBufferInput = (tab: TerminalTab, session: TerminalSession, rawData: string) => {
  const data = stripTerminalSequences(rawData)
  if (!data) return
  let dirty = false
  for (const ch of data) {
    // 粘贴多行文本会一次进来一整段：必须逐行拆开，否则会记成一条巨串
    if (ch === '\r' || ch === '\n') {
      const cmd = session.lineBuffer
      session.lineBuffer = ''
      dirty = true
      if (cmd.trim()) void recordCommand(tab, cmd)
      continue
    }
    if (ch === '\x7f' || ch === '\b') {
      session.lineBuffer = session.lineBuffer.slice(0, -1)
      dirty = true
      continue
    }
    if (ch === '\x15' || ch === '\x03') {
      session.lineBuffer = ''
      dirty = true
      continue
    }
    if (ch >= ' ') {
      session.lineBuffer += ch
      dirty = true
    }
  }
  if (dirty) updateSuggestions(tab, session.lineBuffer)
}

// ==========================================
// 独立命令输入框（Compose Bar）
// ==========================================
/**
 * 设计约束：
 * 1. 不接管终端键盘——vim/htop/less 等全屏程序必须走 xterm，这里只是「写好再发」；
 * 2. 不自己重写历史库——直接复用命令历史（含频次排序、敏感词过滤）与联想词表；
 * 3. 发送前对高风险分组做危险命令确认，避免手滑把 rm -rf 发到生产。
 */
const commandBarOpen = ref(false)
const commandDraft = ref('')
const commandInputRef = ref<HTMLTextAreaElement | null>(null)
const draftHeight = ref(38)
const historyIndex = ref(-1)
/** 输入框打开时记录的历史快照，避免边输边刷候选导致闪烁 */
const commandHistory = ref<CommandHistoryEntry[]>([])

const draftLineCount = computed(() => commandDraft.value.split('\n').length)
const dangerousDraft = computed(() =>
  isLocalTransport.value ? null : detectDangerousCommand(commandDraft.value)
)
const currentGroup = computed(() => groupById(props.groupId))
const groupColorDot = computed(() =>
  isLocalTransport.value ? 'bg-sky-400' : groupColorOf(currentGroup.value.color).dot
)
const groupColorHex = computed(() =>
  isLocalTransport.value ? '#38bdf8' : groupColorOf(currentGroup.value.color).hex
)

/** 输入框自动增高（最多 160px，超过后内部滚动） */
const autoGrowDraft = () => {
  nextTick(() => {
    const el = commandInputRef.value
    if (!el) return
    el.style.height = 'auto'
    draftHeight.value = Math.min(160, Math.max(38, el.scrollHeight))
  })
}

const refreshCommandHistory = () => {
  const tab = currentTab.value
  if (!tab) {
    commandHistory.value = []
    return
  }
  commandHistory.value = (historyCache.get(hostKeyFor(tab)) || []).slice(0, 30)
}

const openCommandBar = () => {
  if (!currentTab.value) return
  commandBarOpen.value = true
  historyIndex.value = -1
  refreshCommandHistory()
  nextTick(() => {
    commandInputRef.value?.focus()
    autoGrowDraft()
  })
}

const closeCommandBar = () => {
  commandBarOpen.value = false
  commandDraft.value = ''
  historyIndex.value = -1
  draftHeight.value = 38
  nextTick(() => focusActive())
}

const pickHistoryCommand = (cmd: string) => {
  commandDraft.value = cmd
  historyIndex.value = -1
  nextTick(() => {
    autoGrowDraft()
    commandInputRef.value?.focus()
  })
}

const onCommandBarKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') {
    e.preventDefault()
    closeCommandBar()
    return
  }
  // Enter 发送，Shift+Enter 换行（与 IM 习惯一致）
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    void sendCommandDraft()
    return
  }
  // ↑/↓ 在草稿为空时浏览历史，否则保持光标移动，避免抢走多行编辑体验
  if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && commandHistory.value.length > 0) {
    if (commandDraft.value.trim() && !e.altKey) return
    e.preventDefault()
    const total = commandHistory.value.length
    // 从「无选中」开始：↑ 选最新一条，↓ 选最旧一条
    if (historyIndex.value < 0) {
      historyIndex.value = e.key === 'ArrowUp' ? 0 : total - 1
    } else {
      historyIndex.value = e.key === 'ArrowDown'
        ? (historyIndex.value + 1) % total
        : (historyIndex.value - 1 + total) % total
    }
    commandDraft.value = commandHistory.value[historyIndex.value].cmd
    nextTick(() => autoGrowDraft())
  }
}

/** 真正发送：危险命令先确认，成功后才清空草稿 */
const sendCommandDraft = async () => {
  const tab = currentTab.value
  const text = commandDraft.value
  if (!tab || !text.trim()) return
  if (rejectIfDisconnected(tab)) return

  const danger = isLocalTransport.value ? null : detectDangerousCommand(text)
  if (danger && currentGroup.value.danger) {
    const ok = await confirmDialog({
      title: '高风险环境：确认发送危险命令',
      variant: 'danger',
      message: `${currentGroup.value.label} 下即将执行危险命令（${danger.label}）`,
      detail: `主机：${tab.sub || tab.title}\n\n${danger.snippet}\n\n确认后将直接发送到远端终端，无法撤销。`,
      confirmText: '确认发送',
      cancelText: '取消'
    })
    if (!ok) return
  }

  // 多行草稿按原样发送（含换行），最后补一个回车确保末行执行
  const payload = text.endsWith('\n') ? text : text + '\n'
  writeToSession(tab.id, payload)
  void recordCommand(tab, text.trim())
  commandDraft.value = ''
  historyIndex.value = -1
  draftHeight.value = 38
  refreshCommandHistory()
  nextTick(() => {
    autoGrowDraft()
    commandInputRef.value?.focus()
  })
}

// ==========================================
// 会话日志（REC 指示 + 开/关 + 打开目录）
// ==========================================
const LOG_AUTO_KEY = 'snail_ssh_session_log_auto'
const loggingPaths = ref<Record<string, string>>({})
const currentLogPath = computed(() => loggingPaths.value[currentTabId.value] || '')
const currentLogging = computed(() => !!loggingPaths.value[currentTabId.value])
const isTabLogging = (tabId: string) => !!loggingPaths.value[tabId]

const readLogAuto = (): boolean => {
  try {
    return localStorage.getItem(LOG_AUTO_KEY) === '1'
  } catch {
    return false
  }
}
const writeLogAuto = (on: boolean) => {
  try {
    localStorage.setItem(LOG_AUTO_KEY, on ? '1' : '0')
  } catch {
    // localStorage 不可用时不影响本次会话
  }
}

const setTabLogPath = (tabId: string, path: string | null) => {
  const next = { ...loggingPaths.value }
  if (path) next[tabId] = path
  else delete next[tabId]
  loggingPaths.value = next
}

const logHostLabel = (tab: TerminalTab) => props.hostKey || tab.sub || tab.title

const enableSessionLog = async (tab: TerminalTab): Promise<string | null> => {
  const path = await setLoggingTransport(tab.id, true, logHostLabel(tab))
  setTabLogPath(tab.id, path)
  return path
}

const stopSessionLog = async (tab: TerminalTab): Promise<string | null> => {
  const path = await setLoggingTransport(tab.id, false, logHostLabel(tab))
  setTabLogPath(tab.id, null)
  return path
}

const toggleSessionLog = async (tabId: string) => {
  const tab = props.tabs.find(t => t.id === tabId)
  if (!tab) return
  if (!isTauri()) {
    emitToast('浏览器预览模式不支持会话日志', 'warning')
    return
  }
  if (isTabLogging(tabId)) {
    try {
      const path = await stopSessionLog(tab)
      writeLogAuto(false)
      emitToast(`已停止记录会话日志（凭据已脱敏，可能与屏幕内容不一致）: ${path || ''}`, 'info')
    } catch (e: any) {
      emitToast(`停止会话日志失败: ${e?.message || e}`, 'error')
    }
  } else {
    try {
      const path = await enableSessionLog(tab)
      writeLogAuto(true)
      emitToast(`已开始记录会话日志（凭据已脱敏，内容可能与屏幕不一致）: ${path || ''}`, 'success')
    } catch (e: any) {
      emitToast(`开始会话日志失败: ${e?.message || e}`, 'error')
    }
  }
}

const openLogDir = async () => {
  if (!isTauri()) {
    emitToast('浏览器预览模式不支持打开日志目录', 'warning')
    return
  }
  try {
    const dir = await getLogDirApi()
    if (!dir) {
      emitToast('日志目录不可用', 'warning')
      return
    }
    await revealInFileManagerApi(dir)
  } catch (e: any) {
    emitToast(`打开日志目录失败: ${e?.message || e}`, 'error')
  }
}

const createSession = (tab: TerminalTab, container: HTMLDivElement): TerminalSession => {
  const term = new Xterm({
    fontFamily: settings.terminal.fontFamily,
    fontSize: settings.terminal.fontSize,
    lineHeight: settings.terminal.lineHeight,
    cursorBlink: settings.terminal.cursorBlink,
    cursorStyle: settings.terminal.cursorStyle,
    theme: currentTerminalTheme().theme,
    allowProposedApi: true,
    // 右键留给「粘贴 / 菜单」，不要顺手选中光标下的单词，否则会盖掉剪贴板
    rightClickSelectsWord: false,
  })

  const fit = new FitAddon()
  const search = new SearchAddon()
  term.loadAddon(fit)
  term.loadAddon(search)
  // 中日韩宽字符宽度修正（中文环境收益明显）
  term.loadAddon(new Unicode11Addon())
  term.unicode.activeVersion = '11'
  term.loadAddon(new WebLinksAddon())

  const session: TerminalSession = {
    term, fit, search,
    highlight: useTerminalHighlight(term, {
      rules: compileHighlightRules(highlightSettings),
      onToast: (msg, type) => emit('toast', msg, type ?? 'info')
    }),
    container, inputBuffer: '', lineBuffer: '', unlisten: [], writeFailed: false, rightClickSelection: '',
    sawLastLogin: false,
    uiId: tab.uiId
  }

  term.open(container)
  term.options.overviewRulerWidth = highlightSettings.overviewRuler ? 6 : 0
  session.highlight.setEnabled(highlightEnabled.value)
  // 非当前工作区的终端虽然还挂在 DOM 上（v-show 隐藏），但看不见：建会话时就先停扫，
  // 避免后台会话每次输出都跑正则、建 marker
  session.highlight.setActive(props.active)

  // 搜索结果计数（仅在开启 search decorations 时触发）
  search.onDidChangeResults((result) => {
    if (tab.id === currentTabId.value) searchResult.value = { resultIndex: result.resultIndex, resultCount: result.resultCount }
  })

  // 右键会先触发 mousedown，此处用捕获阶段提前保存选区
  container.addEventListener('mousedown', (e) => {
    if (e.button === 2) session.rightClickSelection = term.getSelection()
  }, true)

  // 选中即复制：拖动中不写剪贴板，只在选区定稿后写一次，避免中间态反复覆盖剪贴板
  let dragging = false
  let lastCopied = ''
  const copySelectionToClipboard = () => {
    if (!settings.selectToCopy) return
    const text = term.getSelection()
    if (!text || text === lastCopied) return
    lastCopied = text
    // 静默失败：自动复制不该打断用户操作，也不弹 toast
    navigator.clipboard.writeText(text).catch(() => {})
  }
  container.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return
    dragging = true
    lastCopied = ''
  })
  container.addEventListener('mouseup', (e) => {
    if (e.button !== 0) return
    dragging = false
    copySelectionToClipboard()
  })
  // 拖到终端外松开时容器收不到 mouseup，靠 document 兜底（xterm 也在 document 上收尾选择）
  const onDocumentMouseUp = (e: MouseEvent) => {
    if (!dragging) return
    dragging = false
    // 右键松开不复制，避免和右键粘贴抢剪贴板
    if (e.button !== 0) return
    copySelectionToClipboard()
  }
  document.addEventListener('mouseup', onDocumentMouseUp)
  // 双击选词 / 三击选行 / 全选不经过拖动，事件触发时选区已是最终结果
  term.onSelectionChange(() => {
    if (!dragging) copySelectionToClipboard()
  })
  session.unlisten.push(() => document.removeEventListener('mouseup', onDocumentMouseUp))

  // 监听 OSC 7 (标准终端当前目录转义序列)
  term.parser.registerOscHandler(7, (data) => {
    if (props.transport === 'local') return true
    const p = parseOsc7Path(data)
    if (p) {
      tab.cwd = p
      emit('terminal-cwd-change', tab.id, p)
    }
    return true
  })

  // 监听 OSC 1337 (iTerm2 / Shell Integration 扩展目录格式)
  term.parser.registerOscHandler(1337, (data) => {
    if (props.transport === 'local') return true
    const p = parseOsc1337Path(data)
    if (p) {
      tab.cwd = p
      emit('terminal-cwd-change', tab.id, p)
    }
    return true
  })

  // 绑定用户输入
  term.onData((data) => {
    // 断开的会话已经没人收数据了，不再把按键往里塞（重连后 tab.connected 会重新置 true）
    if (!tab.connected) return
    if (data.includes('\r')) {
      emit('terminal-enter', tab.id)
    }
    handleLineBufferInput(tab, session, data)
    writeToSession(tab.id, data)
  })

  // 接管终端内快捷键：剪贴板与查找（返回 false 阻止 xterm 继续处理）
  term.attachCustomKeyEventHandler((e) => {
    if (e.type !== 'keydown') return true
    const mod = e.ctrlKey || e.metaKey
    const key = e.key

    // 复制：Ctrl+Shift+C / Ctrl+Insert
    if (mod && ((e.shiftKey && key.toLowerCase() === 'c') || key === 'Insert')) {
      e.preventDefault()
      copySelection(tab.id)
      return false
    }
    // 粘贴：Ctrl+Shift+V / Shift+Insert
    if ((mod && e.shiftKey && key.toLowerCase() === 'v') || (e.shiftKey && key === 'Insert')) {
      e.preventDefault()
      pasteToTerminal(tab.id)
      return false
    }
    // 终端内查找：Ctrl+F
    if (mod && !e.shiftKey && key.toLowerCase() === 'f') {
      e.preventDefault()
      if (tab.id === currentTabId.value) openSearch()
      return false
    }

    // 历史补全不接管 Tab，留给远端 shell 自身补全。
    // 候选可见时才截获 ↑/↓ / Esc；Enter 只在用户选中某一条后写入命令行（不直接执行）。
    // 没有候选时一概透传——绝不接管裸 ↑/↓，否则 vim/less/man 全部失效
    if (hasSuggestions(tab.id)) {
      if (
        key === 'Enter'
        && suggestArmed.value
        && suggestIndex.value >= 0
        && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey
        && !e.isComposing
      ) {
        e.preventDefault()
        applySuggestion(suggestCandidates.value[suggestIndex.value])
        return false
      }
      if (key === 'Escape') {
        e.preventDefault()
        closeSuggestions()
        return false
      }
      if ((key === 'ArrowUp' || key === 'ArrowDown') && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
        e.preventDefault()
        moveSuggestion(key === 'ArrowDown' ? 1 : -1)
        return false
      }
    }
    return true
  })

  // 窗口尺寸变化时同步给远端 PTY
  term.onResize(({ cols, rows }) => {
    if (tab.id !== currentTabId.value) return
    termSize.value = { cols, rows }
    if (isTauri()) {
      resizeTransport(tab.id, cols, rows).catch(() => {})
    }
  })

  // 真实会话不打印假欢迎信息。浏览器里的本地终端只说明不会启动进程，不模拟 shell。
  if (!isTauri()) {
    if (props.transport === 'local') {
      term.write('\r\n\x1b[33m预览模式不会启动本机 PowerShell\x1b[0m\r\n')
    } else {
      printMockBanner(term)
    }
  }

  return session
}

/** sshd 自己打印的“上次登录”行（不同发行版 PAM 文案不同） */
const LAST_LOGIN_RE = /last login|上次登录|最后一次登录/i
/** 首屏判定后额外观察的窗口，兜住晚到的 sshd 输出，避免重复补打 */
const LAST_LOGIN_WATCH_MS = 300

/**
 * 把 `LC_ALL=C lastlog -u <user>` 的输出还原成 sshd 风格的 Last login 行。
 * 示例：`root  pts/0  192.168.1.5  Fri Oct 16 16:25:59 +0800 2026`
 * 解析不出来（无记录 / Never logged in / 命令不存在）返回 null。
 */
const formatRemoteLastLogin = (raw: string): string | null => {
  for (const line of raw.split(/\r?\n/)) {
    const text = line.trim()
    if (!text || /^username\b/i.test(text) || /never logged in/i.test(text)) continue
    const matched = text.match(
      /^(\S+)\s+([\s\S]*?)([A-Z][a-z]{2}\s+[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})(?:\s+[+-]\d{4})?\s+(\d{4})/
    )
    if (!matched) continue
    const time = `${matched[3]} ${matched[4]}`.replace(/\s+/g, ' ')
    // 时间之前依次是 user / 端口 / 来源地址；本地登录没有来源，端口列也可能为空
    const from = matched[2]
      .trim()
      .split(/\s+/)
      .filter(p => p && !/^(pts\/|tty)/.test(p))
      .pop() || ''
    return `Last login: ${time}${from ? ` from ${from}` : ''}`
  }
  return null
}

/**
 * 远端 sshd 默认会自己打印 Last login；只有它没打印时才由客户端补一行——
 * 数据取自远端 lastlog（真实记录），取不到就什么都不显示，绝不伪造。
 */
const announceLastLoginIfMissing = async (tab: TerminalTab, session: TerminalSession, banner: string) => {
  if (LAST_LOGIN_RE.test(banner)) return
  await new Promise(resolve => setTimeout(resolve, LAST_LOGIN_WATCH_MS))
  // 期间已看到远端输出，或用户已经开始输入（补打会插进命令行中间）则放弃
  if (session.sawLastLogin || session.lineBuffer.length > 0) return
  try {
    const raw = await execSshCommandApi(tab.id, 'LC_ALL=C lastlog -u "$(id -un)" 2>/dev/null')
    const line = formatRemoteLastLogin(raw)
    if (!line || sessions.get(tab.id) !== session || session.sawLastLogin) return
    session.term.write(`\x1b[90m${line}\x1b[0m\r\n`)
  } catch {
    // 远端没有 lastlog / 会话已关闭：不显示即可，不陪一个假时间
  }
}

const bindLocalSessionEvents = async (tab: TerminalTab, session: TerminalSession) => {
  if (!isTauri()) {
    const hostKey = hostKeyFor(tab)
    if (!historyCache.has(hostKey)) await refreshHistory(hostKey)
    return
  }
  const boundId = tab.id
  const pendingChunks: string[] = []
  let live = false
  const unlistenStdout = await onLocalStdout(boundId, (chunk) => {
    if (live) {
      session.term.write(chunk)
      return
    }
    pendingChunks.push(chunk)
  })
  const unlistenClosed = await onLocalClosed(boundId, (reason) => {
    session.term.write(`\r\n\x1b[33m[会话结束] ${reason}\x1b[0m\r\n`)
    setTabLogPath(boundId, null)
    emit('session-closed', boundId, reason)
  })
  session.unlisten.push(unlistenStdout, unlistenClosed)

  let banner = ''
  try {
    banner = await takeLocalSessionBannerApi(tab.id)
  } catch (e) {
    console.warn('取回本地终端首屏失败:', e)
  }
  if (banner) session.term.write(banner)
  live = true
  for (const chunk of pendingChunks) session.term.write(chunk)
  pendingChunks.length = 0

  const hostKey = hostKeyFor(tab)
  if (!historyCache.has(hostKey)) await refreshHistory(hostKey)
  if (readLogAuto() && !isTabLogging(tab.id)) {
    enableSessionLog(tab).catch(e => console.warn('自动开启会话日志失败:', e))
  }
}

const bindSessionEvents = async (tab: TerminalTab, session: TerminalSession) => {
  if (props.transport === 'local') {
    await bindLocalSessionEvents(tab, session)
    return
  }
  if (isTauri()) {
    // 绑定时的后端会话 id：重连后 tab.id 会换，旧会话迟到的 closed 事件
    // 不能再用 tab.id，否则会把刚重连好的标签误标为断开
    const boundId = tab.id
    // 建连时的首屏（Last login / MOTD / 首个提示符）已被后端扣下，否则会在前端注册监听前丢失。
    // 顺序必须是「先注册监听 → 再取回首屏 → 最后放行实时输出」，否则提示符会跑到 MOTD 前面
    const pendingChunks: string[] = []
    let live = false
    const unlistenStdout = await onSshStdout(boundId, (chunk) => {
      if (LAST_LOGIN_RE.test(chunk)) session.sawLastLogin = true
      if (live) {
        session.term.write(chunk)
        return
      }
      pendingChunks.push(chunk)
    })
    const unlistenClosed = await onSshClosed(boundId, (reason) => {
      session.term.write(`\r\n\x1b[33m[会话结束] ${reason}\x1b[0m\r\n`)
      setTabLogPath(boundId, null)
      emit('session-closed', boundId, reason)
    })
    session.unlisten.push(unlistenStdout, unlistenClosed)

    let banner = ''
    try {
      banner = await takeSshSessionBannerApi(tab.id)
    } catch (e) {
      console.warn('取回会话首屏失败:', e)
    }
    if (LAST_LOGIN_RE.test(banner)) session.sawLastLogin = true
    if (banner) session.term.write(banner)
    live = true
    for (const chunk of pendingChunks) session.term.write(chunk)
    pendingChunks.length = 0

    void announceLastLoginIfMissing(tab, session, banner)
  }

  // 预载历史（浏览器预览走 localStorage 降级），真实会话再补齐远端 bash_history
  const hostKey = hostKeyFor(tab)
  if (!historyCache.has(hostKey)) await refreshHistory(hostKey)
  void bootstrapRemoteHistory(tab, tab.id)

  // 记忆上次的日志开关：开启过则新会话自动开始记录
  if (isTauri() && readLogAuto() && !isTabLogging(tab.id)) {
    enableSessionLog(tab).catch(e => console.warn('自动开启会话日志失败:', e))
  }
}

/** 按标签页同步 xterm 实例：新增则创建，移除则销毁；断线重连则原地迁移 */
const syncSessions = async () => {
  await nextTick()

  // 重连只换了后端会话 id，uiId 没变：先把旧键下的 session 迁到新键，
  // 这样 xterm 不会被销毁重建，断开前的输出还能看到
  const keyByUiId = new Map<string, string>()
  for (const [key, session] of sessions) keyByUiId.set(session.uiId, key)
  for (const tab of props.tabs) {
    const oldKey = keyByUiId.get(tab.uiId)
    if (!oldKey || oldKey === tab.id) continue
    const session = sessions.get(oldKey)
    if (!session) continue
    sessions.delete(oldKey)
    sessions.set(tab.id, session)
    // 旧后端会话已经关了：先摘监听，再绑到新会话上
    session.unlisten.forEach(fn => fn())
    session.unlisten = []
    session.sawLastLogin = false
    if (suggestTabId.value === oldKey) closeSuggestions()
    // 旧会话的日志文件已经封存，不能把旧路径当成新会话的 REC 状态；
    // 若用户开启过自动记录，下面的 bindSessionEvents 会为新会话重新开一个文件
    if (loggingPaths.value[oldKey]) setTabLogPath(oldKey, null)
    session.term.write(
      props.transport === 'local'
        ? '\r\n\x1b[32m[已重新打开] 新的本地终端已启动\x1b[0m\r\n'
        : '\r\n\x1b[32m[已重新连接] 新的 SSH 会话已建立\x1b[0m\r\n'
    )
    await bindSessionEvents(tab, session)
    // 新会话的 PTY 尺寸可能不等于当前终端：主动同步一次，否则宽字符会折行错位
    if (isTauri()) {
      resizeTransport(tab.id, session.term.cols, session.term.rows).catch(() => {})
    }
    if (tab.id === currentTabId.value) session.term.focus()
  }

  for (const [tabId, session] of sessions) {
    if (props.tabs.some(t => t.id === tabId)) continue
    session.unlisten.forEach(fn => fn())
    session.highlight.dispose()
    session.term.dispose()
    sessions.delete(tabId)
    if (suggestTabId.value === tabId) closeSuggestions()
    if (loggingPaths.value[tabId]) setTabLogPath(tabId, null)
  }

  for (const tab of props.tabs) {
    if (sessions.has(tab.id)) continue
    const container = containerMap.get(tab.uiId)
    if (!container) continue
    const session = createSession(tab, container)
    sessions.set(tab.id, session)
    // 可见元素才能量出真实行列数，隐藏的标签页等切换时再 fit
    if (container.offsetParent) {
      session.fit.fit()
      termSize.value = { cols: session.term.cols, rows: session.term.rows }
    }
    await bindSessionEvents(tab, session)
  }

  fitActive()
}

const fitActive = () => {
  const session = sessions.get(currentTabId.value)
  if (!session || !session.container.offsetParent) return
  session.fit.fit()
  termSize.value = { cols: session.term.cols, rows: session.term.rows }
}

const focusActive = () => {
  const session = sessions.get(currentTabId.value)
  if (!session || !session.container.offsetParent) return
  session.term.focus()
}

const clearCurrentTerminal = () => clearTerminal(currentTabId.value)

/** 按标签页清屏（右键菜单可作用于非活动标签） */
const clearTerminal = (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  if (isTauri()) {
    // 交给 shell 执行清屏，保留当前提示符。Ctrl+L 本身仍透传，不在这里拦截。
    writeTransport(tabId, '\x0c').catch(() => {})
  } else if (props.transport !== 'local') {
    session.term.clear()
  }
}

// ==========================================
// 终端内搜索 / 剪贴板 / 右键菜单
// ==========================================
const searchOpen = ref(false)
const searchQuery = ref('')
const searchCaseSensitive = ref(false)
const searchWholeWord = ref(false)
const searchRegex = ref(false)
const searchResult = ref<{ resultIndex: number; resultCount: number }>({ resultIndex: -1, resultCount: 0 })
const searchInputRef = ref<HTMLInputElement | null>(null)

const emitToast = (msg: string, type: 'info' | 'success' | 'warning' | 'error' = 'info') => emit('toast', msg, type)

const searchResultText = computed(() => {
  if (!searchQuery.value) return ''
  const { resultIndex, resultCount } = searchResult.value
  if (resultCount <= 0) return '无结果'
  return `${resultIndex + 1}/${resultCount}`
})

const buildSearchOptions = () => ({
  regex: searchRegex.value,
  wholeWord: searchWholeWord.value,
  caseSensitive: searchCaseSensitive.value,
  incremental: true,
  decorations: {
    matchBackground: '#14532d',
    matchOverviewRuler: '#22c55e',
    activeMatchBackground: '#b45309',
    activeMatchColorOverviewRuler: '#fbbf24'
  }
})

const runSearch = (direction: 'next' | 'prev') => {
  const session = currentSession.value
  if (!session) return
  const q = searchQuery.value
  if (!q) {
    session.search.clearDecorations()
    searchResult.value = { resultIndex: -1, resultCount: 0 }
    return
  }
  const options = buildSearchOptions()
  if (direction === 'next') session.search.findNext(q, options)
  else session.search.findPrevious(q, options)
}

const onSearchEnter = (e: KeyboardEvent) => {
  runSearch(e.shiftKey ? 'prev' : 'next')
}

const openSearch = () => {
  const session = currentSession.value
  if (!session) return
  // 搜索条打开时不应再弹出/接受命令候选
  closeSuggestions()
  searchOpen.value = true
  nextTick(() => {
    // 搜索输入框独立于 xterm，不抢终端按键
    session.term.blur()
    searchInputRef.value?.focus()
    searchInputRef.value?.select()
    if (searchQuery.value) runSearch('next')
  })
}

const closeSearch = () => {
  if (!searchOpen.value) return
  searchOpen.value = false
  currentSession.value?.search.clearDecorations()
  searchResult.value = { resultIndex: -1, resultCount: 0 }
  nextTick(() => focusActive())
}

const toggleSearch = () => (searchOpen.value ? closeSearch() : openSearch())

/** 关键字高亮开关：存全局配置，由 watch 同步到所有终端实例 */
const toggleHighlight = () => {
  highlightSettings.enabled = !highlightSettings.enabled
  emitToast(highlightSettings.enabled ? '关键字高亮已开启' : '关键字高亮已关闭', 'info')
}

/** 把最新规则应用到本视图的所有终端（设置变更后防抖调用） */
const applyHighlightRules = () => {
  const rules = compileHighlightRules(highlightSettings)
  const rulerWidth = highlightSettings.overviewRuler ? 6 : 0
  for (const session of sessions.values()) {
    if (session.term.options.overviewRulerWidth !== rulerWidth) {
      session.term.options.overviewRulerWidth = rulerWidth
    }
    session.highlight.setRules(rules)
  }
}

/** 上一条 / 下一条错误行：以当前视口顶部为锚点，越过边界时循环 */
const jumpToProblem = (direction: 1 | -1) => {
  const session = currentSession.value
  if (!session) return
  if (!highlightSettings.enabled) {
    emitToast('请先开启关键字高亮', 'info')
    return
  }

  const lines = session.highlight.getProblemLines()
  if (lines.length === 0) {
    emitToast('当前缓冲区没有错误行', 'info')
    return
  }

  const anchor = session.term.buffer.active.viewportY
  let target: number | undefined
  if (direction > 0) {
    target = lines.find((line) => line > anchor)
  } else {
    for (const line of lines) if (line < anchor) target = line
  }
  if (target === undefined) {
    target = direction > 0 ? lines[0] : lines[lines.length - 1]
    emitToast(direction > 0 ? '已从头开始查找错误' : '已从末尾开始查找错误', 'info')
  }

  session.term.scrollToLine(target)
  session.term.focus()
}

/** 选中即复制：与设置中心同一开关，拖动/双击选中后自动写入剪贴板（不弹提示） */
const toggleSelectCopy = () => {
  settings.selectToCopy = !settings.selectToCopy
  emitToast(settings.selectToCopy ? '选中即复制已开启' : '选中即复制已关闭', 'info')
}

watch([searchQuery, searchCaseSensitive, searchWholeWord, searchRegex], () => {
  if (searchOpen.value) runSearch('next')
})

/** 断开的会话已经没人接数据：入口处先拦一道，提示用户重新连接 */
const rejectIfDisconnected = (tab: TerminalTab | undefined): boolean => {
  if (!tab || tab.connected) return false
  emitToast(
    isLocalTransport.value ? '终端已退出，请先点「重新打开」' : '连接已断开，请先点「重新连接」',
    'warning'
  )
  return true
}

/** 统一的会话写入入口（真实 SSH 与预览降级共用） */
const writeToSession = (tabId: string, data: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  if (isTauri()) {
    writeTransport(tabId, data).catch((err: any) => {
      if (session.writeFailed) return
      session.writeFailed = true
      session.term.write(`\r\n\x1b[31m[写入失败] ${err?.message || err}\x1b[0m\r\n`)
    })
  } else if (props.transport === 'local') {
    // 预览模式不模拟本机 shell，避免再维护一套假 PowerShell
    return
  } else {
    const tab = props.tabs.find(t => t.id === tabId)
    if (tab) handleMockTerminalInput(tab, session, data)
  }
}

const copySelection = async (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  const text = session.term.getSelection()
  if (!text) {
    emitToast('当前没有选中的文本', 'warning')
    return
  }
  try {
    await navigator.clipboard.writeText(text)
    emitToast('已复制选中内容', 'success')
  } catch (e: any) {
    emitToast(`复制失败: ${e?.message || e}`, 'error')
  }
}

const pasteToTerminal = async (tabId: string) => {
  const tab = props.tabs.find(t => t.id === tabId)
  if (rejectIfDisconnected(tab)) return
  const session = sessions.get(tabId)
  if (!session) return
  try {
    const text = await navigator.clipboard.readText()
    if (!text) {
      emitToast('剪贴板为空', 'warning')
      return
    }
    // xterm.paste 会把换行规范成 CR，并在远端开启 bracketed paste 时包上边界，再经 onData 写到会话
    session.term.paste(text)
    session.term.clearSelection()
    session.term.focus()
  } catch {
    emitToast('无法读取剪贴板，请使用 Ctrl+Shift+V 粘贴', 'warning')
  }
}

const selectAllInTerminal = (tabId: string) => {
  const session = sessions.get(tabId)
  if (!session) return
  session.term.selectAll()
  emitToast('已全选终端内容', 'info')
}

const termMenu = ref({ open: false, x: 0, y: 0, tabId: '', hasSelection: false })

const openTermMenu = (event: MouseEvent) => {
  const session = currentSession.value
  if (!session) return
  const selection = session.term.getSelection() || session.rightClickSelection
  termMenu.value = { open: true, x: event.clientX, y: event.clientY, tabId: currentTabId.value, hasSelection: !!selection }
}

/** 右键粘贴开启时直接粘贴；Shift / Ctrl 右键仍打开完整菜单 */
const onTerminalContextMenu = (event: MouseEvent) => {
  if (!currentSession.value) return
  const pasteDirect = settings.pasteOnRightClick && !event.shiftKey && !event.ctrlKey && !event.metaKey
  if (!pasteDirect) {
    openTermMenu(event)
    return
  }
  const tab = currentTab.value
  if (!tab) return
  void pasteToTerminal(tab.id)
}

const termMenuItems = computed<ContextMenuItem[]>(() => [
  { key: 'copy', label: '复制', icon: Copy, hint: 'Ctrl+Shift+C', disabled: !termMenu.value.hasSelection },
  { key: 'paste', label: '粘贴', icon: ClipboardPaste, hint: settings.pasteOnRightClick ? '右键' : 'Ctrl+Shift+V' },
  { key: 'select-all', label: '全选', icon: TextSelect },
  { divider: true },
  { key: 'toggle-highlight', label: highlightEnabled.value ? '关闭语法高亮' : '开启语法高亮', icon: Highlighter },
  { key: 'highlight-settings', label: '语法高亮设置...', icon: SlidersHorizontal },
  { divider: true },
  { key: 'find', label: '查找', icon: Search, hint: 'Ctrl+F' },
  { key: 'clear', label: '清除屏幕', icon: Eraser }
])

const handleTermMenuSelect = (item: ContextMenuItem) => {
  const tabId = termMenu.value.tabId
  if (!tabId) return
  switch (item.key) {
    case 'copy': {
      const session = sessions.get(tabId)
      const text = session?.term.getSelection() || session?.rightClickSelection || ''
      if (!text) {
        emitToast('当前没有选中的文本', 'warning')
        break
      }
      navigator.clipboard.writeText(text)
        .then(() => emitToast('已复制选中内容', 'success'))
        .catch((e: any) => emitToast(`复制失败: ${e?.message || e}`, 'error'))
      break
    }
    case 'paste': pasteToTerminal(tabId); break
    case 'select-all': selectAllInTerminal(tabId); break
    case 'toggle-highlight': toggleHighlight(); break
    case 'highlight-settings': highlightSettingsOpen.value = true; break
    case 'find':
      if (tabId === currentTabId.value) openSearch()
      else emitToast('请先切换到该终端再查找', 'info')
      break
    case 'clear': clearTerminal(tabId); break
  }
}

// ==========================================
// 标签页右键菜单
// ==========================================
const tabMenu = ref({ open: false, x: 0, y: 0, index: -1 })

const openTabMenu = (event: MouseEvent, index: number) => {
  tabMenu.value = { open: true, x: event.clientX, y: event.clientY, index }
}

const tabMenuItems = computed<ContextMenuItem[]>(() => {
  const index = tabMenu.value.index
  const tab = props.tabs[index]
  if (!tab) return []
  const isLast = index === props.tabs.length - 1
  const local = props.transport === 'local'
  const items: ContextMenuItem[] = [
    { key: 'duplicate', label: '复制会话', icon: CopyPlus, hint: '新标签页' },
    { divider: true },
    { key: 'copy-ip', label: '复制 IP 地址', icon: Globe },
    { key: 'copy-login', label: '复制登录信息', icon: User, hint: 'user@host' },
    { key: 'copy-ssh', label: '复制 SSH 命令', icon: Terminal, hint: 'ssh -p' },
    { divider: true },
    { key: 'rename', label: '重命名标签', icon: PenLine },
    { key: 'reconnect', label: local ? '重新打开' : '重新连接', icon: RefreshCw, disabled: tab.connected },
    { divider: true },
    { key: 'toggle-log', label: isTabLogging(tab.id) ? '停止记录会话日志' : '开始记录会话日志', icon: isTabLogging(tab.id) ? CircleStop : CircleDot },
    { key: 'open-log-dir', label: '打开日志目录', icon: FolderOpen },
    { key: 'sftp', label: '打开 SFTP', icon: FolderGit2 },
    { key: 'clear', label: '清除屏幕', icon: Eraser },
    { divider: true },
    { key: 'close', label: '关闭标签', icon: X, danger: true },
    { key: 'close-others', label: '关闭其他标签', icon: XCircle, disabled: props.tabs.length < 2 },
    { key: 'close-after', label: '关闭右侧标签', icon: ChevronsRight, disabled: isLast }
  ]
  if (!local) return items
  const hidden = new Set(['copy-ip', 'copy-login', 'copy-ssh', 'sftp'])
  const visible: ContextMenuItem[] = []
  for (const item of items) {
    if (item.key && hidden.has(item.key)) continue
    if (item.divider && (visible.length === 0 || visible[visible.length - 1]?.divider)) continue
    visible.push(item)
  }
  while (visible.length > 0 && visible[visible.length - 1]?.divider) visible.pop()
  return visible
})

const handleTabMenuSelect = (item: ContextMenuItem) => {
  const index = tabMenu.value.index
  const tab = props.tabs[index]
  if (!tab) return
  switch (item.key) {
    case 'duplicate': emit('duplicate-tab', index); break
    case 'copy-ip': emit('copy-tab-info', index, 'ip'); break
    case 'copy-login': emit('copy-tab-info', index, 'login'); break
    case 'copy-ssh': emit('copy-tab-info', index, 'ssh-command'); break
    case 'rename': emit('rename-tab', index); break
    case 'reconnect': emit('reconnect-tab', index); break
    case 'toggle-log': void toggleSessionLog(tab.id); break
    case 'open-log-dir': void openLogDir(); break
    case 'sftp': emit('open-tab-sftp', index); break
    case 'clear': clearTerminal(tab.id); break
    case 'close': emit('close-tab', index); break
    case 'close-others': emit('close-other-tabs', index); break
    case 'close-after': emit('close-tabs-after', index); break
  }
}

const sendSnippetCommand = (cmd: string) => {
  const session = currentSession.value
  const tab = currentTab.value
  if (!session || !tab) return
  if (rejectIfDisconnected(tab)) return
  if (isTauri()) {
    writeTransport(tab.id, cmd).catch((err: any) => {
      session.term.write(`\r\n\x1b[31m[写入失败] ${err?.message || err}\x1b[0m\r\n`)
    })
  } else {
    session.term.write(cmd)
    handleMockTerminalInput(tab, session, '\r')
  }
}

const parseOsc7Path = (data: string): string | null => {
  try {
    if (data.startsWith('file://')) {
      const url = new URL(data)
      return decodeURIComponent(url.pathname)
    }
    if (data.startsWith('/')) {
      return data.trim()
    }
  } catch {
    const stripped = data.replace(/^file:\/\/[^/]*/, '')
    if (stripped.startsWith('/')) {
      return decodeURIComponent(stripped)
    }
  }
  return null
}

const parseOsc1337Path = (data: string): string | null => {
  if (data.startsWith('CurrentDir=')) {
    const p = data.substring('CurrentDir='.length).trim()
    if (p.startsWith('/')) return p
  }
  return null
}

const getCurrentCwd = async (): Promise<string> => {
  const tab = currentTab.value
  if (!tab) return '/'
  if (props.transport === 'local') return tab.cwd || ''
  if (isTauri()) {
    try {
      const remoteCwd = await getSshSessionCwdApi(tab.id)
      if (remoteCwd) {
        tab.cwd = remoteCwd
        emit('terminal-cwd-change', tab.id, remoteCwd)
        return remoteCwd
      }
    } catch (e) {
      console.warn('获取终端目录失败:', e)
    }
  }
  return tab.cwd || '/'
}

// ==========================================
// 浏览器预览 (非 Tauri) 降级：模拟终端打字响应
// ==========================================
const printMockBanner = (term: Xterm) => {
  term.write('\r\n\x1b[1;36m SnailSSH 浏览器预览模式（未连接真实主机）\x1b[0m\r\n')
  term.write('\x1b[90m在桌面客户端中运行可建立真实 SSH 会话。\x1b[0m\r\n\r\n')
  term.write('\x1b[1;32mpreview@snail-ssh\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m$ \x1b[0m')
}

const writeMockPrompt = (term: Xterm) => {
  term.write('\x1b[1;32mpreview@snail-ssh\x1b[0m:\x1b[1;34m~\x1b[0m\x1b[1;32m$ \x1b[0m')
}

const handleMockTerminalInput = (tab: TerminalTab, session: TerminalSession, data: string) => {
  const { term } = session
  if (data === '\r') {
    term.write('\r\n')
    const cmd = session.inputBuffer.trim()
    session.inputBuffer = ''
    if (cmd === 'clear') {
      term.clear()
    } else if (cmd.startsWith('cd ')) {
      let target = cmd.slice(3).trim()
      if (target === '~' || target === '') target = '/root'
      else if (!target.startsWith('/')) target = '/root/' + target
      tab.cwd = target
      setMockTerminalCwd(tab.id, target)
      emit('terminal-cwd-change', tab.id, target)
      term.write(`\x1b[32m已切换到目录: ${target}\x1b[0m\r\n`)
    } else if (cmd === 'docker ps') {
      term.write('\x1b[90mCONTAINER ID   IMAGE          COMMAND       STATUS       PORTS                   NAMES\x1b[0m\r\n')
      term.write('\x1b[32ma9c812d4ef01\x1b[0m   nginx:alpine   "/docker..."  Up 2 hours   0.0.0.0:80->80/tcp      web-gateway\r\n')
      term.write('\x1b[32mf4b008e1a7b2\x1b[0m   redis:7.2      "docker..."   Up 3 days    0.0.0.0:6379->6379/tcp  redis-prod\r\n')
    } else if (cmd === 'free -h') {
      term.write('               total        used        free      shared  buff/cache   available\r\n')
      term.write('Mem:            15Gi       9.6Gi       1.2Gi       180Mi       4.2Gi       5.2Gi\r\n')
      term.write('Swap:          4.0Gi       320Mi       3.7Gi\r\n')
    } else if (cmd.length > 0) {
      term.write(`\x1b[33mCommand executed: ${cmd} (exit: 0)\x1b[0m\r\n`)
    }
    writeMockPrompt(term)
  } else if (data === '\u007F') {
    if (session.inputBuffer.length > 0) {
      session.inputBuffer = session.inputBuffer.slice(0, -1)
      term.write('\b \b')
    }
  } else if (data === '\x0c') {
    term.clear()
    writeMockPrompt(term)
  } else {
    session.inputBuffer += data
    term.write(data)
  }
}

const handleWindowResize = () => fitActive()

/** 全局 Ctrl+F / Ctrl+Enter：仅在当前活动工作区生效，终端聚焦时交给 xterm 自定义处理器 */
const handleGlobalKeydown = (e: KeyboardEvent) => {
  if (!props.active || dialogState.open) return
  // 在线编辑器打开时，Ctrl+F / Ctrl+Tab 属于编辑器，不要切到终端查找或换标签
  if (document.querySelector('[data-snail-editor]')) return
  const mod = e.ctrlKey || e.metaKey
  if (!mod) return
  const el = e.target as HTMLElement | null
  const inTerminal = !!(el && typeof el.closest === 'function' && el.closest('.xterm'))

  // Ctrl+Enter：唤出 / 收起命令输入框（终端聚焦时也要生效，否则写完命令还得找鼠标）
  if (e.key === 'Enter' && !e.shiftKey) {
    if (!currentTab.value) return
    // 焦点已在输入框内时不要抢：那里 Enter 是“发送”，由 onCommandBarKeydown 处理
    if ((e.target as HTMLElement | null)?.tagName === 'TEXTAREA') return
    e.preventDefault()
    if (commandBarOpen.value) closeCommandBar()
    else openCommandBar()
    return
  }

  // Ctrl+Tab / Ctrl+Shift+Tab 切换终端标签（终端内用户的肌肉记忆）
  if (e.key === 'Tab' && props.tabs.length > 1) {
    e.preventDefault()
    const total = props.tabs.length
    const next = (props.activeTabIndex + (e.shiftKey ? -1 : 1) + total) % total
    emit('switch-tab', next)
    return
  }

  // Ctrl+1..9 直达第 N 个终端标签
  if (!e.shiftKey && /^[1-9]$/.test(e.key)) {
    const index = Number(e.key) - 1
    if (index < props.tabs.length) {
      e.preventDefault()
      emit('switch-tab', index)
      return
    }
  }

  if (e.shiftKey || e.key.toLowerCase() !== 'f') return
  if (inTerminal) return
  if (!currentSession.value) return
  e.preventDefault()
  openSearch()
}

// 标签页增删 -> 创建/销毁终端实例
watch(() => props.tabs.map(t => t.id).join('|'), () => { syncSessions() })
// 切换标签页 -> 重新计算尺寸（隐藏时无法 fit），并关闭搜索条避免作用域错乱
watch(() => props.activeTabIndex, async () => {
  if (searchOpen.value) closeSearch()
  closeSuggestions()
  await nextTick()
  fitActive()
})

// 开关历史补全会增减底部固定行，只在这一次重新 fit；候选显隐不再改行列
watch(() => settings.historySuggestEnabled, async (enabled) => {
  if (!enabled) closeSuggestions()
  await nextTick()
  fitActive()
})

// 高亮开关：立即同步到所有终端
watch(() => highlightSettings.enabled, (enabled) => {
  for (const session of sessions.values()) session.highlight.setEnabled(enabled)
})

/**
 * 工作区可见性 -> 停扫/恢复扫描。
 * 切回是安全的：useTerminalHighlight 只暂停排帧，扫描水位线 lastScannedAbsoluteLine 不被改动，
 * 恢复时置 needsViewportScan 后从水位线继续增量扫，停扫期间写入的行会被补上，不会丢高亮。
 * 卸载时 sessions 已清空、watch 也已随组件停止，不会对已 dispose 的实例调用。
 */
watch(() => props.active, (active) => {
  for (const session of sessions.values()) session.highlight.setActive(active)
})

// 终端外观设置：改动即时应用到所有已打开实例，无需重开会话
watch(
  () => settings.terminal,
  (term) => {
    const theme = currentTerminalTheme().theme
    for (const session of sessions.values()) {
      session.term.options.fontFamily = term.fontFamily
      session.term.options.fontSize = term.fontSize
      session.term.options.lineHeight = term.lineHeight
      session.term.options.cursorStyle = term.cursorStyle
      session.term.options.cursorBlink = term.cursorBlink
      session.term.options.theme = theme
      session.fit.fit()
    }
    termSize.value = currentSession.value
      ? { cols: currentSession.value.term.cols, rows: currentSession.value.term.rows }
      : termSize.value
  },
  { deep: true }
)

// 命令输入框显隐会改变终端可用高度，必须重新 fit，否则行列数会与实际不符
watch(commandBarOpen, async () => {
  await nextTick()
  fitActive()
})

// 关闭命令输入框时把草稿清掉，避免切到另一台主机后误发上一条命令
watch(() => props.activeTabIndex, () => {
  commandDraft.value = ''
  historyIndex.value = -1
})

// 规则 / 配色 / 概览条变化：防抖后重建，避免正则输入时每键一次全量重扫
let highlightApplyTimer: number | null = null
watch(
  [() => highlightSettings.rules, () => highlightSettings.overviewRuler],
  () => {
    if (highlightApplyTimer !== null) window.clearTimeout(highlightApplyTimer)
    highlightApplyTimer = window.setTimeout(() => {
      highlightApplyTimer = null
      applyHighlightRules()
    }, 200)
  },
  { deep: true }
)

onMounted(async () => {
  window.addEventListener('resize', handleWindowResize)
  window.addEventListener('keydown', handleGlobalKeydown)
  await syncSessions()
})

onBeforeUnmount(() => {
  if (highlightApplyTimer !== null) {
    window.clearTimeout(highlightApplyTimer)
    highlightApplyTimer = null
  }
  window.removeEventListener('resize', handleWindowResize)
  window.removeEventListener('keydown', handleGlobalKeydown)
  for (const session of sessions.values()) {
    session.unlisten.forEach(fn => fn())
    session.highlight.dispose()
    session.term.dispose()
  }
  sessions.clear()
})

defineExpose({
  sendCommand: sendSnippetCommand,
  clearTerminal: clearCurrentTerminal,
  getCurrentCwd,
  fit: fitActive,
  focus: focusActive,
  openCommandBar
})
</script>
