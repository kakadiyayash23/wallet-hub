<script setup lang="ts">
defineProps<{ title: string; hint?: string; to?: string; action?: string }>()
</script>

<template>
  <div class="empty">
    <div class="mark">◌</div>
    <div class="title">{{ title }}</div>
    <div v-if="hint" class="muted">{{ hint }}</div>
    <RouterLink v-if="to && action" :to="to"><button class="primary" style="margin-top:10px">{{ action }}</button></RouterLink>
  </div>
</template>

<style scoped>
.empty { text-align: center; padding: 28px 16px; border: 1px dashed var(--line); border-radius: 10px; color: var(--fg); }
.mark { font-size: 28px; color: var(--muted); line-height: 1; margin-bottom: 8px; }
.title { font-family: var(--display); font-weight: 600; }
</style>
