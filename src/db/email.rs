use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

/// A record of a an email which has been sent.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Email {
    pub id: i64,
    pub kind: String,
    pub user_id: i64,
    pub post_id: Option<i64>,
    pub list_id: Option<i64>,
    pub event_id: Option<i64>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
}

impl Email {
    /// A login email.
    pub const LOGIN: &'static str = "login";
    /// A registration email.
    pub const REGISTER: &'static str = "register";
    /// An email containing a post.
    pub const POST: &'static str = "post";

    /// Create the `emails` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emails ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                kind TEXT NOT NULL, \
                email TEXT NOT NULL, \
                user_id INTEGER NOT NULL, \
                post_id INTEGER, \
                list_id INTEGER, \
                event_id INTEGER, \
                error TEXT, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                sent_at TIMESTAMP, \
                opened_at TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Lookup an email by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Email>> {
        let res = sqlx::query_as::<_, Email>("SELECT * FROM emails WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(res)
    }

    /// Lookup an email by address, post, and list.
    pub async fn lookup_post(db: &Db, user_id: i64, post_id: i64, list_id: i64) -> Result<Option<Email>> {
        let res = sqlx::query_as::<_, Email>(
            "SELECT * FROM emails \
             WHERE user_id = ? AND post_id = ? AND list_id = ?",
        )
        .bind(user_id)
        .bind(post_id)
        .bind(list_id)
        .fetch_optional(db)
        .await?;
        Ok(res)
    }

    /// Create a new email record.
    pub async fn create_login(db: &Db, email: &str, user_id: i64) -> Result<i64> {
        let res = sqlx::query("INSERT INTO emails (kind, email, user_id) VALUES (?, ?, ?)")
            .bind(Email::LOGIN)
            .bind(email)
            .bind(user_id)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    pub async fn create_register(db: &Db, email: &str) -> Result<i64> {
        let res = sqlx::query("INSERT INTO emails (kind, email) VALUES (?, ?)")
            .bind(Email::REGISTER)
            .bind(email)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    /// Create a new email record referencing another database entry.
    pub async fn create_post(db: &Db, email: &str, user_id: i64, post_id: i64, list_id: i64) -> Result<i64> {
        let res = sqlx::query("INSERT INTO emails (kind, email, user_id, post_id, list_id) VALUES (?, ?, ?, ?, ?)")
            .bind(Email::POST)
            .bind(email)
            .bind(user_id)
            .bind(post_id)
            .bind(list_id)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    /// Mark an email as sent.
    pub async fn mark_sent(db: &Db, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE emails SET sent_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Mark an email as sent.
    pub async fn mark_error(db: &Db, id: i64, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE emails SET sent_at = CURRENT_TIMESTAMP, error = ? \
             WHERE id = ?",
        )
        .bind(error)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Mark an email as opened.
    pub async fn mark_opened(db: &Db, id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE emails SET opened_at = CURRENT_TIMESTAMP \
             WHERE id = ? AND opened_at IS NULL",
        )
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }
}
