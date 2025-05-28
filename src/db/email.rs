use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

/// A record of a an email which has been sent.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Email {
    pub id: i64,
    pub kind: String,
    pub address: String,
    pub post_id: Option<i64>,
    pub list_id: Option<i64>,
    pub user_id: Option<i64>,
    pub event_id: Option<i64>,
    pub error: Option<String>,
    pub created_at: NaiveDateTime,
    pub sent_at: Option<NaiveDateTime>,
    pub opened_at: Option<NaiveDateTime>,
}

impl Email {
    /// A login email.
    pub const LOGIN: &'static str = "login";
    /// An email containing a post.
    pub const POST: &'static str = "post";

    /// Lookup an email by id.
    pub async fn lookup(db: &Db, id: i64) -> AppResult<Option<Email>> {
        let res = sqlx::query_as!(Self, r#"SELECT * FROM emails WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?;
        Ok(res)
    }

    /// Lookup an email by address, post, and list.
    pub async fn lookup_post(db: &Db, address: &str, post_id: i64, list_id: i64) -> AppResult<Option<Email>> {
        let res = sqlx::query_as!(
            Self,
            r#"SELECT * FROM emails
               WHERE address = ? AND post_id = ? AND list_id = ?"#,
            address,
            post_id,
            list_id
        )
        .fetch_optional(db)
        .await?;
        Ok(res)
    }

    /// Create a new email record.
    pub async fn create_login(db: &Db, address: &str) -> AppResult<i64> {
        let res = sqlx::query!("INSERT INTO emails (kind, address) VALUES (?, ?)", Email::LOGIN, address)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    /// Create a new email record referencing another database entry.
    pub async fn create_post(db: &Db, address: &str, post_id: i64, list_id: i64) -> AppResult<i64> {
        let res = sqlx::query!(
            r#"INSERT INTO emails (kind, address, post_id, list_id) VALUES (?, ?, ?, ?)"#,
            Email::POST,
            address,
            post_id,
            list_id
        )
        .execute(db)
        .await?;
        Ok(res.last_insert_rowid())
    }

    /// Mark an email as sent.
    pub async fn mark_sent(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE emails SET sent_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Mark an email as sent.
    pub async fn mark_error(db: &Db, id: i64, error: &str) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE emails SET sent_at = CURRENT_TIMESTAMP, error = ?
               WHERE id = ?"#,
            error,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Mark an email as opened.
    pub async fn mark_opened(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE emails SET opened_at = CURRENT_TIMESTAMP
               WHERE id = ? AND opened_at IS NULL"#,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }
}
