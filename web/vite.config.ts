import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// /api and /ws are proxied to the Rust api in dev, so the browser sees one origin.
// VITE_API_TARGET lets a second dev server point at another api instance.
const target = process.env.VITE_API_TARGET ?? 'http://localhost:8080'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      '/api': { target, changeOrigin: true },
      '/ws': { target: target.replace(/^http/, 'ws'), ws: true },
    },
  },
})
