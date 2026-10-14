use crate::Store;
use chrono::{DateTime, Utc};
use domain::{Money, PlayerId, WalletError};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub player_id: Uuid,
    pub provider: String,
    pub direction: String,
    pub provider_ref: Option<String>,
    pub status: String,
    pub amount: i64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewPayment<'a> {
    pub id: Uuid,
    pub player: PlayerId,
    pub provider: &'a str,
    /// deposit | withdrawal
    pub direction: &'a str,
    pub amount: Money,
    pub currency: &'a str,
    pub status: &'a str,
}

impl Store {
    pub async fn create_payment(&self, p: NewPayment<'_>) -> Result<Payment, WalletError> {
        sqlx::query_as::<_, Payment>(
            r#"insert into payments (id, player_id, provider, direction, status, amount, currency)
               values ($1, $2, $3, $4, $5, $6, $7) returning *"#,
        )
        .bind(p.id)
        .bind(p.player.0)
        .bind(p.provider)
        .bind(p.direction)
        .bind(p.status)
        .bind(p.amount.minor())
        .bind(p.currency)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn set_payment_ref(&self, id: Uuid, provider_ref: &str) -> Result<(), WalletError> {
        sqlx::query("update payments set provider_ref = $2, updated_at = now() where id = $1")
            .bind(id)
            .bind(provider_ref)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    pub async fn set_payment_status(&self, id: Uuid, status: &str) -> Result<(), WalletError> {
        sqlx::query("update payments set status = $2, updated_at = now() where id = $1")
            .bind(id)
            .bind(status)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    pub async fn payment_by_ref(
        &self,
        provider: &str,
        provider_ref: &str,
    ) -> Result<Option<Payment>, WalletError> {
        sqlx::query_as::<_, Payment>(
            "select * from payments where provider = $1 and provider_ref = $2",
        )
        .bind(provider)
        .bind(provider_ref)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn payment(&self, id: Uuid) -> Result<Option<Payment>, WalletError> {
        sqlx::query_as::<_, Payment>("select * from payments where id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn payments(
        &self,
        player: PlayerId,
        limit: i64,
    ) -> Result<Vec<Payment>, WalletError> {
        sqlx::query_as::<_, Payment>(
            "select * from payments where player_id = $1 order by created_at desc limit $2",
        )
        .bind(player.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
