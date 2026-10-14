<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { TimelineBucket } from '../api/types'

const props = defineProps<{ buckets: TimelineBucket[] }>()
const { locale } = useI18n()
const hover = ref<number | null>(null)

/** Status series in fixed order; colours come from the status tokens, never cycled. */
const series = [
  { key: 'applied', label: 'applied', color: 'var(--ok)' },
  { key: 'duplicates', label: 'duplicate', color: 'var(--info)' },
  { key: 'rejected', label: 'rejected', color: 'var(--warn)' },
  { key: 'errors', label: 'error', color: 'var(--bad)' },
] as const

const W = 720, H = 180, padL = 34, padB = 22, padT = 8, gap = 2
const max = computed(() => Math.max(1, ...props.buckets.map((b) => b.applied + b.duplicates + b.rejected + b.errors)))
/** About four gridlines with a round step (1, 2, 5 × 10^n). */
const ticks = computed(() => {
  const m = max.value
  const raw = m / 4
  const pow = Math.pow(10, Math.floor(Math.log10(Math.max(1, raw))))
  const step = [1, 2, 5, 10].map((k) => k * pow).find((v) => v >= raw) ?? pow
  const out: number[] = []
  for (let v = 0; v <= m; v += step) out.push(v)
  return out
})
const bw = computed(() => (W - padL) / Math.max(1, props.buckets.length))
const y = (v: number) => padT + (H - padT - padB) * (1 - v / max.value)

const bars = computed(() =>
  props.buckets.map((b, i) => {
    let acc = 0
    const segs = series.map((s) => {
      const v = b[s.key]
      const y0 = y(acc + v), y1 = y(acc)
      acc += v
      return { key: s.key, color: s.color, y: y0, h: Math.max(0, y1 - y0 - (v > 0 ? gap : 0)), v }
    })
    return { x: padL + i * bw.value + 2, w: bw.value - 4, segs, total: acc, hour: b.hour }
  }),
)
const hourLabel = (iso: string) => new Intl.DateTimeFormat(locale.value, { hour: '2-digit' }).format(new Date(iso))
const totals = computed(() => series.map((s) => ({ ...s, total: props.buckets.reduce((a, b) => a + b[s.key], 0) })))
</script>

<template>
  <div class="chart">
    <div class="legend">
      <span v-for="s in totals" :key="s.key" class="lg"><i :style="{ background: s.color }"></i>{{ s.label }} <b class="mono">{{ s.total }}</b></span>
    </div>
    <svg :viewBox="`0 0 ${W} ${H}`" width="100%" role="img" aria-label="Callbacks per hour by outcome" @mouseleave="hover = null">
      <g v-for="tk in ticks" :key="tk">
        <line :x1="padL" :x2="W" :y1="y(tk)" :y2="y(tk)" stroke="var(--line)" stroke-width="1" />
        <text :x="padL - 6" :y="y(tk) + 4" text-anchor="end" font-size="10" fill="var(--muted)" font-family="var(--mono)">{{ tk }}</text>
      </g>
      <g v-for="(b, i) in bars" :key="b.hour" @mousemove="hover = i">
        <rect :x="b.x - 2" :y="padT" :width="b.w + 4" :height="H - padT - padB" fill="transparent" />
        <rect v-for="s in b.segs" :key="s.key" :x="b.x" :y="s.y" :width="b.w" :height="s.h" :fill="s.color" rx="2" :opacity="hover === null || hover === i ? 1 : 0.45" />
        <text v-if="i % 3 === 0" :x="b.x + b.w / 2" :y="H - 6" text-anchor="middle" font-size="10" fill="var(--muted)" font-family="var(--mono)">{{ hourLabel(b.hour) }}</text>
      </g>
    </svg>
    <div v-if="hover !== null && bars[hover]" class="tip">
      <b>{{ hourLabel(bars[hover].hour) }}</b> · {{ bars[hover].total }}
      <span v-for="s in bars[hover].segs" :key="s.key" class="lg"><i :style="{ background: s.color }"></i>{{ s.v }}</span>
    </div>
    <div v-else class="tip muted">—</div>
  </div>
</template>

<style scoped>
.chart { position: relative; }
.legend, .tip { display: flex; gap: 14px; flex-wrap: wrap; font-size: 12.5px; margin: 4px 0 6px; }
.tip { min-height: 20px; margin-top: 4px; }
.lg { display: inline-flex; align-items: center; gap: 6px; }
.lg i { width: 10px; height: 10px; border-radius: 2px; display: inline-block; }
</style>
