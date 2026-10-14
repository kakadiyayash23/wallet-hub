//! Inbound callbacks and webhooks, stored raw before anything else runs.

use crate::Store;
use chrono::{DateTime, Utc};
use domain::WalletError;
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InboundEvent {
    pub id: Uuid,
    pub source: String,
    pub received_at: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub headers: serde_json::Value,
    pub body: String,
    pub signature_ok: Option<bool>,
    pub outcome: String,
    pub detail: Option<String>,
    pub journal_id: Option<Uuid>,
    pub replay_of: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EventStats {
    pub received: i64,
    pub duplicates: i64,
    pub rejected: i64,
    pub errors: i64,
}

pub struct NewEvent<'a> {
    pub source: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub headers: serde_json::Value,
    pub body: &'a str,
    pub replay_of: Option<Uuid>,
}

impl Store {
    pub async fn record_event(&self, ev: NewEvent<'_>) -> Result<Uuid, WalletError> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"insert into inbound_events (id, source, method, path, headers, body, replay_of)
               values ($1, $2, $3, $4, $5, $6, $7)"#,
        )
        .bind(id)
        .bind(ev.source)
        .bind(ev.method)
        .bind(ev.path)
        .bind(ev.headers)
        .bind(ev.body)
        .bind(ev.replay_of)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;
        Ok(id)
    }

    pub async fn finish_event(
        &self,
        id: Uuid,
        signature_ok: Option<bool>,
        outcome: &str,
        detail: Option<&str>,
        journal_id: Option<Uuid>,
    ) -> Result<(), WalletError> {
        sqlx::query(
            "update inbound_events set signature_ok = $2, outcome = $3, detail = $4, journal_id = $5 where id = $1",
        )
        .bind(id)
        .bind(signature_ok)
        .bind(outcome)
        .bind(detail)
        .bind(journal_id)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;
        Ok(())
    }

    pub async fn event(&self, id: Uuid) -> Result<InboundEvent, WalletError> {
        sqlx::query_as::<_, InboundEvent>("select * from inbound_events where id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| WalletError::bad_request("event not found"))
    }

    pub async fn events(
        &self,
        limit: i64,
        source: Option<&str>,
    ) -> Result<Vec<InboundEvent>, WalletError> {
        sqlx::query_as::<_, InboundEvent>(
            r#"select * from inbound_events
               where ($2::text is null or source = $2)
               order by received_at desc limit $1"#,
        )
        .bind(limit)
        .bind(source)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn event_stats(&self) -> Result<EventStats, WalletError> {
        sqlx::query_as::<_, EventStats>(
            r#"select count(*)::bigint as received,
                      count(*) filter (where outcome = 'duplicate')::bigint as duplicates,
                      count(*) filter (where outcome = 'rejected')::bigint as rejected,
                      count(*) filter (where outcome = 'error')::bigint as errors
               from inbound_events where received_at > now() - interval '24 hours'"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
