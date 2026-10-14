import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api/client'
import type { Account, Journal, LiveEvent } from '../api/types'

/** Single source of truth for balances. The WebSocket feeds `applyLive`. */
export const useWallet = defineStore('wallet', () => {
  const accounts = ref<Account[]>([])
  const currency = ref('EUR')
  const recent = ref<LiveEvent[]>([])

  const live = computed(() => accounts.value.find((a) => a.env === 'live')?.balance ?? 0)
  const sandbox = computed(() => accounts.value.find((a) => a.env === 'sandbox')?.balance ?? 0)

  async function refresh() {
    const s = await api.get<{ accounts: Account[]; currency: string }>('/api/wallet')
    accounts.value = s.accounts
    currency.value = s.currency
  }

  async function ledger(limit = 50) {
    return api.get<Journal[]>(`/api/wallet/ledger?limit=${limit}`)
  }

  function applyLive(ev: LiveEvent) {
    const acc = accounts.value.find((a) => a.env === ev.env && a.currency === ev.currency)
    if (acc) acc.balance = ev.balance
    else accounts.value.push({ id: ev.journal_id ?? ev.env, currency: ev.currency, env: ev.env, balance: ev.balance })
    recent.value.unshift(ev)
    if (recent.value.length > 50) recent.value.pop()
  }

  return { accounts, currency, recent, live, sandbox, refresh, ledger, applyLive }
})
