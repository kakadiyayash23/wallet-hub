<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { api, setOpsKey } from '../api/client'
import type { ChaosReport, EventStats, InboundEvent, Reconciliation } from '../api/types'
import { formatMinor } from '../lib/money'
import StatusChip from '../components/StatusChip.vue'

const { t, locale } = useI18n()
const KEY = 'wallet-hub.opsKey'
const key = ref('')
const hasKey = ref(false)
const tab = ref<'callbacks' | 'reconcile' | 'chaos'>('callbacks')
const events = ref<InboundEvent[]>([])
const stats = ref<EventStats | null>(null)
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

onMounted(() => {
  try { key.value = localStorage.getItem(KEY) ?? '' } catch { /* ignore */ }
  if (key.value) { setOpsKey(key.value); hasKey.value = true; refresh() }
})

function saveKey() {
  setOpsKey(key.value)
  hasKey.value = key.value.trim().length > 0
  try { localStorage.setItem(KEY, key.value) } catch { /* ignore */ }
  refresh()
}

async function refresh() {
  error.value = ''
  try {
    stats.value = await api.get<EventStats>('/api/ops/events/stats')
    events.value = await api.get<InboundEvent[]>('/api/ops/events?limit=100')
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
</script>

<template>
  <div class="row" style="margin-bottom:12px">
    <h2 style="margin:0">{{ t('ops.title') }}</h2>
    <span style="flex:1"></span>
    <input id="ops_key" v-model="key" type="password" :placeholder="t('ops.key')" @keyup.enter="saveKey" />
    <button @click="saveKey">OK</button>
    <button :disabled="busy" @click="sync">{{ t('ops.sync') }}</button>
  </div>
  <div class="row" style="margin-bottom:12px">
    <button :class="{ primary: tab === 'callbacks' }" @click="tab = 'callbacks'">{{ t('ops.callbacks') }}</button>
    <button :class="{ primary: tab === 'reconcile' }" @click="tab = 'reconcile'">{{ t('ops.reconcile') }}</button>
    <button :class="{ primary: tab === 'chaos' }" @click="tab = 'chaos'">{{ t('ops.chaos') }}</button>
  </div>
  <p v-if="error" class="error">{{ error }}</p>
  <div v-if="!hasKey" class="card" style="border-color:var(--chip);margin-bottom:12px">
    <strong>{{ t('ops.needKey') }}</strong>
    <p class="muted" style="margin:4px 0 0">{{ t('ops.needKeyHint') }}</p>
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
        <thead><tr><th>Time</th><th>Source</th><th>Path</th><th>Sig</th><th>Outcome</th><th>Detail</th><th></th></tr></thead>
        <tbody>
          <tr v-for="ev in events" :key="ev.id">
            <td class="mono">{{ new Date(ev.received_at).toLocaleTimeString(locale) }}</td>
            <td>{{ ev.source }}<small v-if="ev.replay_of" class="muted"> replay</small></td>
            <td class="mono muted">{{ ev.path.split('?')[0].split('/').pop() }}</td>
            <td><span v-if="ev.signature_ok === false" class="chip bad">bad</span><span v-else-if="ev.signature_ok" class="chip ok">ok</span></td>
            <td><StatusChip :status="ev.outcome" /></td>
            <td class="muted" style="max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{{ ev.detail }}</td>
            <td class="row"><button @click="selected = ev">{{ t('ops.view') }}</button><button :disabled="busy" @click="replay(ev)">{{ t('ops.replay') }}</button></td>
          </tr>
        </tbody>
      </table>
    </div>
    <pre v-if="selected">{{ selected.method }} {{ selected.path }}
{{ pretty(selected.body) }}</pre>
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
