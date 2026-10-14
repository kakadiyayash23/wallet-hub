<script setup lang="ts">
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useSession } from '../stores/session'
import BalancePill from './BalancePill.vue'
import type { Locale } from '../i18n'

const { t } = useI18n()
const session = useSession()
const router = useRouter()

async function onLocale(e: Event) {
  await session.changeLocale((e.target as HTMLSelectElement).value as Locale)
}
async function logout() {
  await session.logout()
  router.push('/login')
}
</script>

<template>
  <header class="bar">
    <RouterLink to="/lobby" class="logo" aria-label="Wallet Hub">
      <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10" fill="var(--chip)"/><circle cx="12" cy="12" r="6.5" fill="none" stroke="var(--accent-ink)" stroke-width="1.6" stroke-dasharray="3 2.2"/><circle cx="12" cy="12" r="2.4" fill="var(--accent-ink)"/></svg>
      <span>Wallet<b>Hub</b></span>
    </RouterLink>
    <nav class="row">
      <RouterLink to="/lobby">{{ t('nav.casino') }}</RouterLink>
      <RouterLink to="/sports">{{ t('nav.sports') }}</RouterLink>
      <RouterLink to="/wallet">{{ t('nav.wallet') }}</RouterLink>
      <RouterLink to="/ops" class="muted">{{ t('nav.ops') }}</RouterLink>
    </nav>
    <span class="sp"></span>
    <template v-if="session.player">
      <BalancePill />
      <RouterLink to="/wallet"><button class="primary">{{ t('nav.deposit') }}</button></RouterLink>
      <select id="locale" :value="session.player.locale" @change="onLocale" aria-label="Language">
        <option value="en">EN</option>
        <option value="de">DE</option>
      </select>
      <button @click="logout" :title="t('nav.logout')">{{ session.player.display_name }} ✕</button>
    </template>
  </header>
</template>

<style scoped>
.bar { display: flex; align-items: center; gap: 14px; padding: 10px 22px; border-bottom: 1px solid var(--line); background: rgba(27,29,34,.92); backdrop-filter: blur(8px); flex-wrap: wrap; position: sticky; top: 0; z-index: 5; }
.logo { display: inline-flex; align-items: center; gap: 8px; font-family: var(--display); font-weight: 600; font-size: 17px; color: var(--fg); letter-spacing: .01em; }
.logo b { color: var(--accent); font-weight: 800; }
nav a { color: var(--fg); padding: 5px 9px; border-radius: 7px; font-family: var(--display); font-weight: 500; }
nav a.router-link-active { background: var(--surface-2); }
.sp { flex: 1; }
</style>
