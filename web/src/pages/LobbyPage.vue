<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api } from '../api/client'
import type { CategoryCount, Game } from '../api/types'
import GameTile from '../components/GameTile.vue'

const { t } = useI18n()
const router = useRouter()
const games = ref<Game[]>([])
const categories = ref<CategoryCount[]>([])
const category = ref('')
const search = ref('')
const error = ref('')
let timer: ReturnType<typeof setTimeout> | undefined

async function load() {
  error.value = ''
  const params = new URLSearchParams({ limit: '120' })
  if (category.value) params.set('category', category.value)
  if (search.value.trim()) params.set('search', search.value.trim())
  try {
    games.value = await api.get<Game[]>(`/api/games?${params}`)
  } catch (e) {
    error.value = (e as Error).message
  }
}

onMounted(async () => {
  try { categories.value = await api.get<CategoryCount[]>('/api/games/categories') } catch { /* shown via empty state */ }
  await load()
})
watch(category, load)
watch(search, () => { clearTimeout(timer); timer = setTimeout(load, 250) })

function play(g: Game) { router.push(`/play/${g.id}`) }
function demo(g: Game) { router.push(`/play/${g.id}?demo=1`) }
</script>

<template>
  <div class="row" style="margin-bottom:12px">
    <input id="search" v-model="search" :placeholder="t('lobby.search')" style="flex:1;min-width:200px" />
    <select id="category" v-model="category">
      <option value="">{{ t('lobby.all') }}</option>
      <option v-for="c in categories" :key="c.category" :value="c.category">{{ c.category }} ({{ c.count }})</option>
    </select>
  </div>
  <p v-if="error" class="error">{{ error }}</p>
  <p v-else-if="games.length === 0" class="muted">{{ t('lobby.empty') }}</p>
  <div class="tiles">
    <GameTile v-for="g in games" :key="g.id" :game="g" @play="play" @demo="demo" />
  </div>
</template>
