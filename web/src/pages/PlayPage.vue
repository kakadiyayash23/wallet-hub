<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api } from '../api/client'
import { useWallet } from '../stores/wallet'
import { formatMinor, formatSigned } from '../lib/money'
import EmptyState from '../components/EmptyState.vue'
import type { Game } from '../api/types'

const { t, locale } = useI18n()
const route = useRoute()
const wallet = useWallet()
const url = ref('')
const error = ref('')
const game = ref<Game | null>(null)
const demo = computed(() => route.query.demo === '1')
const frame = ref<HTMLElement | null>(null)
// With a sandbox key, BigBang posts to the sandbox account even in real mode, so both balances are shown.

interface Movement { id: string; kind: string; amount: number; balance: number; currency: string; round_end: boolean | null }
interface Round { round_id: string; movements: Movement[]; net: number; closed: boolean; currency: string }

const historyRounds = ref<Round[]>([])
const expanded = ref<Set<string>>(new Set())
const flash = ref<'flash-pos' | 'flash-neg' | ''>('')

function groupRounds(items: { id: string; kind: string; amount: number; balance: number; currency: string; round_id: string | null; round_end: boolean | null }[]): Round[] {
  const map = new Map<string, Round>()
  for (const m of items) {
    const key = m.round_id ?? m.id
    const r = map.get(key) ?? { round_id: key, movements: [], net: 0, closed: false, currency: m.currency }
    r.movements.push({ id: m.id, kind: m.kind, amount: m.amount, balance: m.balance, currency: m.currency, round_end: m.round_end })
    r.net += m.amount
    if (m.round_end) r.closed = true
    map.set(key, r)
  }
  return [...map.values()]
}

const liveMoves = computed(() => wallet.recent.filter((e) => e.source === 'bigbang'))

const rounds = computed<Round[]>(() => {
  const live = liveMoves.value.map((e) => ({ id: e.journal_id ?? `${e.round_id}-${e.amount}`, kind: e.kind, amount: e.amount, balance: e.balance, currency: e.currency, round_id: e.round_id, round_end: e.round_end }))
  const liveRounds = groupRounds(live)
  const seen = new Set(liveRounds.map((r) => r.round_id))
  return [...liveRounds, ...historyRounds.value.filter((r) => !seen.has(r.round_id))].slice(0, 20)
})

const lastMove = computed(() => liveMoves.value[0] ?? null)

/** Balance after each of the last 20 rounds, oldest first, for the sparkline. */
const trend = computed(() => {
  const pts = rounds.value.slice(0, 20).map((r) => r.movements[r.movements.length - 1]?.balance ?? 0).reverse()
  return pts
})
const spark = computed(() => {
  const pts = trend.value
  if (pts.length < 2) return { line: '', area: '', last: null as null | { x: number; y: number }, up: true }
  const w = 220, h = 44, pad = 3
  const min = Math.min(...pts), max = Math.max(...pts)
  const span = max - min || 1
  const xy = pts.map((v, i) => ({ x: pad + (i / (pts.length - 1)) * (w - 2 * pad), y: h - pad - ((v - min) / span) * (h - 2 * pad) }))
  const line = xy.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')
  const area = `${xy[0].x},${h} ${line} ${xy[xy.length - 1].x},${h}`
  return { line, area, last: xy[xy.length - 1], up: pts[pts.length - 1] >= pts[0] }
})

watch(() => lastMove.value?.journal_id, () => {
  if (!lastMove.value) return
  flash.value = ''
  requestAnimationFrame(() => { flash.value = lastMove.value!.amount >= 0 ? 'flash-pos' : 'flash-neg' })
})

function toggle(id: string) {
  const s = new Set(expanded.value)
  if (s.has(id)) s.delete(id); else s.add(id)
  expanded.value = s
}

function fullscreen() { frame.value?.requestFullscreen?.() }

onMounted(async () => {
  try {
    const list = await api.get<Game[]>(`/api/games?limit=500`)
    game.value = list.find((g) => g.id === Number(route.params.gameId)) ?? null
  } catch { /* header is optional */ }
  try {
    const res = await api.post<{ url: string }>('/api/casino/launch', { game_id: Number(route.params.gameId), demo: demo.value })
    url.value = res.url
  } catch (e) {
    error.value = (e as Error).message
  }
  try {
    const journals = await wallet.ledger(80)
    historyRounds.value = groupRounds(
      journals.filter((j) => j.source === 'bigbang').reverse().map((j) => ({ id: j.id, kind: j.kind, amount: j.amount, balance: j.reply_balance, currency: j.currency, round_id: j.round_id, round_end: j.round_end })),
    ).reverse()
  } catch { /* history is optional */ }
})
</script>

<template>
  <div class="row" style="margin-bottom:10px">
    <RouterLink to="/lobby">← {{ t('play.back') }}</RouterLink>
    <h2 v-if="game" style="margin:0 0 0 6px;font-size:1.2rem">{{ game.title }} <small class="muted" style="font-weight:400">· {{ game.category }}<span v-if="demo"> · {{ t('lobby.demo') }}</span></small></h2>
    <span style="flex:1"></span>
    <button @click="fullscreen">⛶</button>
  </div>
  <p v-if="error" class="error">{{ error }}</p>
  <div v-else class="frame" ref="frame">
    <iframe v-if="url" :src="url" allow="autoplay; fullscreen" title="game"></iframe>
    <p v-else class="muted" style="padding:40px;text-align:center">{{ t('play.loading') }}</p>
  </div>

  <!-- Balance bar -->
  <div class="bbar card">
    <div>
      <div class="muted small">{{ t('play.balance') }}</div>
      <div class="num big" :class="flash">{{ formatMinor(wallet.live, wallet.currency, locale) }}</div>
    </div>
    <div>
      <div class="muted small">{{ t('wallet.sandbox') }}</div>
      <div class="num big" :class="flash">{{ formatMinor(wallet.sandbox, wallet.currency, locale) }}</div>
    </div>
    <div>
      <div class="muted small">{{ t('play.last') }}</div>
      <div v-if="lastMove" class="num" :class="[lastMove.amount > 0 ? 'pos' : lastMove.amount < 0 ? 'neg' : '', flash]">{{ formatSigned(lastMove.amount, lastMove.currency, locale) }} <small class="muted" style="font-weight:400">{{ t(`kind.${lastMove.kind}`, lastMove.kind) }}</small></div>
      <div v-else class="muted">—</div>
    </div>
    <div class="spark">
      <div class="muted small">{{ t('play.trend') }}</div>
      <svg v-if="spark.line" viewBox="0 0 220 44" width="220" height="44" aria-hidden="true">
        <polygon :points="spark.area" :fill="spark.up ? 'rgba(79,196,124,.14)' : 'rgba(239,107,99,.14)'" />
        <polyline :points="spark.line" fill="none" :stroke="spark.up ? 'var(--ok)' : 'var(--bad)'" stroke-width="2" stroke-linejoin="round" />
        <circle v-if="spark.last" :cx="spark.last.x" :cy="spark.last.y" r="3.5" :fill="spark.up ? 'var(--ok)' : 'var(--bad)'" />
      </svg>
      <div v-else class="muted">—</div>
    </div>
  </div>

  <p class="sect">{{ t('play.rounds') }}</p>
  <div v-if="rounds.length" class="rounds">
    <div v-for="r in rounds" :key="r.round_id" class="round">
      <button class="round-head" @click="toggle(r.round_id)">
        <span class="chev">{{ expanded.has(r.round_id) ? '▾' : '▸' }}</span>
        <span class="mono muted">…{{ r.round_id.slice(-8) }}</span>
        <span class="muted">{{ r.movements.length }} {{ t('play.movements') }}</span>
        <span class="chip" :class="r.closed ? 'ok' : 'warn'">{{ r.closed ? t('play.closedRound') : t('play.openRound') }}</span>
        <span style="flex:1"></span>
        <span class="num" :class="r.net > 0 ? 'pos' : r.net < 0 ? 'neg' : 'muted'">{{ formatSigned(r.net, r.currency, locale) }}</span>
      </button>
      <div v-if="expanded.has(r.round_id)">
        <div v-for="m in r.movements" :key="m.id" class="mv">
          <span>{{ t(`kind.${m.kind}`, m.kind) }}</span>
          <span class="mono" :class="m.amount > 0 ? 'pos' : m.amount < 0 ? 'neg' : 'muted'">{{ formatSigned(m.amount, m.currency, locale) }}</span>
          <span class="mono muted">{{ formatMinor(m.balance, m.currency, locale) }}</span>
          <small v-if="m.round_end" class="muted">round_end</small>
        </div>
      </div>
    </div>
  </div>
  <EmptyState v-else :title="t('play.noRounds')" :hint="t('play.noRoundsHint')" />
</template>

<style scoped>
.frame { aspect-ratio: 16 / 9; max-height: 70vh; background: #000; border-radius: 10px; overflow: hidden; border: 1px solid var(--line); }
iframe { width: 100%; height: 100%; border: 0; }
.bbar { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 14px; margin-top: 12px; align-items: center; }
.small { font-size: 12px; }
.big { font-size: 22px; }
.spark svg { display: block; max-width: 100%; }
.rounds { display: grid; gap: 6px; }
.round { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.round-head { display: flex; gap: 10px; align-items: center; width: 100%; text-align: left; padding: 7px 10px; background: transparent; border: 0; border-radius: 0; font-family: var(--body); font-weight: 400; font-size: 13.5px; }
.round-head:hover { background: var(--surface-2); }
.chev { color: var(--muted); width: 10px; }
.mv { display: grid; grid-template-columns: 1fr 110px 120px 80px; gap: 8px; padding: 4px 10px 4px 30px; font-size: 13px; border-top: 1px solid var(--line); }
</style>
