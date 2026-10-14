<script setup lang="ts">
import type { Game } from '../api/types'
defineProps<{ game: Game }>()
defineEmits<{ play: [game: Game]; demo: [game: Game] }>()
</script>

<template>
  <div class="tile" @click="$emit('play', game)" :title="game.title">
    <img v-if="game.thumbnail" :src="game.thumbnail" :alt="game.title" loading="lazy" />
    <div class="label">
      <div>{{ game.title }}</div>
      <div class="row" style="gap:6px;justify-content:space-between">
        <small class="muted">{{ game.category }}</small>
        <small><a href="#" @click.stop.prevent="$emit('demo', game)">demo</a></small>
      </div>
    </div>
  </div>
</template>
