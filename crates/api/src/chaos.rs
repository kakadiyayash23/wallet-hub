//! Chaos simulator: fires BigBang-shaped callbacks at our own wallet endpoint
//! with duplicates, delays, reordering and bad signatures switched on, then
//! checks that the ledger ended exactly where it should.

use crate::state::AppState;
use domain::{AccountEnv, Money, MoveKind, PlayerId, Source, WalletCommand, WalletError};
use provider_bigbang::sign_balance_change;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ChaosRequest {
    pub player_id: Uuid,
    #[serde(default = "d_rounds")]
    pub rounds: usize,
    /// Send every Nth callback twice. 0 = never.
    #[serde(default = "d_dup")]
    pub duplicate_every: usize,
    /// Corrupt the signature of every Nth callback. 0 = never.
    #[serde(default)]
    pub bad_signature_every: usize,
    #[serde(default = "d_delay")]
    pub max_delay_ms: u64,
    #[serde(default = "d_true")]
    pub shuffle: bool,
}

fn d_rounds() -> usize {
    200
}
fn d_dup() -> usize {
    5
}
fn d_delay() -> u64 {
    20
}
fn d_true() -> bool {
    true
}

#[derive(Serialize)]
pub struct ChaosReport {
    pub rounds: usize,
    pub sent: usize,
    pub duplicates_sent: usize,
    pub duplicates_rejected: usize,
    pub bad_signatures_sent: usize,
    pub bad_signatures_rejected: usize,
    pub http_errors: usize,
    pub start_balance: Money,
    pub expected_balance: Money,
    pub final_balance: Money,
    pub drift: Money,
    pub trial_balance: i64,
    pub elapsed_ms: u128,
}

struct Shot {
    body: serde_json::Value,
    amount: Money,
    duplicate: bool,
    bad_signature: bool,
}

pub async fn run(state: &AppState, req: ChaosRequest) -> Result<ChaosReport, WalletError> {
    let rounds = req.rounds.clamp(1, 2000);
    let player = state.store.player_by_id(PlayerId(req.player_id)).await?;
    let currency = state.cfg.platform_currency.clone();
    let api_key = state.cfg.bigbang_api_key.clone();
    let run_id = Uuid::new_v4().simple().to_string();

    // Seed the sandbox account so the biggest possible loss never goes negative.
    let seed = Money::from_minor(rounds as i64 * 500);
    let seeded = state
        .store
        .apply(&WalletCommand::Move {
            player_token: player.external_token.clone(),
            currency: currency.clone(),
            env: AccountEnv::Sandbox,
            source: Source::Chaos,
            idempotency_key: format!("seed:{run_id}"),
            kind: MoveKind::Deposit,
            amount: seed,
            round: None,
            meta: json!({ "chaos_run": run_id }),
        })
        .await?;
    let start_balance = seeded.balance;

    // Build the shots. The RNG stays inside this block so the future is Send.
    let mut shots: Vec<Shot> = Vec::with_capacity(rounds * 2);
    let mut expected = start_balance;
    let amounts: Vec<i64> = {
        let mut rng = rand::thread_rng();
        (0..rounds).map(|_| rng.gen_range(-500..=800)).collect()
    };
    for (i, cents) in amounts.into_iter().enumerate() {
        let amount = Money::from_minor(cents);
        let txn = format!("chaos-{run_id}-{i}");
        let bad = req.bad_signature_every > 0 && (i + 1) % req.bad_signature_every == 0;
        let sig = sign_balance_change(
            &api_key,
            &player.external_token,
            amount.to_f64(),
            "ChaosSlot",
            "Chaos",
            &txn,
        );
        let sig = if bad {
            format!("{}00", &sig[..sig.len() - 2])
        } else {
            sig
        };
        let body = json!({
            "username": player.external_token,
            "amount": amount.to_f64(),
            "game": "ChaosSlot",
            "game_category": "Chaos",
            "transaction_id": txn,
            "signature": sig,
            "round_id": txn,
            "type": "round",
            "round_end": true,
            "sandbox": true,
        });
        if !bad {
            expected = expected.checked_add(amount).map_err(anyhow::Error::from)?;
        }
        shots.push(Shot {
            body: body.clone(),
            amount,
            duplicate: false,
            bad_signature: bad,
        });
        if req.duplicate_every > 0 && (i + 1) % req.duplicate_every == 0 {
            shots.push(Shot {
                body,
                amount,
                duplicate: true,
                bad_signature: bad,
            });
        }
    }
    if req.shuffle {
        shots.shuffle(&mut rand::thread_rng());
    }

    let url = format!(
        "http://127.0.0.1:{}/callbacks/bigbang/balance_change",
        state.cfg.local_port()
    );
    let started = std::time::Instant::now();
    let mut report = ChaosReport {
        rounds,
        sent: 0,
        duplicates_sent: 0,
        duplicates_rejected: 0,
        bad_signatures_sent: 0,
        bad_signatures_rejected: 0,
        http_errors: 0,
        start_balance,
        expected_balance: expected,
        final_balance: Money::ZERO,
        drift: Money::ZERO,
        trial_balance: 0,
        elapsed_ms: 0,
    };

    // Four workers in parallel, like a provider with a few retry threads.
    let shots = std::sync::Arc::new(tokio::sync::Mutex::new(shots));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let shots = shots.clone();
        let http = state.http.clone();
        let url = url.clone();
        let max_delay = req.max_delay_ms.min(500);
        handles.push(tokio::spawn(async move {
            let mut local = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
            loop {
                let shot = { shots.lock().await.pop() };
                let Some(shot) = shot else { break };
                if max_delay > 0 {
                    let d = rand::thread_rng().gen_range(0..=max_delay);
                    tokio::time::sleep(Duration::from_millis(d)).await;
                }
                local.0 += 1;
                // A duplicate of a bad-signature shot is rejected for the signature, not as a duplicate.
                if shot.duplicate && !shot.bad_signature {
                    local.1 += 1;
                }
                if shot.bad_signature {
                    local.3 += 1;
                }
                let res = http.post(&url).json(&shot.body).send().await;
                match res {
                    Ok(r) => {
                        let status = r.status().as_u16();
                        if shot.bad_signature && status == 401 {
                            local.4 += 1;
                        } else if status >= 500 {
                            local.5 += 1;
                        }
                        // A duplicate that was correctly ignored still answers 200 with the stored balance.
                        if shot.duplicate && !shot.bad_signature && status == 200 {
                            local.2 += 1;
                        }
                    }
                    Err(_) => local.5 += 1,
                }
                let _ = shot.amount;
            }
            local
        }));
    }
    for h in handles {
        if let Ok(l) = h.await {
            report.sent += l.0;
            report.duplicates_sent += l.1;
            report.duplicates_rejected += l.2;
            report.bad_signatures_sent += l.3;
            report.bad_signatures_rejected += l.4;
            report.http_errors += l.5;
        }
    }

    let final_balance = state
        .store
        .apply(&WalletCommand::ReadBalance {
            player_token: player.external_token.clone(),
            currency,
            env: AccountEnv::Sandbox,
        })
        .await?
        .balance;
    report.final_balance = final_balance;
    report.drift = final_balance
        .checked_sub(expected)
        .map_err(anyhow::Error::from)?;
    report.trial_balance = state.store.trial_balance().await?;
    report.elapsed_ms = started.elapsed().as_millis();
    Ok(report)
}
