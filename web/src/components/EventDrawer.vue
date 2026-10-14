<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '../api/client'
import type { InboundEvent, Journal } from '../api/types'
import { useToasts } from '../stores/toasts'
import { formatDate, formatSigned, formatMinor } from '../lib/money'
import StatusChip from './StatusChip.vue'

const props = defineProps<{ event: InboundEvent }>()
defineEmits<{ close: [] }>()
const { t, locale } = useI18n()
const toasts = useToasts()
const journal = ref<Journal | null>(null)

async function loadJournal() {
  journal.value = null
  if (props.event.journal_id) {
    try { journal.value = await api.get<Journal>(`/api/ops/journals/${props.event.journal_id}`) } catch { journal.value = null }
  }
}
onMounted(loadJournal)
watch(() => props.event.id, loadJournal)

const pretty = (v: unknown) => { try { return JSON.stringify(typeof v === 'string' ? JSON.parse(v) : v, null, 2) } catch { return String(v ?? '') } }
async function copy(text: string) {
  try { await navigator.clipboard.writeText(text); toasts.push('ok', t('ops.copied')) } catch { toasts.push('warn', t('common.error')) }
}
</script>

<template>
  <aside class="drawer">
    <div class="row" style="justify-content:space-between;margin-bottom:8px">
      <strong>{{ event.source }} · {{ event.path.split('?')[0].split('/').pop() }}</strong>
      <button @click="$emit('close')">{{ t('ops.close') }}</button>
    </div>
    <div class="row" style="gap:8px;margin-bottom:10px">
      <StatusChip :status="event.outcome" />
      <span v-if="event.signature_ok === false" class="chip bad">signature bad</span>
      <span v-else-if="event.signature_ok" class="chip ok">signature ok</span>
      <span class="mono muted">{{ formatDate(event.received_at, locale) }}</span>
      <span v-if="event.duration_ms != null" class="mono muted">{{ event.duration_ms }} ms</span>
    </div>
    <p v-if="event.detail" class="error" style="margin:0 0 10px">{{ event.detail }}</p>

    <div class="row" style="justify-content:space-between"><span class="sect" style="margin:0">{{ t('ops.request') }}</span><button @click="copy(event.body)">{{ t('ops.copy') }}</button></div>
    <pre>{{ event.method }} {{ event.path }}
{{ pretty(event.body) }}</pre>

    <div class="row" style="justify-content:space-between"><span class="sect" style="margin:0">{{ t('ops.reply') }} <span class="mono" v-if="event.reply_status">{{ event.reply_status }}</span></span><button @click="copy(pretty(event.reply_body))">{{ t('ops.copy') }}</button></div>
    <pre>{{ event.reply_body ? pretty(event.reply_body) : '—' }}</pre>

    <span class="sect">{{ t('ops.journal') }}</span>
    <div v-if="journal" class="card" style="font-size:13px">
      <div class="row" style="justify-content:space-between">
        <span>{{ t(`kind.${journal.kind}`, journal.kind) }} · {{ journal.env }}</span>
        <span class="num" :class="journal.amount > 0 ? 'pos' : journal.amount < 0 ? 'neg' : ''">{{ formatSigned(journal.amount, journal.currency, locale) }}</span>
      </div>
      <div class="muted">{{ t('ledger.balance') }} {{ formatMinor(journal.reply_balance, journal.currency, locale) }} · {{ journal.idempotency_key }}</div>
      <div v-if="journal.round_id" class="muted mono">round {{ journal.round_id }}<span v-if="journal.round_end"> · round_end</span></div>
    </div>
    <p v-else class="muted" style="font-size:13px">{{ t('ops.noJournal') }}</p>
  </aside>
</template>

<style scoped>
.drawer { position: fixed; top: 0; right: 0; height: 100vh; width: min(520px, 100vw); background: var(--surface); border-left: 1px solid var(--line); box-shadow: var(--shadow); padding: 16px; overflow-y: auto; z-index: 20; }
pre { max-height: 260px; }
</style>
