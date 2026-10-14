//! Inbound callbacks and webhooks. One function per source, shared by the HTTP
//! route and the operator replay. Every call is stored raw before it runs.

use crate::live::LiveEvent;
use crate::state::AppState;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, Uri};
use domain::traits::{
    BetEvent, CallbackOutcome, CallbackReply, GameProvider, PaymentEvent, PaymentProvider,
    RawCallback, SportsbookProvider,
};
use domain::{AccountEnv, Money, MoveKind, Source, WalletCommand, WalletError};
use ledger::events::NewEvent;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub fn raw_from_http(method: &Method, uri: &Uri, headers: &HeaderMap, body: Bytes) -> RawCallback {
    let query: HashMap<String, String> = uri
        .query()
        .map(|q| {
            q.split('&')
                .map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                    (percent_decode(k), percent_decode(v))
                })
                .collect()
        })
        .unwrap_or_default();
    let headers = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|v| (k.as_str().to_ascii_lowercase(), v.to_string()))
        })
        .collect();
    RawCallback {
        method: method.to_string(),
        path: uri.path().to_string(),
        query,
        headers,
        body: body.to_vec(),
    }
}

/// Rebuild a RawCallback from a stored event, for replay.
pub fn raw_from_event(ev: &ledger::events::InboundEvent) -> RawCallback {
    let (path, query) = match ev.path.split_once('?') {
        Some((p, q)) => (p.to_string(), Some(q)),
        None => (ev.path.clone(), None),
    };
    let uri: Uri = match query {
        Some(q) => format!("{path}?{q}")
            .parse()
            .unwrap_or_else(|_| Uri::from_static("/")),
        None => path.parse().unwrap_or_else(|_| Uri::from_static("/")),
    };
    let mut headers = HeaderMap::new();
    if let Some(obj) = ev.headers.as_object() {
        for (k, v) in obj {
            if let (Ok(name), Some(val)) = (k.parse::<axum::http::HeaderName>(), v.as_str()) {
                if let Ok(val) = val.parse() {
                    headers.insert(name, val);
                }
            }
        }
    }
    let method: Method = ev.method.parse().unwrap_or(Method::POST);
    raw_from_http(&method, &uri, &headers, Bytes::from(ev.body.clone()))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn stored_path(raw: &RawCallback) -> String {
    if raw.query.is_empty() {
        raw.path.clone()
    } else {
        let q: Vec<String> = raw.query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{}?{}", raw.path, q.join("&"))
    }
}

async fn record(
    state: &AppState,
    source: &str,
    raw: &RawCallback,
    replay_of: Option<Uuid>,
) -> Result<Uuid, WalletError> {
    state
        .store
        .record_event(NewEvent {
            source,
            method: &raw.method,
            path: &stored_path(raw),
            headers: json!(raw.headers),
            body: &String::from_utf8_lossy(&raw.body),
            replay_of,
        })
        .await
}

// ---- BigBang -------------------------------------------------------------------

pub async fn handle_bigbang(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
) -> CallbackReply {
    let mut last_event: Option<Uuid> = None;
    let reply = bigbang_inner(state, raw, replay_of, &mut last_event).await;
    if let Some(id) = last_event {
        let _ = state
            .store
            .set_event_reply(id, reply.status, &reply.body)
            .await;
    }
    reply
}

async fn bigbang_inner(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
    last_event: &mut Option<Uuid>,
) -> CallbackReply {
    let provider = state.bigbang.clone();
    let event_id = match record(state, "bigbang", &raw, replay_of).await {
        Ok(id) => id,
        Err(e) => return provider.render_error(&e),
    };
    *last_event = Some(event_id);

    let cmd = match provider.parse_callback(&raw) {
        Ok(cmd) => cmd,
        Err(e) => {
            let sig_ok = !matches!(e, WalletError::InvalidSignature);
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(sig_ok),
                    "rejected",
                    Some(&e.to_string()),
                    None,
                )
                .await;
            return provider.render_error(&e);
        }
    };

    let (kind, amount, round) = match &cmd {
        WalletCommand::Move {
            kind,
            amount,
            round,
            ..
        } => (kind.as_str(), *amount, round.clone()),
        _ => ("read", Money::ZERO, None),
    };

    match state.store.apply(&cmd).await {
        Ok(outcome) => {
            let label = if outcome.applied {
                "applied"
            } else if outcome.duplicate {
                "duplicate"
            } else {
                "read"
            };
            let _ = state
                .store
                .finish_event(event_id, Some(true), label, None, outcome.journal_id)
                .await;
            if outcome.applied {
                state.publish(
                    LiveEvent::from_outcome(&outcome, "bigbang", kind, amount)
                        .with_round(round.as_ref()),
                );
            }
            provider.render_reply(&raw, &CallbackOutcome::Ok(outcome))
        }
        Err(WalletError::InsufficientFunds { balance }) => {
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(true),
                    "rejected",
                    Some("insufficient_funds"),
                    None,
                )
                .await;
            provider.render_reply(
                &raw,
                &CallbackOutcome::InsufficientFunds {
                    balance,
                    currency: state.cfg.platform_currency.clone(),
                },
            )
        }
        Err(WalletError::PlayerNotFound) => {
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(true),
                    "rejected",
                    Some("player_not_found"),
                    None,
                )
                .await;
            provider.render_reply(&raw, &CallbackOutcome::PlayerNotFound)
        }
        Err(WalletError::RolledBack(key)) => {
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(true),
                    "rejected",
                    Some(&format!("rolled back: {key}")),
                    None,
                )
                .await;
            let balance = read_balance(state, &cmd).await;
            provider.render_reply(
                &raw,
                &CallbackOutcome::RolledBack {
                    balance,
                    currency: state.cfg.platform_currency.clone(),
                },
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "bigbang callback failed");
            let _ = state
                .store
                .finish_event(event_id, Some(true), "error", Some(&e.to_string()), None)
                .await;
            provider.render_error(&e)
        }
    }
}

async fn read_balance(state: &AppState, cmd: &WalletCommand) -> Money {
    let (player_token, currency, env) = match cmd {
        WalletCommand::Move {
            player_token,
            currency,
            env,
            ..
        }
        | WalletCommand::Rollback {
            player_token,
            currency,
            env,
            ..
        }
        | WalletCommand::ReadBalance {
            player_token,
            currency,
            env,
        } => (player_token.clone(), currency.clone(), *env),
    };
    state
        .store
        .apply(&WalletCommand::ReadBalance {
            player_token,
            currency,
            env,
        })
        .await
        .map(|o| o.balance)
        .unwrap_or(Money::ZERO)
}

// ---- Stripe ---------------------------------------------------------------------

pub async fn handle_stripe(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
) -> CallbackReply {
    let mut last_event: Option<Uuid> = None;
    let reply = stripe_inner(state, raw, replay_of, &mut last_event).await;
    if let Some(id) = last_event {
        let _ = state
            .store
            .set_event_reply(id, reply.status, &reply.body)
            .await;
    }
    reply
}

async fn stripe_inner(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
    last_event: &mut Option<Uuid>,
) -> CallbackReply {
    let event_id = match record(state, "stripe", &raw, replay_of).await {
        Ok(id) => id,
        Err(_) => {
            return CallbackReply {
                status: 500,
                body: json!({ "received": false }),
            }
        }
    };
    *last_event = Some(event_id);
    let event = match state.stripe.parse_webhook(&raw) {
        Ok(ev) => ev,
        Err(e) => {
            let sig_ok = !matches!(e, WalletError::InvalidSignature);
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(sig_ok),
                    "rejected",
                    Some(&e.to_string()),
                    None,
                )
                .await;
            let status = if sig_ok { 400 } else { 401 };
            return CallbackReply {
                status,
                body: json!({ "received": false, "error": e.to_string() }),
            };
        }
    };

    match event {
        PaymentEvent::DepositCompleted {
            provider_ref,
            payment_id,
            amount,
            currency,
            ..
        } => {
            let payment = match state.store.payment_by_ref("stripe", &provider_ref).await {
                Ok(Some(p)) => Some(p),
                Ok(None) => match payment_id {
                    Some(id) => state.store.payment(id).await.ok().flatten(),
                    None => None,
                },
                Err(_) => None,
            };
            let Some(payment) = payment else {
                let _ = state
                    .store
                    .finish_event(
                        event_id,
                        Some(true),
                        "rejected",
                        Some("unknown checkout session"),
                        None,
                    )
                    .await;
                return CallbackReply {
                    status: 200,
                    body: json!({ "received": true, "note": "unknown session" }),
                };
            };
            if payment.amount != amount.minor() || !payment.currency.eq_ignore_ascii_case(&currency)
            {
                let _ = state
                    .store
                    .finish_event(
                        event_id,
                        Some(true),
                        "rejected",
                        Some("amount or currency mismatch"),
                        None,
                    )
                    .await;
                return CallbackReply {
                    status: 200,
                    body: json!({ "received": true, "note": "mismatch" }),
                };
            }
            let player = match state
                .store
                .player_by_id(domain::PlayerId(payment.player_id))
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    let _ = state
                        .store
                        .finish_event(event_id, Some(true), "error", Some(&e.to_string()), None)
                        .await;
                    return CallbackReply {
                        status: 500,
                        body: json!({ "received": false }),
                    };
                }
            };
            let cmd = WalletCommand::Move {
                player_token: player.external_token.clone(),
                currency: payment.currency.clone(),
                env: AccountEnv::Live,
                source: Source::Stripe,
                idempotency_key: format!("deposit:{provider_ref}"),
                kind: MoveKind::Deposit,
                amount,
                round: None,
                meta: json!({ "payment_id": payment.id, "checkout_session": provider_ref }),
            };
            match state.store.apply(&cmd).await {
                Ok(outcome) => {
                    let label = if outcome.duplicate {
                        "duplicate"
                    } else {
                        "applied"
                    };
                    let _ = state
                        .store
                        .finish_event(event_id, Some(true), label, None, outcome.journal_id)
                        .await;
                    let _ = state
                        .store
                        .set_payment_status(payment.id, "completed")
                        .await;
                    if outcome.applied {
                        state.publish(LiveEvent::from_outcome(
                            &outcome, "stripe", "deposit", amount,
                        ));
                    }
                    CallbackReply {
                        status: 200,
                        body: json!({ "received": true }),
                    }
                }
                Err(e) => {
                    let _ = state
                        .store
                        .finish_event(event_id, Some(true), "error", Some(&e.to_string()), None)
                        .await;
                    CallbackReply {
                        status: 500,
                        body: json!({ "received": false }),
                    }
                }
            }
        }
        PaymentEvent::DepositFailed {
            provider_ref,
            payment_id,
            ..
        } => {
            let payment = match state.store.payment_by_ref("stripe", &provider_ref).await {
                Ok(Some(p)) => Some(p),
                _ => match payment_id {
                    Some(id) => state.store.payment(id).await.ok().flatten(),
                    None => None,
                },
            };
            if let Some(p) = payment {
                let _ = state.store.set_payment_status(p.id, "failed").await;
            }
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(true),
                    "applied",
                    Some("payment marked failed"),
                    None,
                )
                .await;
            CallbackReply {
                status: 200,
                body: json!({ "received": true }),
            }
        }
        PaymentEvent::Ignored { event_type, .. } => {
            let _ = state
                .store
                .finish_event(event_id, Some(true), "ignored", Some(&event_type), None)
                .await;
            CallbackReply {
                status: 200,
                body: json!({ "received": true, "ignored": event_type }),
            }
        }
    }
}

// ---- Sportsbook -----------------------------------------------------------------

pub async fn handle_sports(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
) -> CallbackReply {
    let mut last_event: Option<Uuid> = None;
    let reply = sports_inner(state, raw, replay_of, &mut last_event).await;
    if let Some(id) = last_event {
        let _ = state
            .store
            .set_event_reply(id, reply.status, &reply.body)
            .await;
    }
    reply
}

async fn sports_inner(
    state: &AppState,
    raw: RawCallback,
    replay_of: Option<Uuid>,
    last_event: &mut Option<Uuid>,
) -> CallbackReply {
    let event_id = match record(state, "sports", &raw, replay_of).await {
        Ok(id) => id,
        Err(_) => {
            return CallbackReply {
                status: 500,
                body: json!({ "ok": false }),
            }
        }
    };
    *last_event = Some(event_id);
    let event = match state.sports.parse_callback(&raw) {
        Ok(ev) => ev,
        Err(e) => {
            let sig_ok = !matches!(e, WalletError::InvalidSignature);
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(sig_ok),
                    "rejected",
                    Some(&e.to_string()),
                    None,
                )
                .await;
            return CallbackReply {
                status: if sig_ok { 400 } else { 401 },
                body: json!({ "ok": false, "error": e.to_string() }),
            };
        }
    };

    let (ticket_ref, kind, amount, status, key) = match &event {
        BetEvent::Settled {
            ticket_ref,
            result,
            payout,
        } => match result.as_str() {
            "won" => (
                ticket_ref.clone(),
                MoveKind::Win,
                *payout,
                "won",
                format!("settle:{ticket_ref}"),
            ),
            "void" => (
                ticket_ref.clone(),
                MoveKind::Refund,
                *payout,
                "void",
                format!("settle:{ticket_ref}"),
            ),
            _ => (
                ticket_ref.clone(),
                MoveKind::Win,
                Money::ZERO,
                "lost",
                format!("settle:{ticket_ref}"),
            ),
        },
        BetEvent::CashedOut { ticket_ref, amount } => (
            ticket_ref.clone(),
            MoveKind::Cashout,
            *amount,
            "cashed_out",
            format!("cashout:{ticket_ref}"),
        ),
    };

    let bet = match state.store.bet_by_ticket(&ticket_ref).await {
        Ok(Some(b)) => b,
        _ => {
            let _ = state
                .store
                .finish_event(
                    event_id,
                    Some(true),
                    "rejected",
                    Some("unknown ticket"),
                    None,
                )
                .await;
            return CallbackReply {
                status: 200,
                body: json!({ "ok": true, "note": "unknown ticket" }),
            };
        }
    };
    let player = match state
        .store
        .player_by_id(domain::PlayerId(bet.player_id))
        .await
    {
        Ok(p) => p,
        Err(_) => {
            return CallbackReply {
                status: 500,
                body: json!({ "ok": false }),
            }
        }
    };
    let cmd = WalletCommand::Move {
        player_token: player.external_token.clone(),
        currency: bet.currency.clone(),
        env: AccountEnv::Live,
        source: Source::Sports,
        idempotency_key: key,
        kind,
        amount,
        round: Some(domain::RoundRef {
            round_id: ticket_ref.clone(),
            round_end: true,
        }),
        meta: json!({ "bet_id": bet.id, "status": status }),
    };
    match state.store.apply(&cmd).await {
        Ok(outcome) => {
            let label = if outcome.duplicate {
                "duplicate"
            } else {
                "applied"
            };
            let _ = state
                .store
                .finish_event(event_id, Some(true), label, None, outcome.journal_id)
                .await;
            if outcome.applied {
                let _ = state
                    .store
                    .settle_bet(&ticket_ref, status, Some(amount))
                    .await;
                state.publish(LiveEvent::from_outcome(
                    &outcome,
                    "sports",
                    kind.as_str(),
                    amount,
                ));
            }
            CallbackReply {
                status: 200,
                body: json!({ "ok": true, "duplicate": outcome.duplicate }),
            }
        }
        Err(e) => {
            let _ = state
                .store
                .finish_event(event_id, Some(true), "error", Some(&e.to_string()), None)
                .await;
            CallbackReply {
                status: 500,
                body: json!({ "ok": false }),
            }
        }
    }
}
