import { createRouter, createWebHistory } from 'vue-router'
import { useSession } from './stores/session'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/lobby' },
    { path: '/login', component: () => import('./pages/LoginPage.vue') },
    { path: '/lobby', component: () => import('./pages/LobbyPage.vue') },
    { path: '/play/:gameId', component: () => import('./pages/PlayPage.vue') },
    { path: '/wallet', component: () => import('./pages/WalletPage.vue') },
    { path: '/sports', component: () => import('./pages/SportsPage.vue') },
    { path: '/ops', component: () => import('./pages/OpsPage.vue') },
  ],
})

router.beforeEach(async (to) => {
  const session = useSession()
  await session.boot()
  if (to.path !== '/login' && to.path !== '/ops' && !session.player) return '/login'
  return true
})
