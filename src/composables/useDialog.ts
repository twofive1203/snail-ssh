import { reactive } from 'vue'

/**
 * 应用内对话框（替代原生 window.confirm / window.prompt）。
 *
 * 为什么不用原生弹窗：
 * 1. Tauri 各平台 WebView 对 confirm/prompt 的实现不一致，实测出现过
 *    "弹窗还显示在屏幕上，确认逻辑已经继续执行"的情况；prompt 在
 *    Linux WebKitGTK 下更是完全不支持；
 * 2. 原生弹窗会阻塞整个渲染线程，且无法定制文案与样式。
 * 因此统一走应用内模态框，等待用户明确选择后再返回结果。
 */
export type DialogVariant = 'default' | 'danger' | 'warning'

export interface DialogInputOptions {
  label?: string
  placeholder?: string
  value?: string
  /** 为 true 时内容为空不允许提交 */
  required?: boolean
}

export interface DialogOptions {
  title?: string
  message: string
  /** 次要说明，可多行展示 */
  detail?: string
  confirmText?: string
  cancelText?: string
  variant?: DialogVariant
  input?: DialogInputOptions
  /** 只展示信息（如「属性」），隐藏取消按钮 */
  hideCancel?: boolean
}

interface DialogState {
  open: boolean
  mode: 'confirm' | 'prompt'
  title: string
  message: string
  detail: string
  confirmText: string
  cancelText: string
  variant: DialogVariant
  input: DialogInputOptions | null
  value: string
  hideCancel: boolean
  resolve: ((value: any) => void) | null
}

export const dialogState = reactive<DialogState>({
  open: false,
  mode: 'confirm',
  title: '请确认',
  message: '',
  detail: '',
  confirmText: '确定',
  cancelText: '取消',
  variant: 'default',
  input: null,
  value: '',
  hideCancel: false,
  resolve: null
})

function openDialog(mode: 'confirm' | 'prompt', options: DialogOptions): Promise<any> {
  // 同一时刻只允许一个对话框，避免上一个 Promise 永久悬空
  if (dialogState.resolve) {
    const pending = dialogState.resolve
    dialogState.resolve = null
    pending(mode === 'prompt' ? null : false)
  }

  return new Promise((resolve) => {
    dialogState.mode = mode
    dialogState.title = options.title || (mode === 'prompt' ? '请输入' : '请确认')
    dialogState.message = options.message
    dialogState.detail = options.detail || ''
    dialogState.confirmText = options.confirmText || '确定'
    dialogState.cancelText = options.cancelText || '取消'
    dialogState.variant = options.variant || 'default'
    dialogState.input = options.input || null
    dialogState.value = options.input?.value ?? ''
    dialogState.hideCancel = options.hideCancel ?? false
    dialogState.resolve = resolve
    dialogState.open = true
  })
}

/** 二次确认，返回用户是否点击了确认按钮 */
export function confirmDialog(options: DialogOptions): Promise<boolean> {
  return openDialog('confirm', options)
}

/** 文本输入，返回用户输入内容；取消或关闭返回 null */
export function promptDialog(options: DialogOptions): Promise<string | null> {
  return openDialog('prompt', options)
}

/** 由 DialogHost 组件在用户选择后调用 */
export function resolveDialog(accepted: boolean): void {
  if (!dialogState.open) return
  const resolve = dialogState.resolve
  const result = dialogState.mode === 'prompt' ? (accepted ? dialogState.value : null) : accepted
  dialogState.open = false
  dialogState.resolve = null
  if (resolve) resolve(result)
}
