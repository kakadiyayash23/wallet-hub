<script setup lang="ts">
defineProps<{ count?: number; tile?: boolean; height?: string }>()
</script>

<template>
  <template v-if="tile">
    <div v-for="i in count ?? 6" :key="i" class="skeleton" style="aspect-ratio:4/3"></div>
  </template>
  <template v-else>
    <div v-for="i in count ?? 3" :key="i" class="skeleton" :style="{ height: height ?? '18px', marginBottom: '8px' }"></div>
  </template>
</template>
