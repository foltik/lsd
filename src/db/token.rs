use rand::Rng;
use rand::rngs::OsRng;

use crate::prelude::*;

/// A token which can be used to authenticate as a user.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct SessionToken {
    pub user_id: i64,
    pub token: String,
    pub created_at: NaiveDateTime,
}

/// A token which can be used to login as a user.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct LoginToken {
    pub user_id: i64,
    pub token: String,
    pub created_at: NaiveDateTime,
    pub used_at: Option<NaiveDateTime>,
}

impl SessionToken {
    /// Create a new session token for a user.
    pub async fn create(db: &Db, user: &User) -> AppResult<String> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());

        sqlx::query!("INSERT INTO session_tokens (user_id, token) VALUES (?, ?)", user.id, token)
            .execute(db)
            .await?;

        Ok(token)
    }
}

impl LoginToken {
    /// Create a new login token for an email address.
    pub async fn create(db: &Db, user: &User) -> AppResult<String> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());

        sqlx::query!("INSERT INTO login_tokens (user_id, token) VALUES (?, ?)", user.id, token)
            .execute(db)
            .await?;

        Ok(token)
    }
}
