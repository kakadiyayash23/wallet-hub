<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{ status: string; label?: string }>()
const tone = computed(() => {
  switch (props.status) {
    case 'applied': case 'completed': case 'won': case 'booked': case 'paid': case 'read': return 'ok'
    case 'duplicate': case 'ignored': return 'info'
    case 'pending': case 'open': case 'pending_review': return 'warn'
    case 'rejected': case 'error': case 'failed': case 'lost': case 'rolled back': return 'bad'
    default: return 'muted'
  }
})
</script>

<template><span class="chip" :class="tone">{{ label ?? status }}</span></template>
