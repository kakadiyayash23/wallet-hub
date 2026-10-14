import type { LiveEvent } from '../api/types'

/** One socket per session. Reconnects with backoff. */
export function connectLive(playerId: string, onEvent: (ev: LiveEvent) => void): () => void {
  let socket: WebSocket | null = null
  let closed = false
  let delay = 500

  const open = () => {
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    socket = new WebSocket(`${proto}://${location.host}/ws?player_id=${encodeURIComponent(playerId)}`)
    socket.onmessage = (m) => {
      try { onEvent(JSON.parse(m.data) as LiveEvent) } catch { /* ignore bad frame */ }
    }
    socket.onopen = () => { delay = 500 }
    socket.onclose = () => {
      if (closed) return
      setTimeout(open, delay)
      delay = Math.min(delay * 2, 10_000)
    }
  }
  open()
  return () => { closed = true; socket?.close() }
}
