use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, Rng};

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
    /// Create the `session_tokens` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_tokens ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                user_id INTEGER NOT NULL, \
                token TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                FOREIGN KEY (user_id) REFERENCES users(id) \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new session token for a user.
    pub async fn create(db: &Db, user_id: i64) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query("INSERT INTO session_tokens (user_id, token) VALUES (?, ?)")
            .bind(user_id)
            .bind(&token)
            .execute(db)
            .await?;

        Ok(token)
    }
}

impl LoginToken {
    /// Create the `login_tokens` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS login_tokens ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                email TEXT NOT NULL, \
                token TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new login token for an email address.
    pub async fn create(db: &Db, email: &str) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query("INSERT INTO login_tokens (email, token) VALUES (?, ?)")
            .bind(email)
            .bind(&token)
            .execute(db)
            .await?;

        Ok(token)
    }

    /// Lookup the email address for the given login token, if it's valid.
    pub async fn lookup_email(db: &Db, token: &str) -> Result<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>("SELECT email FROM login_tokens WHERE token = ?")
            .bind(token)
            .fetch_optional(db)
            .await?;
        Ok(row.map(|r| r.0))
    }
}
