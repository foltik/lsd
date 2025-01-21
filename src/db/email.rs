use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

/// A record of a an email which has been sent.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Email {
    pub id: i64,
    pub reference_id: Option<i64>,
    pub kind: String,
    pub address: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
}

impl Email {
    /// A login email.
    pub const LOGIN: &'static str = "login";
    /// An email containing a post.
    pub const POST: &'static str = "post";

    /// Create the `emails` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emails ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                reference_id INTEGER, \
                kind TEXT NOT NULL, \
                address TEXT NOT NULL, \
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

    /// Lookup an email referencing another database entry.
    pub async fn lookup_ref(db: &Db, kind: &str, reference_id: i64, address: &str) -> Result<Option<Email>> {
        let res = sqlx::query_as::<_, Email>(
            "SELECT * FROM emails WHERE kind = ? AND reference_id = ? AND address = ?",
        )
        .bind(kind)
        .bind(reference_id)
        .bind(address)
        .fetch_optional(db)
        .await?;
        Ok(res)
    }

    /// Create a new email record.
    pub async fn create(db: &Db, kind: &str, address: &str) -> Result<i64> {
        let res = sqlx::query("INSERT INTO emails (kind, address) VALUES (?, ?)")
            .bind(kind)
            .bind(address)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    /// Create a new email record referencing another database entry.
    pub async fn create_ref(db: &Db, kind: &str, reference_id: i64, address: &str) -> Result<i64> {
        let res = sqlx::query("INSERT INTO emails (kind, reference_id, address) VALUES (?, ?, ?)")
            .bind(kind)
            .bind(reference_id)
            .bind(address)
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
