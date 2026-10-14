import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api, setPlayerId } from '../api/client'
import type { Player } from '../api/types'
import { setLocale, type Locale } from '../i18n'

const KEY = 'wallet-hub.player'

function load(): Player | null {
  try { return JSON.parse(localStorage.getItem(KEY) ?? 'null') } catch { return null }
}

export const useSession = defineStore('session', () => {
  const player = ref<Player | null>(load())
  if (player.value) {
    setPlayerId(player.value.id)
    setLocale(player.value.locale as Locale)
  }

  async function login(displayName: string, locale: Locale) {
    const p = await api.post<Player>('/api/players', { display_name: displayName, locale })
    player.value = p
    setPlayerId(p.id)
    setLocale(p.locale as Locale)
    try { localStorage.setItem(KEY, JSON.stringify(p)) } catch { /* private mode */ }
  }

  async function changeLocale(locale: Locale) {
    if (!player.value) return
    const p = await api.patch<Player>('/api/players/me', { locale })
    player.value = p
    setLocale(locale)
    try { localStorage.setItem(KEY, JSON.stringify(p)) } catch { /* ignore */ }
  }

  function logout() {
    player.value = null
    setPlayerId(null)
    try { localStorage.removeItem(KEY) } catch { /* ignore */ }
  }

  return { player, login, changeLocale, logout }
})
