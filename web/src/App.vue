<script setup lang="ts">
import { onMounted, onUnmounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import TopBar from './components/TopBar.vue'
import { useSession } from './stores/session'
import { useWallet } from './stores/wallet'
import { connectLive } from './live/ws'

const session = useSession()
const wallet = useWallet()
const route = useRoute()
let disconnect: (() => void) | null = null

function start() {
  disconnect?.()
  disconnect = null
  if (!session.player) return
  wallet.refresh().catch(() => {})
  disconnect = connectLive(session.player.id, (ev) => wallet.applyLive(ev))
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
</template>
