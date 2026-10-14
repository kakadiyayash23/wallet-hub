//! Integration tests against a real Postgres. They need DATABASE_URL; without
//! it they print a notice and pass, so `cargo test` works on a laptop with no DB.

use domain::{AccountEnv, Money, MoveKind, RoundRef, Source, WalletCommand, WalletError};
use ledger::Store;
use uuid::Uuid;

async fn store() -> Option<Store> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping ledger integration tests");
        return None;
    };
    let store = Store::connect(&url).await.expect("connect");
    store.migrate().await.expect("migrate");
    Some(store)
}

/// Idempotency keys are unique per source across all players (a provider's
/// transaction id is global), so every test prefixes its keys with the
/// player token.
fn mv(token: &str, key: &str, kind: MoveKind, minor: i64) -> WalletCommand {
    WalletCommand::Move {
        player_token: token.to_string(),
        currency: "EUR".into(),
        env: AccountEnv::Sandbox,
        source: Source::Bigbang,
        idempotency_key: format!("{token}:{key}"),
        kind,
        amount: Money::from_minor(minor),
        round: Some(RoundRef {
            round_id: key.to_string(),
            round_end: true,
        }),
        meta: serde_json::json!({}),
    }
}

#[tokio::test]
async fn idempotent_moves_and_insufficient_funds() {
    let Some(store) = store().await else { return };
    let name = format!("t-{}", Uuid::new_v4().simple());
    let p = store.ensure_player(&name, "en", "EUR").await.unwrap();
    let tok = p.external_token.as_str();

    let a = store
        .apply(&mv(tok, "dep1", MoveKind::Deposit, 1000))
        .await
        .unwrap();
    assert!(a.applied && !a.duplicate);
    assert_eq!(a.balance.minor(), 1000);

    // Same key again: nothing moves, same balance comes back.
    let b = store
        .apply(&mv(tok, "dep1", MoveKind::Deposit, 1000))
        .await
        .unwrap();
    assert!(!b.applied && b.duplicate);
    assert_eq!(b.balance.minor(), 1000);
    assert_eq!(a.journal_id, b.journal_id);

    // Too big a bet is an outcome with the current balance, not a crash.
    match store.apply(&mv(tok, "bet-big", MoveKind::Bet, -2000)).await {
        Err(WalletError::InsufficientFunds { balance }) => assert_eq!(balance.minor(), 1000),
        other => panic!("expected insufficient funds, got {other:?}"),
    }

    // Zero amount closes a round and is accepted.
    let z = store
        .apply(&mv(tok, "zero", MoveKind::Win, 0))
        .await
        .unwrap();
    assert!(z.applied);
    assert_eq!(z.balance.minor(), 1000);

    assert_eq!(store.trial_balance().await.unwrap(), 0);
}

#[tokio::test]
async fn rollback_reverses_and_tombstones() {
    let Some(store) = store().await else { return };
    let name = format!("t-{}", Uuid::new_v4().simple());
    let p = store.ensure_player(&name, "en", "EUR").await.unwrap();
    let tok = p.external_token.as_str();
    store
        .apply(&mv(tok, "dep", MoveKind::Deposit, 1000))
        .await
        .unwrap();
    store
        .apply(&mv(tok, "bet1", MoveKind::Bet, -250))
        .await
        .unwrap();

    let rb = |key: &str| WalletCommand::Rollback {
        player_token: tok.to_string(),
        currency: "EUR".into(),
        env: AccountEnv::Sandbox,
        source: Source::Bigbang,
        original_key: format!("{tok}:{key}"),
    };

    // Rollback of a known move restores the balance exactly once.
    let r1 = store.apply(&rb("bet1")).await.unwrap();
    assert!(r1.applied);
    assert_eq!(r1.balance.minor(), 1000);
    let r2 = store.apply(&rb("bet1")).await.unwrap();
    assert!(r2.duplicate);
    assert_eq!(r2.balance.minor(), 1000);

    // Rollback before the original arrived: tombstone, then the original is refused.
    let t = store.apply(&rb("ghost")).await.unwrap();
    assert!(t.applied);
    assert_eq!(t.balance.minor(), 1000);
    match store.apply(&mv(tok, "ghost", MoveKind::Bet, -100)).await {
        Err(WalletError::RolledBack(k)) => assert_eq!(k, format!("{tok}:ghost")),
        other => panic!("expected RolledBack, got {other:?}"),
    }
    let bal = store
        .apply(&WalletCommand::ReadBalance {
            player_token: tok.into(),
            currency: "EUR".into(),
            env: AccountEnv::Sandbox,
        })
        .await
        .unwrap();
    assert_eq!(bal.balance.minor(), 1000);
    assert_eq!(store.trial_balance().await.unwrap(), 0);
}

#[tokio::test]
async fn concurrent_duplicates_apply_once() {
    let Some(store) = store().await else { return };
    let name = format!("t-{}", Uuid::new_v4().simple());
    let p = store.ensure_player(&name, "en", "EUR").await.unwrap();
    let tok = p.external_token.clone();
    store
        .apply(&mv(&tok, "dep", MoveKind::Deposit, 10_000))
        .await
        .unwrap();

    // 40 workers race on 10 distinct keys, four copies each.
    let mut handles = Vec::new();
    for i in 0..40 {
        let store = store.clone();
        let tok = tok.clone();
        handles.push(tokio::spawn(async move {
            store
                .apply(&mv(&tok, &format!("race-{}", i % 10), MoveKind::Bet, -100))
                .await
                .unwrap()
        }));
    }
    let mut applied = 0;
    for h in handles {
        if h.await.unwrap().applied {
            applied += 1;
        }
    }
    assert_eq!(applied, 10);
    let bal = store
        .apply(&WalletCommand::ReadBalance {
            player_token: tok,
            currency: "EUR".into(),
            env: AccountEnv::Sandbox,
        })
        .await
        .unwrap();
    assert_eq!(bal.balance.minor(), 10_000 - 1_000);
    assert_eq!(store.trial_balance().await.unwrap(), 0);
}

#[tokio::test]
async fn same_key_for_another_player_is_refused() {
    let Some(store) = store().await else { return };
    let a = store
        .ensure_player(&format!("t-{}", Uuid::new_v4().simple()), "en", "EUR")
        .await
        .unwrap();
    let b = store
        .ensure_player(&format!("t-{}", Uuid::new_v4().simple()), "en", "EUR")
        .await
        .unwrap();
    store
        .apply(&mv(&a.external_token, "shared", MoveKind::Deposit, 500))
        .await
        .unwrap();
    // Player B presents A's transaction id.
    let mut cmd = mv(&b.external_token, "x", MoveKind::Deposit, 500);
    if let WalletCommand::Move {
        idempotency_key, ..
    } = &mut cmd
    {
        *idempotency_key = format!("{}:shared", a.external_token);
    }
    match store.apply(&cmd).await {
        Err(WalletError::BadRequest(m)) => assert!(m.contains("another player"), "{m}"),
        other => panic!("expected BadRequest, got {other:?}"),
    }
    let bal = store
        .apply(&WalletCommand::ReadBalance {
            player_token: b.external_token.clone(),
            currency: "EUR".into(),
            env: AccountEnv::Sandbox,
        })
        .await
        .unwrap();
    assert_eq!(bal.balance.minor(), 0);
}
