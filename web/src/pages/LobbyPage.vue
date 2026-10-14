<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api } from '../api/client'
import type { Game, Lobby } from '../api/types'
import GameTile from '../components/GameTile.vue'
import Skeleton from '../components/Skeleton.vue'
import EmptyState from '../components/EmptyState.vue'

const { t } = useI18n()
const router = useRouter()
const route = useRoute()
const lobby = ref<Lobby | null>(null)
const games = ref<Game[]>([])
const category = ref((route.query.provider as string) ?? '')
const search = ref((route.query.q as string) ?? '')
const loading = ref(true)
const error = ref('')
let timer: ReturnType<typeof setTimeout> | undefined

const filtering = computed(() => category.value !== '' || search.value.trim() !== '')

async function loadLobby() {
  loading.value = true
  try { lobby.value = await api.get<Lobby>('/api/games/lobby') } catch (e) { error.value = (e as Error).message } finally { loading.value = false }
}

async function loadFiltered() {
  loading.value = true
  error.value = ''
  const params = new URLSearchParams({ limit: '200' })
  if (category.value) params.set('category', category.value)
  if (search.value.trim()) params.set('search', search.value.trim())
  try { games.value = await api.get<Game[]>(`/api/games?${params}`) } catch (e) { error.value = (e as Error).message } finally { loading.value = false }
}

function syncUrl() {
  const q: Record<string, string> = {}
  if (category.value) q.provider = category.value
  if (search.value.trim()) q.q = search.value.trim()
  router.replace({ query: q })
}

onMounted(async () => {
  await loadLobby()
  if (filtering.value) await loadFiltered()
})
watch(category, () => { syncUrl(); if (filtering.value) loadFiltered() })
watch(search, () => { clearTimeout(timer); timer = setTimeout(() => { syncUrl(); if (filtering.value) loadFiltered() }, 250) })

function play(g: Game) { router.push(`/play/${g.id}`) }
function demo(g: Game) { router.push(`/play/${g.id}?demo=1`) }
function clear() { category.value = ''; search.value = '' }
</script>

<template>
  <div class="row" style="margin-bottom:14px">
    <input id="search" v-model="search" :placeholder="t('lobby.search')" style="flex:1;min-width:220px" />
    <button v-if="filtering" @click="clear">{{ t('lobby.clear') }}</button>
  </div>
  <div v-if="lobby" class="chips">
    <button class="pchip" :class="{ active: category === '' }" @click="category = ''">{{ t('lobby.all') }}</button>
    <button v-for="p in lobby.providers" :key="p.category" class="pchip" :class="{ active: category === p.category }" @click="category = p.category">
      <span class="ini">{{ p.category.slice(0, 2).toUpperCase() }}</span>{{ p.category }} <span class="muted">{{ p.count }}</span>
    </button>
  </div>
  <p v-if="error" class="error">{{ error }}</p>

  <!-- Filtered grid -->
  <template v-if="filtering">
    <p class="sect">{{ games.length }} {{ t('lobby.results') }}<span v-if="category"> · {{ category }}</span></p>
    <div v-if="loading" class="tiles"><Skeleton tile :count="12" /></div>
    <EmptyState v-else-if="games.length === 0" :title="t('lobby.noResults')" :hint="t('lobby.tryAgain')" />
    <div v-else class="tiles"><GameTile v-for="g in games" :key="g.id" :game="g" @play="play" @demo="demo" /></div>
  </template>

  <!-- Lobby rows -->
  <template v-else>
    <template v-if="loading && !lobby">
      <p class="sect">{{ t('lobby.popular') }}</p>
      <div class="scroller"><Skeleton tile :count="6" /></div>
    </template>
    <template v-else-if="lobby">
      <template v-if="lobby.continue_playing.length">
        <p class="sect">{{ t('lobby.continue') }}</p>
        <div class="scroller"><GameTile v-for="g in lobby.continue_playing" :key="g.id" :game="g" @play="play" @demo="demo" /></div>
      </template>
      <template v-if="lobby.popular.length">
        <p class="sect">{{ t('lobby.popular') }}</p>
        <div class="scroller"><GameTile v-for="g in lobby.popular" :key="g.id" :game="g" @play="play" @demo="demo" /></div>
      </template>
      <EmptyState v-if="lobby.providers.length === 0" :title="t('lobby.empty')" />
      <section v-for="p in lobby.providers" :key="p.category" class="prow">
        <div class="row" style="justify-content:space-between">
          <p class="sect" style="margin:0">{{ p.category }} <span class="muted">· {{ p.count }} {{ t('lobby.games') }}</span></p>
          <a href="#" @click.prevent="category = p.category">{{ t('lobby.seeAll') }} →</a>
        </div>
        <div class="scroller" style="margin-top:8px"><GameTile v-for="g in p.games" :key="g.id" :game="g" @play="play" @demo="demo" /></div>
      </section>
    </template>
  </template>
</template>

<style scoped>
.chips { display: flex; gap: 6px; overflow-x: auto; padding-bottom: 6px; margin-bottom: 6px; scrollbar-width: thin; }
.pchip { white-space: nowrap; display: inline-flex; align-items: center; gap: 7px; padding: 5px 10px 5px 6px; border-radius: 999px; font-weight: 500; }
.pchip.active { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }
.pchip.active .muted { color: var(--accent-ink); opacity: .75; }
.ini { display: inline-grid; place-items: center; width: 22px; height: 22px; border-radius: 50%; background: var(--surface-3); font-size: 10px; font-weight: 700; color: var(--fg); }
.pchip.active .ini { background: rgba(0,0,0,.18); color: var(--accent-ink); }
.prow { margin-top: 18px; }
</style>
