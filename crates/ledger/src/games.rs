use crate::Store;
use domain::traits::GameListing;
use domain::WalletError;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Game {
    pub id: i64,
    pub provider: String,
    pub provider_game_id: String,
    pub name: String,
    pub title: String,
    pub category: String,
    pub thumbnail: Option<String>,
    pub game_type: String,
    pub is_premium: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CategoryCount {
    pub category: String,
    pub count: i64,
}

impl Store {
    pub async fn upsert_games(
        &self,
        provider: &str,
        listings: &[GameListing],
    ) -> Result<usize, WalletError> {
        let mut tx = self.pool.begin().await.map_err(anyhow::Error::from)?;
        for g in listings {
            sqlx::query(
                r#"insert into games (provider, provider_game_id, name, title, category, thumbnail, game_type, is_premium, synced_at)
                   values ($1, $2, $3, $4, $5, $6, $7, $8, now())
                   on conflict (provider, provider_game_id) do update set
                     name = excluded.name, title = excluded.title, category = excluded.category,
                     thumbnail = excluded.thumbnail, game_type = excluded.game_type,
                     is_premium = excluded.is_premium, synced_at = now()"#,
            )
            .bind(provider)
            .bind(&g.provider_game_id)
            .bind(&g.name)
            .bind(&g.title)
            .bind(&g.category)
            .bind(&g.thumbnail)
            .bind(&g.game_type)
            .bind(g.is_premium)
            .execute(&mut *tx)
            .await
            .map_err(anyhow::Error::from)?;
        }
        tx.commit().await.map_err(anyhow::Error::from)?;
        Ok(listings.len())
    }

    pub async fn games(
        &self,
        category: Option<&str>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Game>, WalletError> {
        sqlx::query_as::<_, Game>(
            r#"select id, provider, provider_game_id, name, title, category, thumbnail, game_type, is_premium
               from games
               where ($1::text is null or category = $1)
                 and ($2::text is null or title ilike '%' || $2 || '%')
               order by category, title limit $3 offset $4"#,
        )
        .bind(category)
        .bind(search)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn game(&self, id: i64) -> Result<Option<Game>, WalletError> {
        sqlx::query_as::<_, Game>(
            "select id, provider, provider_game_id, name, title, category, thumbnail, game_type, is_premium from games where id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    pub async fn categories(&self) -> Result<Vec<CategoryCount>, WalletError> {
        sqlx::query_as::<_, CategoryCount>(
            "select category, count(*)::bigint as count from games group by category order by count desc, category",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
