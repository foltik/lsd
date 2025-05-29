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

    /// Create a new email record.
    pub async fn create_login(db: &Db, address: &str) -> AppResult<i64> {
        let res = sqlx::query!("INSERT INTO emails (kind, address) VALUES (?, ?)", Email::LOGIN, address)
            .execute(db)
            .await?;
        Ok(res.last_insert_rowid())
    }

    /// Create email entries for the given post for all users on the given list if they don't already exist.
    pub async fn create_posts(db: &Db, post_id: i64, list_id: i64) -> AppResult<Vec<Email>> {
        let emails = sqlx::query_as!(
            Email,
            r#"
            INSERT INTO emails (kind, address, post_id, list_id)
                SELECT ?, email, ?, list_id
                FROM list_members
                WHERE list_id = ?
            ON CONFLICT(address, post_id, list_id) DO UPDATE
                SET kind = emails.kind -- no-op so the rows are still returned
            RETURNING *;
            "#,
            Email::POST,
            post_id,
            list_id,
        )
        .fetch_all(db)
        .await?;
        Ok(emails)
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
