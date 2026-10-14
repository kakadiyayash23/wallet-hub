<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api, ApiError } from '../api/client'
import type { Bet, Market, Selection, SportInfo } from '../api/types'
import { useWallet } from '../stores/wallet'
import { formatMinor, normalizeAmountInput } from '../lib/money'
import StatusChip from '../components/StatusChip.vue'
import EmptyState from '../components/EmptyState.vue'

const { t, locale } = useI18n()
const wallet = useWallet()

const sports = ref<SportInfo[]>([])
const sport = ref('')
const markets = ref<Market[]>([])
const bets = ref<Bet[]>([])
const pick = ref<{ market: Market; sel: Selection } | null>(null)
const stake = ref('10')
const error = ref('')
const notice = ref('')
const busy = ref(false)
const loading = ref(false)

const ICONS: Record<string, string> = { soccer: '⚽', cricket: '🏏', basketball: '🏀', tennis: '🎾' }

/** Upcoming events grouped by league, leagues ordered by their first start time. */
const leagues = computed(() => {
  const groups = new Map<string, Market[]>()
  for (const m of [...markets.value].sort((a, b) => a.starts_at.localeCompare(b.starts_at))) {
    const list = groups.get(m.league) ?? []
    list.push(m)
    groups.set(m.league, list)
  }
  return [...groups.entries()].map(([league, events]) => ({ league, events }))
})

const openBets = computed(() => bets.value.filter((b) => b.status === 'open'))
const settledBets = computed(() => bets.value.filter((b) => b.status !== 'open').slice(0, 8))

const potential = computed(() => {
  const s = normalizeAmountInput(stake.value)
  if (!s || !pick.value) return 0
  return Math.floor(Number(s) * 100 * pick.value.sel.odds_milli / 1000)
})

const odds = (m: number) => (m / 1000).toFixed(2)

function startLabel(iso: string) {
  const d = new Date(iso)
  const now = new Date()
  const sameDay = d.toDateString() === now.toDateString()
  const tomorrow = new Date(now); tomorrow.setDate(now.getDate() + 1)
  const time = new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit' }).format(d)
  if (sameDay) return `${t('sports.today')} ${time}`
  if (d.toDateString() === tomorrow.toDateString()) return `${t('sports.tomorrow')} ${time}`
  return new Intl.DateTimeFormat(locale.value, { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }).format(d)
}

function selLabel(s: Selection) {
  if (s.key === 'draw') return t('sports.draw')
  return s.label
}

function isPicked(m: Market, s: Selection) {
  return pick.value?.market.event_id === m.event_id && pick.value?.sel.key === s.key
}

function choose(m: Market, s: Selection) {
  pick.value = isPicked(m, s) ? null : { market: m, sel: s }
  error.value = ''
  notice.value = ''
}

async function loadSports() {
  sports.value = await api.get<SportInfo[]>('/api/sports')
  if (!sport.value) sport.value = sports.value.find((s) => s.count > 0)?.key ?? sports.value[0]?.key ?? ''
}

async function loadMarkets() {
  if (!sport.value) return
  loading.value = true
  try {
    markets.value = await api.get<Market[]>(`/api/sports/markets?sport=${encodeURIComponent(sport.value)}`)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    loading.value = false
  }
}

async function loadBets() {
  bets.value = await api.get<Bet[]>('/api/sports/bets')
}

onMounted(async () => {
  try { await loadSports() } catch (e) { error.value = (e as Error).message }
  await Promise.all([loadMarkets(), loadBets()])
})
watch(sport, () => { pick.value = null; loadMarkets() })
watch(() => wallet.recent.length, loadBets)

async function place() {
  if (!pick.value) return
  const s = normalizeAmountInput(stake.value)
  if (!s) { error.value = t('sports.stake'); return }
  error.value = ''
  notice.value = ''
  busy.value = true
  try {
    await api.post('/api/sports/bets', {
      event_id: pick.value.market.event_id,
      event_name: `${pick.value.market.home} v ${pick.value.market.away}`,
      selection: pick.value.sel.key,
      selection_label: selLabel(pick.value.sel),
      odds_milli: pick.value.sel.odds_milli,
      stake: s,
    })
    notice.value = t('sports.placed')
    pick.value = null
    await loadBets()
  } catch (e) {
    if (e instanceof ApiError && e.code === 'provider_error' && e.message.includes('odds changed')) {
      error.value = t('sports.oddsChanged')
      await loadMarkets()
    } else {
      error.value = (e as Error).message
    }
  } finally {
    busy.value = false
  }
}

async function cashOut(b: Bet) {
  try { await api.post(`/api/sports/bets/${b.id}/cashout`); await loadBets() } catch (e) { error.value = (e as Error).message }
}
</script>

<template>
  <div class="sb">
    <!-- Sport rail -->
    <aside class="rail">
      <p class="sect" style="margin-top:4px">{{ t('sports.title') }}</p>
      <button v-for="s in sports" :key="s.key" class="rail-item" :class="{ active: s.key === sport }" @click="sport = s.key">
        <span class="ico">{{ ICONS[s.key] ?? '🏅' }}</span>
        <span class="lbl">{{ s.label }}</span>
        <span class="cnt mono">{{ s.count }}</span>
      </button>
    </aside>

    <!-- Events -->
    <section class="events">
      <header class="row" style="justify-content:space-between;margin-bottom:8px">
        <h2 style="margin:0">{{ ICONS[sport] }} {{ sports.find((s) => s.key === sport)?.label ?? '' }} · {{ t('sports.upcoming') }}</h2>
        <span class="muted">{{ markets.length }} {{ t('sports.upcoming').toLowerCase() }}</span>
      </header>
      <p v-if="loading" class="muted">{{ t('common.loading') }}</p>
      <p v-else-if="leagues.length === 0" class="muted">{{ t('sports.noEvents') }}</p>
      <div v-for="g in leagues" :key="g.league" class="league">
        <div class="league-head">
          <span>{{ g.league }}</span>
          <span class="cols muted">
            <template v-if="g.events[0].selections.length === 3"><span>1</span><span>X</span><span>2</span></template>
            <template v-else><span>1</span><span>2</span></template>
          </span>
        </div>
        <div v-for="m in g.events" :key="m.event_id" class="event">
          <div class="when mono muted">{{ startLabel(m.starts_at) }}</div>
          <div class="teams">
            <div :title="m.home">{{ m.home }}</div>
            <div :title="m.away">{{ m.away }}</div>
          </div>
          <div class="odds" :class="{ two: m.selections.length === 2 }">
            <button v-for="s in m.selections" :key="s.key" class="odd" :class="{ picked: isPicked(m, s) }" @click="choose(m, s)">
              <small>{{ s.key === 'draw' ? 'X' : s.key === 'home' ? '1' : '2' }}</small>
              <b class="mono">{{ odds(s.odds_milli) }}</b>
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- Bet slip -->
    <aside class="slip">
      <div class="card">
        <p class="sect" style="margin-top:0">{{ t('sports.slip') }}</p>
        <template v-if="pick">
          <div class="pickbox">
            <div class="row" style="justify-content:space-between">
              <strong>{{ selLabel(pick.sel) }}</strong>
              <b class="mono accent">{{ odds(pick.sel.odds_milli) }}</b>
            </div>
            <small class="muted">{{ pick.market.home }} v {{ pick.market.away }} · {{ pick.market.league }}</small>
          </div>
          <label class="muted" style="font-size:12px">{{ t('sports.stake') }} ({{ wallet.currency }})</label>
          <div class="row">
            <input id="stake" v-model="stake" inputmode="decimal" style="flex:1" />
            <button v-for="q in ['5', '10', '25', '50']" :key="q" @click="stake = q">{{ q }}</button>
          </div>
          <div class="row" style="justify-content:space-between;margin:10px 0">
            <span class="muted">{{ t('sports.return') }}</span>
            <strong class="mono">{{ formatMinor(potential, wallet.currency, locale) }}</strong>
          </div>
          <button class="primary" style="width:100%" :disabled="busy" @click="place">{{ t('sports.place') }}</button>
          <button style="width:100%;margin-top:6px" @click="pick = null">{{ t('sports.clear') }}</button>
        </template>
        <p v-else class="muted">{{ t('sports.pick') }}</p>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-if="notice" class="pos" style="font-size:13px">{{ notice }}</p>
      </div>

      <div class="card" style="margin-top:12px">
        <p class="sect" style="margin-top:0">{{ t('sports.open') }} · {{ openBets.length }}</p>
        <div v-for="b in openBets" :key="b.id" class="ticket">
          <div><strong>{{ b.selection }}</strong> <span class="mono muted">@ {{ odds(b.odds_milli) }}</span></div>
          <small class="muted">{{ b.event_name }}</small>
          <div class="row" style="justify-content:space-between;margin-top:4px">
            <span class="mono">{{ formatMinor(b.stake, b.currency, locale) }}</span>
            <button @click="cashOut(b)">{{ t('sports.cashout') }}</button>
          </div>
        </div>
        <EmptyState v-if="openBets.length === 0" :title="t('sports.noBets')" :hint="t('sports.noBetsHint')" />
      </div>

      <div v-if="settledBets.length" class="card" style="margin-top:12px">
        <p class="sect" style="margin-top:0">{{ t('sports.mySlip') }}</p>
        <div v-for="b in settledBets" :key="b.id" class="ticket">
          <div class="row" style="justify-content:space-between">
            <span><strong>{{ b.selection }}</strong> <span class="mono muted">@ {{ odds(b.odds_milli) }}</span></span>
            <StatusChip :status="b.status" :label="t(`sports.status.${b.status}`, b.status)" />
          </div>
          <div class="row" style="justify-content:space-between">
            <small class="muted">{{ b.event_name }}</small>
            <span v-if="b.payout" class="mono pos">+{{ formatMinor(b.payout, b.currency, locale) }}</span>
          </div>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.sb { display: grid; grid-template-columns: 180px minmax(0, 1fr) 300px; gap: 16px; align-items: start; }
.rail { position: sticky; top: 64px; display: grid; gap: 4px; }
.rail-item { display: flex; align-items: center; gap: 8px; text-align: left; padding: 8px 10px; background: transparent; border-color: transparent; }
.rail-item:hover { background: var(--surface-2); }
.rail-item.active { background: var(--surface-2); border-color: var(--line); }
.rail-item .ico { width: 22px; text-align: center; }
.rail-item .lbl { flex: 1; }
.rail-item .cnt { color: var(--muted); font-size: 12px; }
.league { background: var(--surface); border: 1px solid var(--line); border-radius: 10px; margin-bottom: 12px; overflow: hidden; }
.league-head { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; background: var(--surface-2); font-weight: 700; font-size: 13px; }
.league-head .cols { display: grid; grid-auto-flow: column; grid-auto-columns: 64px; gap: 6px; text-align: center; font-size: 11px; font-weight: 600; }
.event { display: grid; grid-template-columns: 110px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 10px 12px; border-top: 1px solid var(--line); }
.event:hover { background: rgba(255,255,255,.02); }
.when { font-size: 11px; line-height: 1.3; }
.teams { display: grid; gap: 2px; min-width: 0; }
.teams div { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.odds { display: grid; grid-auto-flow: column; grid-auto-columns: 64px; gap: 6px; }
.odd { display: grid; gap: 0; padding: 4px 0; text-align: center; line-height: 1.2; }
.odd small { color: var(--muted); font-size: 10px; }
.odd b { font-size: 13px; }
.odd.picked { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }
.odd.picked small { color: var(--accent-ink); opacity: .8; }
.slip { position: sticky; top: 64px; }
.pickbox { border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: 6px; padding: 8px 10px; margin-bottom: 10px; }
.accent { color: var(--accent); }
.ticket { border-top: 1px solid var(--line); padding: 8px 0; }
.ticket:first-of-type { border-top: 0; }
@media (max-width: 900px) {
  .sb { grid-template-columns: 1fr; }
  .rail { position: static; display: flex; flex-wrap: wrap; }
  .slip { position: static; }
  .event { grid-template-columns: 1fr; }
  .odds { grid-auto-columns: 1fr; }
}
</style>
