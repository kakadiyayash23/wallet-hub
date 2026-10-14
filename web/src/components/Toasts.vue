<script setup lang="ts">
import { useToasts } from '../stores/toasts'
const toasts = useToasts()
</script>

<template>
  <div class="toasts" aria-live="polite">
    <TransitionGroup name="toast">
      <div v-for="t in toasts.items" :key="t.id" class="toast" :class="t.tone" @click="toasts.dismiss(t.id)">
        <div class="t">{{ t.title }}</div>
        <div v-if="t.detail" class="d">{{ t.detail }}</div>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toasts { position: fixed; right: 16px; bottom: 16px; display: grid; gap: 8px; z-index: 50; width: min(340px, calc(100vw - 32px)); }
.toast { background: var(--surface-2); border: 1px solid var(--line); border-left: 3px solid var(--info); border-radius: 8px; padding: 9px 12px; box-shadow: var(--shadow); cursor: pointer; }
.toast.ok { border-left-color: var(--ok); } .toast.warn { border-left-color: var(--warn); } .toast.bad { border-left-color: var(--bad); }
.t { font-family: var(--display); font-weight: 600; font-size: 14px; }
.d { color: var(--muted); font-size: 12.5px; margin-top: 2px; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(8px); }
.toast-enter-active, .toast-leave-active { transition: all .18s ease; }
</style>
