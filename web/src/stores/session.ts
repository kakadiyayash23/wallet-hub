import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api/client'
import type { Player, Whoami } from '../api/types'
import { setLocale, type Locale } from '../i18n'

/** Who is logged in. The truth lives in the session cookie; this store mirrors it. */
export const useSession = defineStore('session', () => {
  const player = ref<Player | null>(null)
  const ops = ref(false)
  let booted: Promise<void> | null = null

  /** Ask the api who the cookie belongs to. Called once before routing. */
  function boot(): Promise<void> {
    if (!booted) {
      booted = api
        .get<Whoami>('/api/auth/whoami')
        .then((w) => {
          player.value = w.player
          ops.value = w.ops
          if (w.player) setLocale(w.player.locale as Locale)
        })
        .catch(() => {
          player.value = null
          ops.value = false
        })
    }
    return booted
  }

  async function login(displayName: string, locale: Locale) {
    const p = await api.post<Player>('/api/players', { display_name: displayName, locale })
    player.value = p
    setLocale(p.locale as Locale)
  }

  async function changeLocale(locale: Locale) {
    if (!player.value) return
    const p = await api.patch<Player>('/api/players/me', { locale })
    player.value = p
    setLocale(locale)
  }

  async function opsLogin(key: string) {
    await api.post('/api/ops/login', { key })
    ops.value = true
  }

  async function logout() {
    try { await api.post('/api/auth/logout') } catch { /* cookie may already be gone */ }
    player.value = null
    ops.value = false
  }

  return { player, ops, boot, login, changeLocale, opsLogin, logout }
})
