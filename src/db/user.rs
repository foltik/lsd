use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUser {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

impl User {
    /// Create the `users` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                first_name TEXT NOT NULL, \
                last_name TEXT NOT NULL, \
                email TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new user.
    pub async fn create(db: &Db, user: &UpdateUser) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO users \
                (first_name, last_name, email) \
                VALUES (?, ?, ?)",
        )
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.email)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a user by email address, if one exists.
    pub async fn lookup_by_email(db: &Db, email: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
    /// Lookup a user by a login token, if it's valid.
    pub async fn lookup_by_login_token(db: &Db, token: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, User>(
            "SELECT u.* \
             FROM login_tokens t \
             LEFT JOIN users u on u.email = t.email \
             WHERE t.token = ?",
        )
        .bind(token)
        .fetch_optional(db)
        .await?;
        Ok(row)
    }
    /// Lookup a user by a session token, if it's valid.
    pub async fn lookup_by_session_token(db: &Db, token: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT u.* \
             FROM session_tokens t \
             JOIN users u on u.id = t.user_id \
             WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(db)
        .await?;
        Ok(user)
    }
}
