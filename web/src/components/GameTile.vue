<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { Game } from '../api/types'
defineProps<{ game: Game }>()
defineEmits<{ play: [game: Game]; demo: [game: Game] }>()
const { t } = useI18n()
</script>

<template>
  <div class="tile" @click="$emit('play', game)" :title="game.title">
    <img v-if="game.thumbnail" :src="game.thumbnail" :alt="game.title" loading="lazy" />
    <div class="hover">
      <button class="primary" @click.stop="$emit('play', game)">{{ t('lobby.real') }}</button>
      <button @click.stop="$emit('demo', game)">{{ t('lobby.demo') }}</button>
    </div>
    <div class="label">
      <div class="title">{{ game.title }}</div>
      <small class="muted">{{ game.category }}</small>
    </div>
  </div>
</template>

<style scoped>
.hover { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; gap: 6px; background: rgba(10,12,16,.55); opacity: 0; transition: opacity .12s; }
.tile:hover .hover, .tile:focus-within .hover { opacity: 1; }
</style>
