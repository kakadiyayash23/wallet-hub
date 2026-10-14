<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { api, ApiError } from '../api/client'
import type { ChaosReport, EventStats, InboundEvent, Reconciliation, SourceHealth } from '../api/types'
import { formatDate, formatMinor } from '../lib/money'
import { useSession } from '../stores/session'
import StatusChip from '../components/StatusChip.vue'

const { t, locale } = useI18n()
const session = useSession()
const key = ref('')
const tab = ref<'callbacks' | 'health' | 'reconcile' | 'chaos'>('callbacks')
const events = ref<InboundEvent[]>([])
const stats = ref<EventStats | null>(null)
const health = ref<SourceHealth[]>([])
const selected = ref<InboundEvent | null>(null)
const players = ref<{ id: string; external_token: string; display_name: string }[]>([])
const playerId = ref('')
const recon = ref<Reconciliation | null>(null)
const chaosRounds = ref(200)
const chaosDup = ref(5)
const chaosBad = ref(0)
const chaosReport = ref<ChaosReport | null>(null)
const error = ref('')
const busy = ref(false)
const currency = 'EUR'

onMounted(async () => {
  await session.boot()
  if (session.ops) refresh()
})

async function login() {
  error.value = ''
  try {
    await session.opsLogin(key.value)
    key.value = ''
    await refresh()
  } catch (e) {
    error.value = e instanceof ApiError && e.status === 401 ? t('ops.badKey') : (e as Error).message
  }
}

async function refresh() {
  error.value = ''
  try {
    stats.value = await api.get<EventStats>('/api/ops/events/stats')
    events.value = await api.get<InboundEvent[]>('/api/ops/events?limit=100')
    health.value = await api.get<SourceHealth[]>('/api/ops/health')
    players.value = await api.get('/api/ops/players')
    if (!playerId.value && players.value[0]) playerId.value = players.value[0].id
  } catch (e) {
    error.value = (e as Error).message
  }
}

async function replay(ev: InboundEvent) {
  busy.value = true
  try { await api.post(`/api/ops/events/${ev.id}/replay`); await refresh() } catch (e) { error.value = (e as Error).message } finally { busy.value = false }
}

async function sync() {
  busy.value = true
  try { const r = await api.post<{ synced: number }>('/api/ops/catalog/sync'); error.value = `synced ${r.synced} games` } catch (e) { error.value = (e as Error).message } finally { busy.value = false }
}

async function reconcile() {
  busy.value = true
  try { recon.value = await api.get<Reconciliation>(`/api/ops/reconcile/${playerId.value}`) } catch (e) { error.value = (e as Error).message } finally { busy.value = false }
}

async function chaos() {
  busy.value = true
  chaosReport.value = null
  try {
    chaosReport.value = await api.post<ChaosReport>('/api/ops/chaos', { player_id: playerId.value, rounds: chaosRounds.value, duplicate_every: chaosDup.value, bad_signature_every: chaosBad.value, max_delay_ms: 20, shuffle: true })
    await refresh()
  } catch (e) { error.value = (e as Error).message } finally { busy.value = false }
}

function pretty(body: string) { try { return JSON.stringify(JSON.parse(body), null, 2) } catch { return body } }
const ms = (v: number | null) => (v == null ? '—' : Math.round(v).toString())
const healthTone = (h: SourceHealth) => (h.errors_24h > 0 ? 'bad' : h.rejected_24h > 0 ? 'warn' : 'ok')
</script>

<template>
  <div class="row" style="margin-bottom:12px">
    <h2 style="margin:0">{{ t('ops.title') }}</h2>
    <span style="flex:1"></span>
    <template v-if="session.ops">
      <span class="chip ok">{{ t('ops.loggedIn') }}</span>
      <button :disabled="busy" @click="sync">{{ t('ops.sync') }}</button>
      <button @click="refresh">↻</button>
    </template>
    <template v-else>
      <input id="ops_key" v-model="key" type="password" :placeholder="t('ops.key')" @keyup.enter="login" />
      <button class="primary" @click="login">{{ t('ops.login') }}</button>
    </template>
  </div>
  <p v-if="error" class="error">{{ error }}</p>
  <div v-if="!session.ops" class="card" style="border-color:var(--chip);margin-bottom:12px">
    <strong>{{ t('ops.needKey') }}</strong>
    <p class="muted" style="margin:4px 0 0">{{ t('ops.needKeyHint') }}</p>
  </div>

  <template v-if="session.ops">
    <div class="row" style="margin-bottom:12px">
      <button :class="{ primary: tab === 'callbacks' }" @click="tab = 'callbacks'">{{ t('ops.callbacks') }}</button>
      <button :class="{ primary: tab === 'health' }" @click="tab = 'health'">{{ t('ops.health') }}</button>
      <button :class="{ primary: tab === 'reconcile' }" @click="tab = 'reconcile'">{{ t('ops.reconcile') }}</button>
      <button :class="{ primary: tab === 'chaos' }" @click="tab = 'chaos'">{{ t('ops.chaos') }}</button>
    </div>

    <template v-if="tab === 'callbacks'">
      <div v-if="stats" class="grid" style="grid-template-columns:repeat(auto-fit,minmax(160px,1fr));margin-bottom:12px">
        <div class="card"><div class="mono" style="font-size:22px">{{ stats.received }}</div><div class="muted">{{ t('ops.received') }}</div></div>
        <div class="card"><div class="mono" style="font-size:22px">{{ stats.duplicates }}</div><div class="muted">{{ t('ops.duplicates') }}</div></div>
        <div class="card"><div class="mono" style="font-size:22px">{{ stats.rejected }}</div><div class="muted">{{ t('ops.rejected') }}</div></div>
        <div class="card"><div class="mono" style="font-size:22px">{{ stats.errors }}</div><div class="muted">{{ t('ops.errors') }}</div></div>
      </div>
      <div style="overflow-x:auto">
        <table>
          <thead><tr><th>Time</th><th>Source</th><th>Path</th><th>Sig</th><th>Outcome</th><th>ms</th><th>Detail</th><th></th></tr></thead>
          <tbody>
            <tr v-for="ev in events" :key="ev.id">
              <td class="mono">{{ new Date(ev.received_at).toLocaleTimeString(locale) }}</td>
              <td>{{ ev.source }}<small v-if="ev.replay_of" class="muted"> replay</small></td>
              <td class="mono muted">{{ ev.path.split('?')[0].split('/').pop() }}</td>
              <td><span v-if="ev.signature_ok === false" class="chip bad">bad</span><span v-else-if="ev.signature_ok" class="chip ok">ok</span></td>
              <td><StatusChip :status="ev.outcome" /></td>
              <td class="mono muted">{{ ev.duration_ms ?? '' }}</td>
              <td class="muted" style="max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{{ ev.detail }}</td>
              <td class="row"><button @click="selected = ev">{{ t('ops.view') }}</button><button :disabled="busy" @click="replay(ev)">{{ t('ops.replay') }}</button></td>
            </tr>
          </tbody>
        </table>
      </div>
      <pre v-if="selected">{{ selected.method }} {{ selected.path }}
{{ pretty(selected.body) }}</pre>
    </template>

    <template v-if="tab === 'health'">
      <div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(300px,1fr))">
        <div v-for="h in health" :key="h.source" class="card">
          <div class="row" style="justify-content:space-between">
            <strong style="text-transform:capitalize">{{ h.source }}</strong>
            <span class="chip" :class="healthTone(h)">{{ healthTone(h) === 'ok' ? 'OK' : healthTone(h) === 'warn' ? 'rejected' : 'errors' }}</span>
          </div>
          <div class="kv">
            <span class="muted">{{ t('ops.received24') }}</span><span class="mono">{{ h.received_24h }}</span>
            <span class="muted">applied / dup / rej / err</span><span class="mono">{{ h.applied_24h }} / {{ h.duplicates_24h }} / {{ h.rejected_24h }} / {{ h.errors_24h }}</span>
            <span class="muted">{{ t('ops.latency') }}</span><span class="mono">{{ ms(h.avg_duration_ms) }} / {{ ms(h.p95_duration_ms) }}</span>
            <span class="muted">{{ t('ops.lastSuccess') }}</span><span class="mono">{{ h.last_success_at ? formatDate(h.last_success_at, locale) : t('ops.never') }}</span>
            <span class="muted">{{ t('ops.lastError') }}</span><span class="mono">{{ h.last_error_at ? formatDate(h.last_error_at, locale) : t('ops.never') }}</span>
          </div>
          <p v-if="h.last_error_detail" class="muted" style="font-size:12px;margin:6px 0 0">{{ h.last_error_detail }}</p>
        </div>
      </div>
      <p v-if="health.length === 0" class="muted">—</p>
    </template>

    <template v-if="tab === 'reconcile'">
      <div class="row" style="margin-bottom:10px">
        <select id="recon_player" v-model="playerId"><option v-for="p in players" :key="p.id" :value="p.id">{{ p.display_name }} ({{ p.external_token }})</option></select>
        <button class="primary" :disabled="busy || !playerId" @click="reconcile">{{ t('ops.run') }}</button>
      </div>
      <div v-if="recon" class="grid" style="grid-template-columns:repeat(auto-fit,minmax(160px,1fr))">
        <div class="card"><div class="mono" style="font-size:20px">{{ formatMinor(recon.provider_net, currency, locale) }}</div><div class="muted">{{ t('ops.providerSays') }} · {{ recon.provider_rows }} rows</div></div>
        <div class="card"><div class="mono" style="font-size:20px">{{ formatMinor(recon.ledger_net, currency, locale) }}</div><div class="muted">{{ t('ops.ledgerSays') }} · {{ recon.env }}</div></div>
        <div class="card"><div class="mono" style="font-size:20px" :class="recon.drift === 0 ? 'pos' : 'neg'">{{ formatMinor(recon.drift, currency, locale) }}</div><div class="muted">{{ t('ops.drift') }}</div></div>
      </div>
    </template>

    <template v-if="tab === 'chaos'">
      <div class="card" style="margin-bottom:12px">
        <div class="row">
          <select id="chaos_player" v-model="playerId"><option v-for="p in players" :key="p.id" :value="p.id">{{ p.display_name }}</option></select>
          <label>rounds <input id="chaos_rounds" v-model.number="chaosRounds" type="number" min="1" max="2000" style="width:90px" /></label>
          <label>duplicate every <input id="chaos_dup" v-model.number="chaosDup" type="number" min="0" style="width:70px" /></label>
          <label>bad signature every <input id="chaos_bad" v-model.number="chaosBad" type="number" min="0" style="width:70px" /></label>
          <button class="primary" :disabled="busy || !playerId" @click="chaos">{{ busy ? t('common.loading') : t('ops.run') }}</button>
        </div>
      </div>
      <div v-if="chaosReport" class="grid" style="grid-template-columns:repeat(auto-fit,minmax(160px,1fr))">
        <div class="card"><div class="mono" style="font-size:20px">{{ chaosReport.sent }}</div><div class="muted">callbacks sent · {{ chaosReport.elapsed_ms }} ms</div></div>
        <div class="card"><div class="mono" style="font-size:20px">{{ chaosReport.duplicates_rejected }} / {{ chaosReport.duplicates_sent }}</div><div class="muted">duplicates ignored</div></div>
        <div class="card"><div class="mono" style="font-size:20px">{{ chaosReport.bad_signatures_rejected }} / {{ chaosReport.bad_signatures_sent }}</div><div class="muted">bad signatures rejected</div></div>
        <div class="card"><div class="mono" style="font-size:20px" :class="chaosReport.drift === 0 ? 'pos' : 'neg'">{{ formatMinor(chaosReport.drift, currency, locale) }}</div><div class="muted">ledger drift · trial balance {{ chaosReport.trial_balance }}</div></div>
      </div>
    </template>
  </template>
</template>

<style scoped>
.kv { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; margin-top: 8px; font-size: 13px; }
</style>
