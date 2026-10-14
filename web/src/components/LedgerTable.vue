<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { Journal } from '../api/types'
import { formatDate, formatMinor, formatSigned } from '../lib/money'
import StatusChip from './StatusChip.vue'

defineProps<{ rows: Journal[]; compact?: boolean }>()
const { t, locale } = useI18n()
</script>

<template>
  <div style="overflow-x:auto">
    <table>
      <thead>
        <tr>
          <th>{{ t('ledger.date') }}</th>
          <th>{{ t('ledger.kind') }}</th>
          <th v-if="!compact">{{ t('ledger.source') }}</th>
          <th>{{ t('ledger.amount') }}</th>
          <th>{{ t('ledger.balance') }}</th>
          <th v-if="!compact">{{ t('ledger.env') }}</th>
          <th>{{ t('ledger.status') }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="j in rows" :key="j.id">
          <td class="mono">{{ formatDate(j.created_at, locale) }}</td>
          <td>{{ t(`kind.${j.kind}`, j.kind) }}<small v-if="j.meta?.game" class="muted"> · {{ j.meta.game_category }}</small></td>
          <td v-if="!compact" class="muted">{{ j.source }}</td>
          <td class="mono" :class="j.amount > 0 ? 'pos' : j.amount < 0 ? 'neg' : ''">{{ formatSigned(j.amount, j.currency, locale) }}</td>
          <td class="mono">{{ formatMinor(j.reply_balance, j.currency, locale) }}</td>
          <td v-if="!compact" class="muted">{{ j.env }}</td>
          <td><StatusChip :status="j.rolled_back ? 'rolled back' : 'booked'" :label="j.rolled_back ? t('ledger.rolled_back') : t('ledger.booked')" /></td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
