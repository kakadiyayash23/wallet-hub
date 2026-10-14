<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import TopBar from './components/TopBar.vue'
import Toasts from './components/Toasts.vue'
import { useSession } from './stores/session'
import { useWallet } from './stores/wallet'
import { useToasts } from './stores/toasts'
import { connectLive } from './live/ws'
import { formatSigned } from './lib/money'
import type { LiveEvent } from './api/types'

const session = useSession()
const wallet = useWallet()
const toasts = useToasts()
const route = useRoute()
const { t, locale } = useI18n()
let disconnect: (() => void) | null = null

function announce(ev: LiveEvent) {
  const amount = formatSigned(ev.amount, ev.currency, locale.value)
  const where = ev.env === 'sandbox' ? ` · ${t('wallet.sandbox')}` : ''
  if (ev.source === 'stripe' && ev.kind === 'deposit') return toasts.push('ok', t('toast.deposit', { amount }), where)
  if (ev.source === 'sports' && ev.kind === 'win' && ev.amount > 0) return toasts.push('ok', t('toast.betWon', { amount }))
  if (ev.source === 'sports' && ev.kind === 'win') return toasts.push('info', t('toast.betLost'))
  if (ev.source === 'sports' && ev.kind === 'cashout') return toasts.push('ok', t('toast.cashout', { amount }))
  if (ev.source === 'bigbang' && ev.amount > 0) return toasts.push('ok', t('toast.win', { amount }), where)
  if (ev.source === 'bigbang') return undefined
  return toasts.push('info', `${ev.kind} ${amount}`, where)
}

function start() {
  disconnect?.()
  disconnect = null
  if (!session.player) return
  wallet.refresh().catch(() => {})
  disconnect = connectLive((ev) => {
    wallet.applyLive(ev)
    announce(ev)
  })
}

onMounted(start)
watch(() => session.player?.id, start)
onUnmounted(() => disconnect?.())
</script>

<template>
  <TopBar v-if="route.path !== '/login'" />
  <main class="page">
    <RouterView />
  </main>
  <Toasts />
</template>
