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
    pub duration_ms: Option<i32>,
    pub reply_status: Option<i32>,
    pub reply_body: Option<serde_json::Value>,
}

/// One hour of callbacks split by outcome, for the operator chart.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TimelineBucket {
    pub hour: DateTime<Utc>,
    pub applied: i64,
    pub duplicates: i64,
    pub rejected: i64,
    pub errors: i64,
}

/// One row per provider for the operator health page.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SourceHealth {
    pub source: String,
    pub received_24h: i64,
    pub applied_24h: i64,
    pub duplicates_24h: i64,
    pub rejected_24h: i64,
    pub errors_24h: i64,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error_detail: Option<String>,
    pub avg_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
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
            r#"update inbound_events
               set signature_ok = $2, outcome = $3, detail = $4, journal_id = $5,
                   duration_ms = (extract(epoch from (now() - received_at)) * 1000)::int
               where id = $1"#,
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

impl Store {
    pub async fn set_event_reply(
        &self,
        id: Uuid,
        status: u16,
        body: &serde_json::Value,
    ) -> Result<(), WalletError> {
        sqlx::query("update inbound_events set reply_status = $2, reply_body = $3 where id = $1")
            .bind(id)
            .bind(status as i32)
            .bind(body)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }

    /// The last 24 hours in hourly buckets, oldest first. Empty hours are included.
    pub async fn timeline_24h(&self) -> Result<Vec<TimelineBucket>, WalletError> {
        sqlx::query_as::<_, TimelineBucket>(
            r#"with hours as (
                 select generate_series(date_trunc('hour', now()) - interval '23 hours', date_trunc('hour', now()), interval '1 hour') as hour
               )
               select h.hour,
                      count(e.id) filter (where e.outcome in ('applied', 'read', 'ignored'))::bigint as applied,
                      count(e.id) filter (where e.outcome = 'duplicate')::bigint as duplicates,
                      count(e.id) filter (where e.outcome = 'rejected')::bigint as rejected,
                      count(e.id) filter (where e.outcome = 'error')::bigint as errors
               from hours h
               left join inbound_events e on date_trunc('hour', e.received_at) = h.hour
               group by h.hour order by h.hour"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    /// Per-source summary of the last 24 hours plus the last success and error ever.
    pub async fn source_health(&self) -> Result<Vec<SourceHealth>, WalletError> {
        sqlx::query_as::<_, SourceHealth>(
            r#"with recent as (
                 select * from inbound_events where received_at > now() - interval '24 hours'
               ),
               per_source as (
                 select source,
                        count(*)::bigint as received_24h,
                        count(*) filter (where outcome in ('applied', 'read', 'ignored'))::bigint as applied_24h,
                        count(*) filter (where outcome = 'duplicate')::bigint as duplicates_24h,
                        count(*) filter (where outcome = 'rejected')::bigint as rejected_24h,
                        count(*) filter (where outcome = 'error')::bigint as errors_24h,
                        avg(duration_ms)::float8 as avg_duration_ms,
                        percentile_cont(0.95) within group (order by duration_ms)::float8 as p95_duration_ms
                 from recent group by source
               ),
               sources as (
                 select distinct source from inbound_events
               )
               select s.source,
                      coalesce(p.received_24h, 0) as received_24h,
                      coalesce(p.applied_24h, 0) as applied_24h,
                      coalesce(p.duplicates_24h, 0) as duplicates_24h,
                      coalesce(p.rejected_24h, 0) as rejected_24h,
                      coalesce(p.errors_24h, 0) as errors_24h,
                      (select max(received_at) from inbound_events e
                        where e.source = s.source and e.outcome in ('applied', 'duplicate', 'read', 'ignored')) as last_success_at,
                      (select max(received_at) from inbound_events e
                        where e.source = s.source and e.outcome in ('rejected', 'error')) as last_error_at,
                      (select detail from inbound_events e
                        where e.source = s.source and e.outcome in ('rejected', 'error')
                        order by received_at desc limit 1) as last_error_detail,
                      p.avg_duration_ms,
                      p.p95_duration_ms
               from sources s left join per_source p on p.source = s.source
               order by s.source"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
