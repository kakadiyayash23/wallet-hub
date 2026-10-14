import type { ApiErrorBody } from './types'

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message)
  }
}

let playerId: string | null = null
let opsKey: string | null = null

export function setPlayerId(id: string | null) { playerId = id }
export function setOpsKey(key: string | null) { opsKey = key }

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' }
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  if (playerId) headers['X-Player-Id'] = playerId
  if (opsKey) headers['X-Ops-Key'] = opsKey
  const res = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  const text = await res.text()
  const data = text ? JSON.parse(text) : null
  if (!res.ok) {
    const err = data as ApiErrorBody | null
    throw new ApiError(res.status, err?.error?.code ?? 'http_error', err?.error?.message ?? res.statusText)
  }
  return data as T
}

export const api = {
  get: <T>(path: string) => request<T>('GET', path),
  post: <T>(path: string, body?: unknown) => request<T>('POST', path, body ?? {}),
  patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
}
