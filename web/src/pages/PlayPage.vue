<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api } from '../api/client'
import { useWallet } from '../stores/wallet'
import { formatMinor, formatSigned } from '../lib/money'

const { t, locale } = useI18n()
const route = useRoute()
const wallet = useWallet()
const url = ref('')
const error = ref('')
const demo = computed(() => route.query.demo === '1')
// With a sandbox key, BigBang posts to the sandbox account even in real mode, so both balances are shown.

interface Movement { id: string; kind: string; amount: number; balance: number; currency: string; round_end: boolean | null }
interface Round { round_id: string; movements: Movement[]; net: number; closed: boolean; currency: string }

/** Movements from the ledger history plus live events, grouped by round_id, newest round first.
 *  A standard game sends one closed movement per round; linked providers send bet, win… then a close. */
const history = ref<Movement[] & { round_id?: string }[]>([])
const historyRounds = ref<Round[]>([])

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

const rounds = computed<Round[]>(() => {
  const live = wallet.recent
    .filter((e) => e.source === 'bigbang')
    .map((e) => ({ id: e.journal_id ?? `${e.round_id}-${e.amount}`, kind: e.kind, amount: e.amount, balance: e.balance, currency: e.currency, round_id: e.round_id, round_end: e.round_end }))
  const liveRounds = groupRounds(live)
  const seen = new Set(liveRounds.map((r) => r.round_id))
  return [...liveRounds, ...historyRounds.value.filter((r) => !seen.has(r.round_id))].slice(0, 12)
})

onMounted(async () => {
  try {
    const res = await api.post<{ url: string }>('/api/casino/launch', { game_id: Number(route.params.gameId), demo: demo.value })
    url.value = res.url
  } catch (e) {
    error.value = (e as Error).message
  }
  try {
    const journals = await wallet.ledger(60)
    historyRounds.value = groupRounds(
      journals
        .filter((j) => j.source === 'bigbang')
        .reverse()
        .map((j) => ({ id: j.id, kind: j.kind, amount: j.amount, balance: j.reply_balance, currency: j.currency, round_id: j.round_id, round_end: j.round_end })),
    ).reverse()
  } catch { /* history is optional */ }
  void history
})
</script>

<template>
  <div class="row" style="margin-bottom:10px">
    <RouterLink to="/lobby">← {{ t('play.back') }}</RouterLink>
    <span class="sp" style="flex:1"></span>
    <span class="muted">{{ t('play.balance') }}</span>
    <strong class="mono">{{ formatMinor(wallet.live, wallet.currency, locale) }}</strong>
    <span class="muted">· {{ t('wallet.sandbox') }}</span>
    <strong class="mono">{{ formatMinor(wallet.sandbox, wallet.currency, locale) }}</strong>
  </div>
  <p v-if="error" class="error">{{ error }}</p>
  <div v-else class="frame">
    <iframe v-if="url" :src="url" allow="autoplay; fullscreen" title="game"></iframe>
    <p v-else class="muted" style="padding:40px;text-align:center">{{ t('play.loading') }}</p>
  </div>
  <p class="sect">{{ t('play.rounds') }}</p>
  <div v-if="rounds.length" class="rounds">
    <div v-for="r in rounds" :key="r.round_id" class="round">
      <div class="round-head">
        <span class="mono muted">{{ t('play.round') }} …{{ r.round_id.slice(-8) }}</span>
        <span class="muted">{{ r.movements.length }} {{ t('play.movements') }}</span>
        <span class="chip" :class="r.closed ? 'ok' : 'warn'">{{ r.closed ? t('play.closedRound') : t('play.openRound') }}</span>
        <span class="sp" style="flex:1"></span>
        <span class="muted">{{ t('play.net') }}</span>
        <strong class="mono" :class="r.net > 0 ? 'pos' : r.net < 0 ? 'neg' : ''">{{ formatSigned(r.net, r.currency, locale) }}</strong>
      </div>
      <div v-for="m in r.movements" :key="m.id" class="mv">
        <span>{{ t(`kind.${m.kind}`, m.kind) }}</span>
        <span class="mono" :class="m.amount > 0 ? 'pos' : m.amount < 0 ? 'neg' : 'muted'">{{ formatSigned(m.amount, m.currency, locale) }}</span>
        <span class="mono muted">{{ formatMinor(m.balance, m.currency, locale) }}</span>
        <small v-if="m.round_end" class="muted">round_end</small>
      </div>
    </div>
  </div>
  <p v-else class="muted">—</p>
</template>

<style scoped>
.frame { aspect-ratio: 16 / 9; max-height: 70vh; background: #000; border-radius: 10px; overflow: hidden; border: 1px solid var(--line); }
iframe { width: 100%; height: 100%; border: 0; }
.rounds { display: grid; gap: 8px; }
.round { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; overflow: hidden; }
.round-head { display: flex; gap: 10px; align-items: center; padding: 6px 10px; background: var(--surface-2); font-size: 13px; }
.mv { display: grid; grid-template-columns: 1fr 110px 120px 80px; gap: 8px; padding: 4px 10px; font-size: 13px; border-top: 1px solid var(--line); }
</style>
