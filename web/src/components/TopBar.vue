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
    <RouterLink to="/lobby" class="logo">WALLET HUB</RouterLink>
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
.bar { display: flex; align-items: center; gap: 14px; padding: 10px 20px; border-bottom: 1px solid var(--line); background: var(--surface); flex-wrap: wrap; position: sticky; top: 0; z-index: 5; }
.logo { font-weight: 800; letter-spacing: .06em; color: var(--fg); }
nav a { color: var(--fg); padding: 4px 8px; border-radius: 6px; }
nav a.router-link-active { background: var(--surface-2); }
.sp { flex: 1; }
</style>
