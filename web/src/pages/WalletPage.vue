<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { api, ApiError } from '../api/client'
import type { Journal, Payment } from '../api/types'
import { useWallet } from '../stores/wallet'
import { formatDate, formatMinor, normalizeAmountInput } from '../lib/money'
import LedgerTable from '../components/LedgerTable.vue'
import StatusChip from '../components/StatusChip.vue'

const { t, locale } = useI18n()
const route = useRoute()
const wallet = useWallet()
const journals = ref<Journal[]>([])
const payments = ref<Payment[]>([])
const depositAmount = ref('50')
const withdrawAmount = ref('')
const error = ref('')
const notice = ref('')
const busy = ref(false)

async function load() {
  journals.value = await wallet.ledger(50)
  payments.value = await api.get<Payment[]>('/api/wallet/payments')
}

onMounted(async () => {
  if (route.query.deposit === 'success') notice.value = t('wallet.depositSuccess')
  if (route.query.deposit === 'cancel') notice.value = t('wallet.depositCancel')
  await wallet.refresh()
  await load()
})
watch(() => wallet.recent.length, load)

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
  } finally {
    busy.value = false
  }
}

async function topUpSandbox() {
  error.value = ''
  busy.value = true
  try {
    await api.post('/api/wallet/sandbox/topup', { amount: '1000.00' })
    await wallet.refresh()
    await load()
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
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
    await load()
  } catch (e) {
    error.value = e instanceof ApiError && e.code === 'insufficient_funds' ? `${e.message}` : (e as Error).message
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <h2 style="margin:0 0 12px">{{ t('wallet.title') }}</h2>
  <p v-if="notice" class="card" style="border-color:var(--chip)">{{ notice }}</p>
  <div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(260px,1fr))">
    <div class="card">
      <p class="sect" style="margin-top:0">{{ t('wallet.deposit') }}</p>
      <div class="row">
        <input id="deposit_amount" v-model="depositAmount" inputmode="decimal" style="width:120px" />
        <span class="muted">{{ wallet.currency }}</span>
        <button class="primary" :disabled="busy" @click="deposit">{{ t('wallet.toStripe') }} →</button>
      </div>
      <p class="sect">{{ t('wallet.withdraw') }}</p>
      <div class="row">
        <input id="withdraw_amount" v-model="withdrawAmount" inputmode="decimal" :placeholder="t('wallet.amount')" style="width:120px" />
        <button :disabled="busy" @click="withdraw">{{ t('wallet.request') }}</button>
      </div>
      <p v-if="error" class="error">{{ error }}</p>
    </div>
    <div class="card">
      <div v-for="a in wallet.accounts" :key="a.id" style="margin-bottom:8px">
        <div class="mono" style="font-size:22px">{{ formatMinor(a.balance, a.currency, locale) }}</div>
        <div class="row">
          <span class="muted">{{ a.env === 'live' ? t('wallet.available') : t('wallet.sandbox') }}</span>
          <button v-if="a.env === 'sandbox'" :disabled="busy" @click="topUpSandbox">{{ t('wallet.topupSandbox') }}</button>
        </div>
      </div>
    </div>
  </div>

  <p class="sect">{{ t('wallet.payments') }}</p>
  <table v-if="payments.length">
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
  <p v-else class="muted">—</p>

  <p class="sect">{{ t('wallet.transactions') }}</p>
  <LedgerTable :rows="journals" />
</template>
