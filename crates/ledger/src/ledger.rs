//! The money rules. Everything here runs inside one database transaction per
//! command, with the player's account row locked.
//!
//! Rules:
//! 1. Double entry: a journal owns two entries that sum to zero.
//! 2. Idempotency: `(source, idempotency_key)` is unique. A duplicate returns
//!    the stored reply balance and moves nothing.
//! 3. Insufficient funds is an outcome, not a crash: the caller gets the
//!    current balance to return to the provider.
//! 4. Rollback leaves a tombstone. A rollback for a key that never arrived still
//!    writes a journal; if the original arrives later it is refused.

use crate::Store;
use chrono::{DateTime, Utc};
use domain::{
    AccountEnv, Money, MoveKind, PlayerId, Source, WalletCommand, WalletError, WalletOutcome,
};
use serde::Serialize;
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct AccountRow {
    id: Uuid,
    player_id: Uuid,
    balance: i64,
}

#[derive(Debug, FromRow)]
struct JournalLookup {
    id: Uuid,
    player_id: Uuid,
    kind: String,
    amount: i64,
    reply_balance: i64,
    rolled_back: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct JournalView {
    pub id: Uuid,
    pub source: String,
    pub idempotency_key: String,
    pub kind: String,
    pub amount: i64,
    pub round_id: Option<String>,
    pub round_end: Option<bool>,
    pub reply_balance: i64,
    pub rolled_back: bool,
    pub env: String,
    pub currency: String,
    pub meta: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AccountView {
    pub id: Uuid,
    pub currency: String,
    pub env: String,
    pub balance: i64,
}

impl Store {
    pub async fn apply(&self, cmd: &WalletCommand) -> Result<WalletOutcome, WalletError> {
        let mut tx = self.pool.begin().await.map_err(anyhow::Error::from)?;
        let out = match cmd {
            WalletCommand::ReadBalance {
                player_token,
                currency,
                env,
            } => {
                let acc = lock_account(&mut tx, player_token, currency, *env).await?;
                WalletOutcome {
                    balance: Money::from_minor(acc.balance),
                    currency: currency.clone(),
                    env: *env,
                    applied: false,
                    duplicate: false,
                    journal_id: None,
                    player_id: PlayerId(acc.player_id),
                }
            }
            WalletCommand::Move {
                player_token,
                currency,
                env,
                source,
                idempotency_key,
                kind,
                amount,
                round,
                meta,
            } => {
                let acc = lock_account(&mut tx, player_token, currency, *env).await?;
                if let Some(existing) = find_journal(&mut tx, *source, idempotency_key).await? {
                    // A provider transaction id is global. The same id for a
                    // different player is a replay or a provider bug: refuse it
                    // instead of leaking the other player's balance.
                    if existing.player_id != acc.player_id {
                        tx.rollback().await.map_err(anyhow::Error::from)?;
                        return Err(WalletError::bad_request(format!(
                            "transaction {idempotency_key} belongs to another player"
                        )));
                    }
                    if existing.kind == "tombstone" {
                        tx.rollback().await.map_err(anyhow::Error::from)?;
                        return Err(WalletError::RolledBack(idempotency_key.clone()));
                    }
                    tx.commit().await.map_err(anyhow::Error::from)?;
                    return Ok(WalletOutcome {
                        balance: Money::from_minor(existing.reply_balance),
                        currency: currency.clone(),
                        env: *env,
                        applied: false,
                        duplicate: true,
                        journal_id: Some(existing.id),
                        player_id: PlayerId(acc.player_id),
                    });
                }
                let new_balance = acc
                    .balance
                    .checked_add(amount.minor())
                    .ok_or_else(|| anyhow::anyhow!("balance overflow"))?;
                if new_balance < 0 {
                    tx.rollback().await.map_err(anyhow::Error::from)?;
                    return Err(WalletError::InsufficientFunds {
                        balance: Money::from_minor(acc.balance),
                    });
                }
                let journal_id = insert_journal(
                    &mut tx,
                    &acc,
                    *source,
                    idempotency_key,
                    kind.as_str(),
                    *amount,
                    round.as_ref().map(|r| r.round_id.as_str()),
                    round.as_ref().map(|r| r.round_end),
                    new_balance,
                    meta,
                )
                .await?;
                if let Some(r) = round {
                    upsert_round(
                        &mut tx,
                        *source,
                        &r.round_id,
                        r.round_end,
                        acc.player_id,
                        *amount,
                    )
                    .await?;
                }
                WalletOutcome {
                    balance: Money::from_minor(new_balance),
                    currency: currency.clone(),
                    env: *env,
                    applied: true,
                    duplicate: false,
                    journal_id: Some(journal_id),
                    player_id: PlayerId(acc.player_id),
                }
            }
            WalletCommand::Rollback {
                player_token,
                currency,
                env,
                source,
                original_key,
            } => {
                let acc = lock_account(&mut tx, player_token, currency, *env).await?;
                let rb_key = format!("rb:{original_key}");
                if let Some(existing) = find_journal(&mut tx, *source, &rb_key).await? {
                    tx.commit().await.map_err(anyhow::Error::from)?;
                    return Ok(WalletOutcome {
                        balance: Money::from_minor(existing.reply_balance),
                        currency: currency.clone(),
                        env: *env,
                        applied: false,
                        duplicate: true,
                        journal_id: Some(existing.id),
                        player_id: PlayerId(acc.player_id),
                    });
                }
                match find_journal(&mut tx, *source, original_key).await? {
                    Some(orig) if !orig.rolled_back && orig.kind != "tombstone" => {
                        let reversed = Money::from_minor(orig.amount)
                            .checked_neg()
                            .map_err(anyhow::Error::from)?;
                        let new_balance = acc.balance + reversed.minor();
                        let journal_id = insert_journal(
                            &mut tx,
                            &acc,
                            *source,
                            &rb_key,
                            "rollback",
                            reversed,
                            None,
                            None,
                            new_balance,
                            &serde_json::json!({ "original_key": original_key }),
                        )
                        .await?;
                        sqlx::query("update journals set rolled_back = true where id = $1")
                            .bind(orig.id)
                            .execute(&mut *tx)
                            .await
                            .map_err(anyhow::Error::from)?;
                        WalletOutcome {
                            balance: Money::from_minor(new_balance),
                            currency: currency.clone(),
                            env: *env,
                            applied: true,
                            duplicate: false,
                            journal_id: Some(journal_id),
                            player_id: PlayerId(acc.player_id),
                        }
                    }
                    Some(_) => WalletOutcome {
                        balance: Money::from_minor(acc.balance),
                        currency: currency.clone(),
                        env: *env,
                        applied: false,
                        duplicate: true,
                        journal_id: None,
                        player_id: PlayerId(acc.player_id),
                    },
                    None => {
                        // Tombstone: the original never arrived. Refuse it if it shows up later.
                        let journal_id = insert_journal(
                            &mut tx,
                            &acc,
                            *source,
                            original_key,
                            "tombstone",
                            Money::ZERO,
                            None,
                            None,
                            acc.balance,
                            &serde_json::json!({ "reason": "rollback before original" }),
                        )
                        .await?;
                        WalletOutcome {
                            balance: Money::from_minor(acc.balance),
                            currency: currency.clone(),
                            env: *env,
                            applied: true,
                            duplicate: false,
                            journal_id: Some(journal_id),
                            player_id: PlayerId(acc.player_id),
                        }
                    }
                }
            }
        };
        tx.commit().await.map_err(anyhow::Error::from)?;
        Ok(out)
    }

    pub async fn accounts(&self, player: PlayerId) -> Result<Vec<AccountView>, WalletError> {
        sqlx::query_as::<_, AccountView>(
            "select id, currency, env, balance from accounts where player_id = $1 order by env",
        )
        .bind(player.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn journals(
        &self,
        player: PlayerId,
        limit: i64,
    ) -> Result<Vec<JournalView>, WalletError> {
        sqlx::query_as::<_, JournalView>(
            r#"select j.id, j.source, j.idempotency_key, j.kind, j.amount, j.round_id, j.round_end,
                      j.reply_balance, j.rolled_back, a.env, a.currency, j.meta, j.created_at
               from journals j join accounts a on a.id = j.account_id
               where j.player_id = $1 order by j.created_at desc limit $2"#,
        )
        .bind(player.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    /// Net amount a source moved for a player in one environment. Used by reconciliation.
    pub async fn net_by_source(
        &self,
        player: PlayerId,
        source: Source,
        env: AccountEnv,
    ) -> Result<Money, WalletError> {
        let sum: Option<i64> = sqlx::query_scalar(
            r#"select sum(j.amount)::bigint from journals j join accounts a on a.id = j.account_id
               where j.player_id = $1 and j.source = $2 and a.env = $3"#,
        )
        .bind(player.0)
        .bind(source.as_str())
        .bind(env.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;
        Ok(Money::from_minor(sum.unwrap_or(0)))
    }

    /// Sum of every ledger entry. Must always be zero.
    pub async fn trial_balance(&self) -> Result<i64, WalletError> {
        let sum: Option<i64> = sqlx::query_scalar("select sum(amount)::bigint from ledger_entries")
            .fetch_one(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(sum.unwrap_or(0))
    }
}

async fn lock_account(
    tx: &mut Transaction<'_, Postgres>,
    player_token: &str,
    currency: &str,
    env: AccountEnv,
) -> Result<AccountRow, WalletError> {
    let player_id: Option<Uuid> =
        sqlx::query_scalar("select id from players where external_token = $1")
            .bind(player_token)
            .fetch_optional(&mut **tx)
            .await
            .map_err(anyhow::Error::from)?;
    let player_id = player_id.ok_or(WalletError::PlayerNotFound)?;
    sqlx::query(
        "insert into accounts (id, player_id, currency, env) values ($1, $2, $3, $4) on conflict do nothing",
    )
    .bind(Uuid::new_v4())
    .bind(player_id)
    .bind(currency)
    .bind(env.as_str())
    .execute(&mut **tx)
    .await
    .map_err(anyhow::Error::from)?;
    sqlx::query_as::<_, AccountRow>(
        "select id, player_id, balance from accounts where player_id = $1 and currency = $2 and env = $3 for update",
    )
    .bind(player_id)
    .bind(currency)
    .bind(env.as_str())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| anyhow::Error::from(e).into())
}

async fn find_journal(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    key: &str,
) -> Result<Option<JournalLookup>, WalletError> {
    sqlx::query_as::<_, JournalLookup>(
        "select id, player_id, kind, amount, reply_balance, rolled_back from journals where source = $1 and idempotency_key = $2",
    )
    .bind(source.as_str())
    .bind(key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| anyhow::Error::from(e).into())
}

#[allow(clippy::too_many_arguments)]
async fn insert_journal(
    tx: &mut Transaction<'_, Postgres>,
    acc: &AccountRow,
    source: Source,
    key: &str,
    kind: &str,
    amount: Money,
    round_id: Option<&str>,
    round_end: Option<bool>,
    new_balance: i64,
    meta: &serde_json::Value,
) -> Result<Uuid, WalletError> {
    let journal_id = Uuid::new_v4();
    sqlx::query(
        r#"insert into journals (id, source, idempotency_key, kind, player_id, account_id, amount,
                                 round_id, round_end, reply_balance, meta)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
    )
    .bind(journal_id)
    .bind(source.as_str())
    .bind(key)
    .bind(kind)
    .bind(acc.player_id)
    .bind(acc.id)
    .bind(amount.minor())
    .bind(round_id)
    .bind(round_end)
    .bind(new_balance)
    .bind(meta)
    .execute(&mut **tx)
    .await
    .map_err(|e| match e.as_database_error().and_then(|d| d.code()) {
        // Two players racing on the same provider id: the unique index wins.
        Some(code) if code == "23505" => {
            WalletError::bad_request(format!("transaction {key} already used"))
        }
        _ => WalletError::Internal(e.into()),
    })?;
    // Double entry: player side and provider counter side.
    sqlx::query(
        r#"insert into ledger_entries (journal_id, account_id, counter, amount)
           values ($1, $2, 'player', $3), ($1, null, $4, $5)"#,
    )
    .bind(journal_id)
    .bind(acc.id)
    .bind(amount.minor())
    .bind(format!("provider:{source}"))
    .bind(-amount.minor())
    .execute(&mut **tx)
    .await
    .map_err(anyhow::Error::from)?;
    sqlx::query("update accounts set balance = $2 where id = $1")
        .bind(acc.id)
        .bind(new_balance)
        .execute(&mut **tx)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(journal_id)
}

async fn upsert_round(
    tx: &mut Transaction<'_, Postgres>,
    source: Source,
    round_id: &str,
    round_end: bool,
    player_id: Uuid,
    amount: Money,
) -> Result<(), WalletError> {
    sqlx::query(
        r#"insert into rounds (source, round_id, player_id, net, closed_at)
           values ($1, $2, $3, $4, case when $5 then now() else null end)
           on conflict (source, round_id) do update
             set net = rounds.net + excluded.net,
                 closed_at = coalesce(rounds.closed_at, excluded.closed_at)"#,
    )
    .bind(source.as_str())
    .bind(round_id)
    .bind(player_id)
    .bind(amount.minor())
    .bind(round_end)
    .execute(&mut **tx)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(())
}

/// Kinds that the api layer needs to name in SQL. Kept here so the strings exist once.
pub fn kind_str(kind: MoveKind) -> &'static str {
    kind.as_str()
}
