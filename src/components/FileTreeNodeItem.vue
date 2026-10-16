<template>
  <div class="select-none text-xs font-mono">
    <!-- 节点行 -->
    <div
      class="group flex items-center gap-1.5 px-2 py-1.5 rounded cursor-pointer transition-colors relative"
      :class="[
        isSelected ? 'bg-brand-500/25 text-emerald-300 font-semibold border-l-2 border-brand-500' : 'hover:bg-surface-card/70 text-slate-300',
        node.isHidden ? 'opacity-60' : ''
      ]"
      :style="{ paddingLeft: `${depth * 14 + 8}px` }"
      @click="handleClick"
      @dblclick="handleDoubleClick"
      @contextmenu.prevent.stop="handleContextMenu($event)"
    >
      <!-- 展开/折叠箭头（仅文件夹展示） -->
      <button
        v-if="node.isDir"
        class="w-4 h-4 flex items-center justify-center shrink-0 text-slate-400 hover:text-white rounded"
        @click.stop="toggleExpand"
      >
        <component
          :is="node.loading ? RefreshCw : (node.expanded ? ChevronDown : ChevronRight)"
          class="w-3.5 h-3.5"
          :class="{ 'animate-spin text-brand-400': node.loading }"
        />
      </button>
      <span v-else class="w-4 shrink-0"></span>

      <!-- 类型图标 -->
      <component
        :is="iconInfo.icon"
        class="w-4 h-4 shrink-0"
        :class="iconInfo.color"
      />

      <!-- 文件/目录名 -->
      <span class="truncate flex-1 text-[12px]" :title="node.name">
        {{ node.name }}
      </span>

      <!-- 快捷标识/右侧大小 -->
      <span
        v-if="!node.isDir && node.size && node.size !== '-'"
        class="text-[10px] text-slate-500 group-hover:text-slate-400 shrink-0 font-sans"
      >
        {{ node.size }}
      </span>
    </div>

    <!-- 子目录树（展开时递归渲染） -->
    <div v-if="node.isDir && node.expanded && node.children && node.children.length > 0">
      <FileTreeNodeItem
        v-for="child in filteredChildren"
        :key="child.path"
        :node="child"
        :depth="depth + 1"
        :selected-path="selectedPath"
        :show-hidden="showHidden"
        @select="(n) => emit('select', n)"
        @toggle="(n) => emit('toggle', n)"
        @open-file="(n) => emit('open-file', n)"
        @context-menu="(e, n) => emit('context-menu', e, n)"
      />
    </div>

    <!-- 展开后如果为空目录 -->
    <div
      v-else-if="node.isDir && node.expanded && node.loaded && (!node.children || node.children.length === 0)"
      class="text-[11px] text-slate-500 py-1 italic"
      :style="{ paddingLeft: `${(depth + 1) * 14 + 24}px` }"
    >
      (空目录)
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  ChevronRight,
  ChevronDown,
  RefreshCw,
  Folder,
  FolderOpen,
  FileText,
  FileCode,
  Archive,
  Terminal,
  Settings,
  Shield,
  FileImage,
  File
} from 'lucide-vue-next'
import type { FileTreeNode } from '@/types'

const props = defineProps<{
  node: FileTreeNode
  depth: number
  selectedPath: string
  showHidden: boolean
}>()

const emit = defineEmits<{
  (e: 'select', node: FileTreeNode): void
  (e: 'toggle', node: FileTreeNode): void
  (e: 'open-file', node: FileTreeNode): void
  (e: 'context-menu', evt: MouseEvent, node: FileTreeNode): void
}>()

const isSelected = computed(() => props.selectedPath === props.node.path)

const filteredChildren = computed(() => {
  if (!props.node.children) return []
  return props.node.children.filter(c => props.showHidden || !c.isHidden)
})

// 图标与配色计算
const iconInfo = computed(() => {
  if (props.node.isDir) {
    if (props.node.expanded) {
      return { icon: FolderOpen, color: 'text-amber-400' }
    }
    return { icon: Folder, color: 'text-amber-400' }
  }

  const name = props.node.name.toLowerCase()
  if (name.endsWith('.sh') || name.endsWith('.bash') || name.endsWith('.zsh')) {
    return { icon: Terminal, color: 'text-emerald-400' }
  }
  if (
    name.endsWith('.tar.gz') ||
    name.endsWith('.tgz') ||
    name.endsWith('.zip') ||
    name.endsWith('.gz') ||
    name.endsWith('.rar') ||
    name.endsWith('.7z') ||
    name.endsWith('.jar') ||
    name.endsWith('.war')
  ) {
    return { icon: Archive, color: 'text-orange-400' }
  }
  if (
    name.endsWith('.yaml') ||
    name.endsWith('.yml') ||
    name.endsWith('.json') ||
    name.endsWith('.toml') ||
    name.endsWith('.conf') ||
    name.endsWith('.cfg') ||
    name.endsWith('.ini') ||
    name.endsWith('.env')
  ) {
    return { icon: Settings, color: 'text-cyan-400' }
  }
  if (
    name.endsWith('.js') ||
    name.endsWith('.ts') ||
    name.endsWith('.vue') ||
    name.endsWith('.py') ||
    name.endsWith('.go') ||
    name.endsWith('.rs') ||
    name.endsWith('.java') ||
    name.endsWith('.c') ||
    name.endsWith('.cpp') ||
    name.endsWith('.html') ||
    name.endsWith('.css')
  ) {
    return { icon: FileCode, color: 'text-blue-400' }
  }
  if (name.endsWith('.log') || name.endsWith('.out')) {
    return { icon: FileText, color: 'text-purple-400' }
  }
  if (name.endsWith('.png') || name.endsWith('.jpg') || name.endsWith('.jpeg') || name.endsWith('.svg') || name.endsWith('.gif')) {
    return { icon: FileImage, color: 'text-rose-400' }
  }
  if (name.endsWith('.key') || name.endsWith('.pem') || name.endsWith('.crt') || name.endsWith('.cert')) {
    return { icon: Shield, color: 'text-amber-300' }
  }
  return { icon: File, color: 'text-slate-400' }
})

const handleClick = () => {
  emit('select', props.node)
}

const toggleExpand = () => {
  if (props.node.isDir) {
    emit('toggle', props.node)
  }
}

const handleDoubleClick = () => {
  if (props.node.isDir) {
    emit('toggle', props.node)
  } else {
    emit('open-file', props.node)
  }
}

const handleContextMenu = (evt: MouseEvent) => {
  evt.stopPropagation()
  emit('select', props.node)
  emit('context-menu', evt, props.node)
}
</script>
