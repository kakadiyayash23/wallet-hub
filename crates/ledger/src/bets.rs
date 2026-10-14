use crate::Store;
use chrono::{DateTime, Utc};
use domain::{Money, PlayerId, WalletError};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Bet {
    pub id: Uuid,
    pub player_id: Uuid,
    pub ticket_ref: Option<String>,
    pub event_name: String,
    pub market: String,
    pub selection: String,
    pub stake: i64,
    pub odds_milli: i32,
    pub currency: String,
    pub status: String,
    pub payout: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

pub struct NewBet<'a> {
    pub id: Uuid,
    pub player: PlayerId,
    pub event_name: &'a str,
    pub market: &'a str,
    pub selection: &'a str,
    pub stake: Money,
    pub odds_milli: i64,
    pub currency: &'a str,
}

impl Store {
    pub async fn create_bet(&self, b: NewBet<'_>) -> Result<Bet, WalletError> {
        sqlx::query_as::<_, Bet>(
            r#"insert into bets (id, player_id, event_name, market, selection, stake, odds_milli, currency, status)
               values ($1, $2, $3, $4, $5, $6, $7, $8, 'open') returning *"#,
        )
        .bind(b.id)
        .bind(b.player.0)
        .bind(b.event_name)
        .bind(b.market)
        .bind(b.selection)
        .bind(b.stake.minor())
        .bind(b.odds_milli as i32)
        .bind(b.currency)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn set_bet_ticket(
        &self,
        id: Uuid,
        ticket_ref: &str,
        odds_milli: i64,
    ) -> Result<(), WalletError> {
        sqlx::query("update bets set ticket_ref = $2, odds_milli = $3 where id = $1")
            .bind(id)
            .bind(ticket_ref)
            .bind(odds_milli as i32)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    pub async fn settle_bet(
        &self,
        ticket_ref: &str,
        status: &str,
        payout: Option<Money>,
    ) -> Result<Option<Bet>, WalletError> {
        sqlx::query_as::<_, Bet>(
            r#"update bets set status = $2, payout = $3, settled_at = now()
               where ticket_ref = $1 returning *"#,
        )
        .bind(ticket_ref)
        .bind(status)
        .bind(payout.map(Money::minor))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn set_bet_status(&self, id: Uuid, status: &str) -> Result<(), WalletError> {
        sqlx::query("update bets set status = $2 where id = $1")
            .bind(id)
            .bind(status)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    pub async fn bet_by_ticket(&self, ticket_ref: &str) -> Result<Option<Bet>, WalletError> {
        sqlx::query_as::<_, Bet>("select * from bets where ticket_ref = $1")
            .bind(ticket_ref)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn bet(&self, id: Uuid) -> Result<Option<Bet>, WalletError> {
        sqlx::query_as::<_, Bet>("select * from bets where id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn bets(&self, player: PlayerId, limit: i64) -> Result<Vec<Bet>, WalletError> {
        sqlx::query_as::<_, Bet>(
            "select * from bets where player_id = $1 order by created_at desc limit $2",
        )
        .bind(player.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
