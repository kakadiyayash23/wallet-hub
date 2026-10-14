import { defineStore } from 'pinia'
import { ref } from 'vue'

export type ToastTone = 'ok' | 'info' | 'warn' | 'bad'
export interface Toast { id: number; tone: ToastTone; title: string; detail?: string }

let seq = 0
const lastShown = new Map<string, number>()

/** Short messages at the bottom right. Anything that happens off-screen lands here. */
export const useToasts = defineStore('toasts', () => {
  const items = ref<Toast[]>([])

  function push(tone: ToastTone, title: string, detail?: string, ttl = 4500) {
    // A burst of the same kind of event (a chaos run, a free-spin feature) shows one toast per 1.5 s.
    const bucket = `${tone}:${title.replace(/[\d.,€$£+−-]+/g, '#')}`
    const now = Date.now()
    if ((lastShown.get(bucket) ?? 0) > now - 1500) return -1
    lastShown.set(bucket, now)
    const id = ++seq
    items.value.push({ id, tone, title, detail })
    if (items.value.length > 5) items.value.shift()
    setTimeout(() => dismiss(id), ttl)
    return id
  }

  function dismiss(id: number) {
    items.value = items.value.filter((t) => t.id !== id)
  }

  return { items, push, dismiss }
})
