use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::{Rng, rngs::OsRng};

use super::Db;

/// A token which can be used to authenticate as a user.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct SessionToken {
    pub id: i64,
    pub user_id: i64,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

/// A token which can be used to login or register.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct LoginToken {
    pub id: i64,
    pub email: String,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

impl SessionToken {
    /// Create a new session token for a user.
    pub async fn create(db: &Db, user_id: i64) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query!("INSERT INTO session_tokens (user_id, token) VALUES (?, ?)", user_id, token)
            .execute(db)
            .await?;

        Ok(token)
    }
}

impl LoginToken {
    /// Create a new login token for an email address.
    pub async fn create(db: &Db, email: &str) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query!("INSERT INTO login_tokens (email, token) VALUES (?, ?)", email, token)
            .execute(db)
            .await?;

        Ok(token)
    }

    /// Lookup the email address for the given login token, if it's valid.
    pub async fn lookup_email(db: &Db, token: &str) -> Result<Option<String>> {
        let row = sqlx::query_scalar!("SELECT email FROM login_tokens WHERE token = ?", token)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
}
