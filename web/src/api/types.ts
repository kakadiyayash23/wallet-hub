export interface Player { id: string; external_token: string; display_name: string; locale: string; currency: string }
export interface Account { id: string; currency: string; env: 'live' | 'sandbox'; balance: number }
export interface Journal {
  id: string; source: string; idempotency_key: string; kind: string; amount: number
  round_id: string | null; round_end: boolean | null; reply_balance: number; rolled_back: boolean
  env: string; currency: string; meta: Record<string, unknown>; created_at: string
}
export interface Payment { id: string; provider: string; direction: string; provider_ref: string | null; status: string; amount: number; currency: string; created_at: string }
export interface Game { id: number; provider: string; provider_game_id: string; name: string; title: string; category: string; thumbnail: string | null; game_type: string; is_premium: boolean }
export interface CategoryCount { category: string; count: number }
export interface SportInfo { key: string; label: string; count: number }
export interface Selection { key: string; label: string; odds_milli: number }
export interface Market { event_id: string; sport: string; league: string; home: string; away: string; starts_at: string; selections: Selection[] }
export interface Bet { id: string; ticket_ref: string | null; event_name: string; market: string; selection: string; stake: number; odds_milli: number; currency: string; status: string; payout: number | null; created_at: string; settled_at: string | null }
export interface InboundEvent { id: string; source: string; received_at: string; method: string; path: string; headers: Record<string, string>; body: string; signature_ok: boolean | null; outcome: string; detail: string | null; journal_id: string | null; replay_of: string | null; duration_ms: number | null; reply_status: number | null; reply_body: unknown }
export interface EventStats { received: number; duplicates: number; rejected: number; errors: number }
export interface LiveEvent { player_id: string; env: 'live' | 'sandbox'; currency: string; balance: number; journal_id: string | null; source: string; kind: string; amount: number; round_id: string | null; round_end: boolean | null }
export interface ChaosReport { rounds: number; sent: number; duplicates_sent: number; duplicates_rejected: number; bad_signatures_sent: number; bad_signatures_rejected: number; http_errors: number; start_balance: number; expected_balance: number; final_balance: number; drift: number; trial_balance: number; elapsed_ms: number }
export interface Reconciliation { player_id: string; env: string; provider_net: number; ledger_net: number; drift: number; provider_rows: number; rows: { external_id: string; game: string; bet: number; win: number; at: string }[] }
export interface ApiErrorBody { error: { code: string; message: string; balance?: number } }
export interface Whoami { player: Player | null; ops: boolean }
export interface SourceHealth { source: string; received_24h: number; applied_24h: number; duplicates_24h: number; rejected_24h: number; errors_24h: number; last_success_at: string | null; last_error_at: string | null; last_error_detail: string | null; avg_duration_ms: number | null; p95_duration_ms: number | null }
export interface ProviderRow { category: string; count: number; games: Game[] }
export interface Lobby { continue_playing: Game[]; popular: Game[]; providers: ProviderRow[] }
export interface TimelineBucket { hour: string; applied: number; duplicates: number; rejected: number; errors: number }
