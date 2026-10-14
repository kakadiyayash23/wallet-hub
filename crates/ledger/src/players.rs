use crate::Store;
use domain::traits::PlayerRef;
use domain::{PlayerId, WalletError};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct PlayerRow {
    id: Uuid,
    external_token: String,
    display_name: String,
    locale: String,
    currency: String,
}

impl From<PlayerRow> for PlayerRef {
    fn from(r: PlayerRow) -> Self {
        PlayerRef {
            id: PlayerId(r.id),
            external_token: r.external_token,
            display_name: r.display_name,
            locale: r.locale,
            currency: r.currency,
        }
    }
}

impl Store {
    /// Dev login: create the player on first use. The external token is what
    /// every provider sees as the username.
    pub async fn ensure_player(
        &self,
        display_name: &str,
        locale: &str,
        currency: &str,
    ) -> Result<PlayerRef, WalletError> {
        let token = slugify(display_name);
        let row = sqlx::query_as::<_, PlayerRow>(
            r#"insert into players (id, external_token, display_name, locale, currency)
               values ($1, $2, $3, $4, $5)
               on conflict (external_token) do update set locale = excluded.locale
               returning id, external_token, display_name, locale, currency"#,
        )
        .bind(Uuid::new_v4())
        .bind(&token)
        .bind(display_name)
        .bind(locale)
        .bind(currency)
        .fetch_one(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;
        Ok(row.into())
    }

    pub async fn player_by_id(&self, id: PlayerId) -> Result<PlayerRef, WalletError> {
        sqlx::query_as::<_, PlayerRow>(
            "select id, external_token, display_name, locale, currency from players where id = $1",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(anyhow::Error::from)?
        .map(Into::into)
        .ok_or(WalletError::PlayerNotFound)
    }

    pub async fn player_by_token(&self, token: &str) -> Result<PlayerRef, WalletError> {
        sqlx::query_as::<_, PlayerRow>(
            "select id, external_token, display_name, locale, currency from players where external_token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(anyhow::Error::from)?
        .map(Into::into)
        .ok_or(WalletError::PlayerNotFound)
    }

    pub async fn set_locale(&self, id: PlayerId, locale: &str) -> Result<(), WalletError> {
        sqlx::query("update players set locale = $2 where id = $1")
            .bind(id.0)
            .bind(locale)
            .execute(&self.pool)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(())
    }
}

fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("player");
    }
    out
}
