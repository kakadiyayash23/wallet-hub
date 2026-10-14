<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api, ApiError } from '../api/client'
import type { Journal, Payment } from '../api/types'
import { useWallet } from '../stores/wallet'
import { formatDate, formatMinor, normalizeAmountInput } from '../lib/money'
import LedgerTable from '../components/LedgerTable.vue'
import StatusChip from '../components/StatusChip.vue'
import EmptyState from '../components/EmptyState.vue'
import Skeleton from '../components/Skeleton.vue'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()
const wallet = useWallet()
const journals = ref<Journal[]>([])
const payments = ref<Payment[]>([])
const loading = ref(true)
const depositAmount = ref('50')
const withdrawAmount = ref('')
const error = ref('')
const busy = ref(false)

/** Return from Stripe: wait for the webhook, then show the confirmed amount. */
const returning = ref<'waiting' | 'confirmed' | 'cancelled' | null>(null)
const confirmedAmount = ref(0)
const shownBalance = ref(0)
let waitTimer: ReturnType<typeof setTimeout> | undefined
const waitedLong = ref(false)

const presets = ['10', '25', '50', '100', '250']

async function load() {
  loading.value = true
  try {
    journals.value = await wallet.ledger(50)
    payments.value = await api.get<Payment[]>('/api/wallet/payments')
  } finally {
    loading.value = false
  }
}

function countUp(to: number) {
  const from = shownBalance.value
  const start = performance.now()
  const step = (now: number) => {
    const k = Math.min(1, (now - start) / 700)
    shownBalance.value = Math.round(from + (to - from) * (1 - Math.pow(1 - k, 3)))
    if (k < 1) requestAnimationFrame(step)
  }
  requestAnimationFrame(step)
}

onMounted(async () => {
  if (route.query.deposit === 'success') {
    returning.value = 'waiting'
    waitTimer = setTimeout(() => { waitedLong.value = true }, 20_000)
    router.replace({ query: {} })
  } else if (route.query.deposit === 'cancel') {
    returning.value = 'cancelled'
    router.replace({ query: {} })
  }
  await wallet.refresh()
  shownBalance.value = wallet.live
  await load()
  // The webhook may have landed before the page did.
  if (returning.value === 'waiting' && payments.value[0]?.status === 'completed' && payments.value[0].direction === 'deposit') {
    confirm(payments.value[0].amount)
  }
})
onUnmounted(() => clearTimeout(waitTimer))

function confirm(amount: number) {
  confirmedAmount.value = amount
  returning.value = 'confirmed'
  clearTimeout(waitTimer)
  countUp(wallet.live)
}

watch(() => wallet.recent.length, async () => {
  const ev = wallet.recent[0]
  if (returning.value === 'waiting' && ev?.source === 'stripe' && ev.kind === 'deposit') confirm(ev.amount)
  else shownBalance.value = wallet.live
  await load()
})

const liveAccount = computed(() => wallet.accounts.find((a) => a.env === 'live'))
const sandboxAccount = computed(() => wallet.accounts.find((a) => a.env === 'sandbox'))

async function deposit() {
  error.value = ''
  const amount = normalizeAmountInput(depositAmount.value)
  if (!amount) { error.value = t('wallet.amount'); return }
  busy.value = true
  try {
    const res = await api.post<{ redirect_url: string }>('/api/wallet/deposits', { amount })
    location.href = res.redirect_url
  } catch (e) {
    error.value = (e as Error).message
    busy.value = false
  }
}

async function topUpSandbox() {
  error.value = ''
  busy.value = true
  try { await api.post('/api/wallet/sandbox/topup', { amount: '1000.00' }); await wallet.refresh(); await load() } catch (e) { error.value = (e as Error).message } finally { busy.value = false }
}

async function withdraw() {
  error.value = ''
  const amount = normalizeAmountInput(withdrawAmount.value)
  if (!amount) { error.value = t('wallet.amount'); return }
  busy.value = true
  try {
    await api.post('/api/wallet/withdrawals', { amount })
    withdrawAmount.value = ''
    await wallet.refresh()
    shownBalance.value = wallet.live
    await load()
  } catch (e) {
    error.value = e instanceof ApiError && e.code === 'insufficient_funds' ? e.message : (e as Error).message
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <h2 style="margin:0 0 12px">{{ t('wallet.title') }}</h2>

  <!-- Return from Stripe -->
  <div v-if="returning === 'waiting'" class="card notice">
    <span class="spinner"></span>
    <div>
      <strong>{{ t('wallet.waiting') }}</strong>
      <div v-if="waitedLong" class="muted">{{ t('wallet.waitLong') }}</div>
    </div>
  </div>
  <div v-else-if="returning === 'confirmed'" class="card notice ok">
    <span class="tick">✓</span>
    <div>
      <strong>{{ t('wallet.confirmed') }} · {{ formatMinor(confirmedAmount, wallet.currency, locale) }}</strong>
      <div class="muted">{{ t('wallet.available') }}: <span class="num">{{ formatMinor(shownBalance, wallet.currency, locale) }}</span></div>
    </div>
    <button style="margin-left:auto" @click="returning = null">✕</button>
  </div>
  <div v-else-if="returning === 'cancelled'" class="card notice">
    <span>{{ t('wallet.depositCancel') }}</span>
    <button style="margin-left:auto" @click="returning = null">✕</button>
  </div>

  <div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(300px,1fr))">
    <!-- Live -->
    <div class="card">
      <div class="muted small">{{ t('wallet.available') }}</div>
      <div class="num" style="font-size:28px;margin-bottom:10px">{{ formatMinor(liveAccount?.balance ?? 0, wallet.currency, locale) }}</div>
      <p class="sect" style="margin-top:0">{{ t('wallet.deposit') }}</p>
      <div class="row" style="margin-bottom:6px">
        <button v-for="p in presets" :key="p" :class="{ primary: depositAmount === p }" @click="depositAmount = p">{{ p }}</button>
      </div>
      <div class="row">
        <input id="deposit_amount" v-model="depositAmount" inputmode="decimal" style="width:120px" />
        <span class="muted">{{ wallet.currency }}</span>
        <button class="primary" :disabled="busy" @click="deposit">{{ t('wallet.toStripe') }} →</button>
      </div>
      <p class="muted small" style="margin:8px 0 0">{{ t('wallet.stripeNote') }}</p>
      <p class="sect">{{ t('wallet.withdraw') }}</p>
      <div class="row">
        <input id="withdraw_amount" v-model="withdrawAmount" inputmode="decimal" :placeholder="t('wallet.amount')" style="width:120px" />
        <button :disabled="busy" @click="withdraw">{{ t('wallet.request') }}</button>
      </div>
      <p v-if="error" class="error">{{ error }}</p>
    </div>
    <!-- Sandbox -->
    <div class="card">
      <div class="muted small">{{ t('wallet.sandbox') }}</div>
      <div class="num" style="font-size:28px;margin-bottom:10px">{{ formatMinor(sandboxAccount?.balance ?? 0, wallet.currency, locale) }}</div>
      <p class="muted" style="margin:0 0 10px">{{ t('wallet.sandboxHint') }}</p>
      <button :disabled="busy" @click="topUpSandbox">{{ t('wallet.topupSandbox') }}</button>
    </div>
  </div>

  <p class="sect">{{ t('wallet.payments') }}</p>
  <Skeleton v-if="loading" :count="3" />
  <EmptyState v-else-if="payments.length === 0" :title="t('wallet.noPayments')" :hint="t('wallet.noPaymentsHint')" />
  <table v-else>
    <thead><tr><th>{{ t('ledger.date') }}</th><th>{{ t('ledger.kind') }}</th><th>{{ t('ledger.amount') }}</th><th>{{ t('ledger.status') }}</th></tr></thead>
    <tbody>
      <tr v-for="p in payments" :key="p.id">
        <td class="mono">{{ formatDate(p.created_at, locale) }}</td>
        <td>{{ t(`kind.${p.direction}`) }} · {{ p.provider }}</td>
        <td class="mono">{{ formatMinor(p.amount, p.currency, locale) }}</td>
        <td><StatusChip :status="p.status" :label="t(`wallet.${p.status}`, p.status)" /></td>
      </tr>
    </tbody>
  </table>

  <p class="sect">{{ t('wallet.transactions') }}</p>
  <Skeleton v-if="loading" :count="4" />
  <EmptyState v-else-if="journals.length === 0" :title="t('wallet.noLedger')" :hint="t('wallet.noLedgerHint')" />
  <LedgerTable v-else :rows="journals" />
</template>

<style scoped>
.small { font-size: 12.5px; }
.notice { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; border-color: var(--chip); }
.notice.ok { border-color: var(--ok); }
.tick { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 50%; background: var(--ok); color: var(--accent-ink); font-weight: 800; }
.spinner { width: 20px; height: 20px; border: 3px solid var(--line); border-top-color: var(--accent); border-radius: 50%; animation: spin .9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg) } }
</style>
