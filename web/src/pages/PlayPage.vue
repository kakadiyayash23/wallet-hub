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
const rounds = computed(() => wallet.recent.filter((e) => e.source === 'bigbang').slice(0, 12))

onMounted(async () => {
  try {
    const res = await api.post<{ url: string }>('/api/casino/launch', { game_id: Number(route.params.gameId), demo: demo.value })
    url.value = res.url
  } catch (e) {
    error.value = (e as Error).message
  }
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
  <table v-if="rounds.length">
    <thead><tr><th>Journal</th><th>{{ t('ledger.kind') }}</th><th>{{ t('ledger.amount') }}</th><th>{{ t('ledger.balance') }}</th></tr></thead>
    <tbody>
      <tr v-for="(r, i) in rounds" :key="r.journal_id ?? i">
        <td class="mono muted">…{{ (r.journal_id ?? '').slice(-6) }}</td>
        <td>{{ t(`kind.${r.kind}`, r.kind) }}</td>
        <td class="mono" :class="r.amount > 0 ? 'pos' : r.amount < 0 ? 'neg' : ''">{{ formatSigned(r.amount, r.currency, locale) }}</td>
        <td class="mono">{{ formatMinor(r.balance, r.currency, locale) }}</td>
      </tr>
    </tbody>
  </table>
  <p v-else class="muted">—</p>
</template>

<style scoped>
.frame { aspect-ratio: 16 / 9; max-height: 70vh; background: #000; border-radius: 10px; overflow: hidden; border: 1px solid var(--line); }
iframe { width: 100%; height: 100%; border: 0; }
</style>
