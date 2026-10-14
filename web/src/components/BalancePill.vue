<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useWallet } from '../stores/wallet'
import { formatMinor } from '../lib/money'

const { locale } = useI18n()
const wallet = useWallet()
const live = computed(() => formatMinor(wallet.live, wallet.currency, locale.value))
const sandbox = computed(() => formatMinor(wallet.sandbox, wallet.currency, locale.value))
</script>

<template>
  <span class="pill mono" :title="`sandbox ${sandbox}`">{{ live }}<small v-if="wallet.sandbox" class="muted"> · {{ sandbox }} sb</small></span>
</template>

<style scoped>
.pill { border: 1px solid var(--chip); border-radius: 999px; padding: 3px 12px; font-weight: 600; }
</style>
