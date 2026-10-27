<template>
  <div v-if="prompt" class="fixed inset-0 bg-black/75 backdrop-blur-sm z-[70] flex items-center justify-center p-4">
    <form class="w-full max-w-md bg-surface-sidebar border border-surface-border rounded-2xl shadow-2xl overflow-hidden" @submit.prevent="submit">
      <div class="px-5 py-3.5 border-b border-surface-border bg-surface-card/30">
        <h2 class="text-sm font-bold text-white">服务器要求继续认证</h2>
        <p class="text-[11px] text-surface-subtle font-mono mt-0.5">{{ prompt.hostLabel }}</p>
      </div>
      <div class="px-5 py-4 space-y-3">
        <p v-if="prompt.name" class="text-xs text-slate-200">{{ prompt.name }}</p>
        <p v-if="prompt.instructions" class="text-[11px] text-slate-400 whitespace-pre-wrap">{{ prompt.instructions }}</p>
        <div v-for="(field, index) in prompt.prompts" :key="index" class="space-y-1.5">
          <label class="text-[11px] font-medium text-slate-300">{{ field.prompt || '请输入' }}</label>
          <input
            v-model="answers[index]"
            :type="field.secret ? 'password' : 'text'"
            class="w-full bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-brand-500 font-mono"
          />
        </div>
      </div>
      <div class="px-5 py-3 border-t border-surface-border flex justify-end gap-2">
        <button type="button" class="px-3.5 py-1.5 rounded-lg bg-surface-card border border-surface-border text-xs text-slate-300" @click="cancel">取消</button>
        <button type="submit" class="px-4 py-1.5 rounded-lg bg-brand-500 text-xs font-semibold text-slate-950">提交</button>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import type { AuthPromptEvent } from '@/types'
import { submitSshAuthPromptApi } from '@/api/tauri'

const props = defineProps<{ prompt: AuthPromptEvent | null }>()
const emit = defineEmits<{ (e: 'done'): void }>()
const answers = ref<string[]>([])

watch(() => props.prompt, (value) => {
  answers.value = value ? value.prompts.map(() => '') : []
}, { immediate: true })

const submit = async () => {
  if (!props.prompt) return
  await submitSshAuthPromptApi(props.prompt.requestId, answers.value)
  emit('done')
}

const cancel = async () => {
  if (!props.prompt) return
  await submitSshAuthPromptApi(props.prompt.requestId, null)
  emit('done')
}
</script>
