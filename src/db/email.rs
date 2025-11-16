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
    pub notification_id: Option<i64>,
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

    /// Create email entries for sending the given post to all users on the given list.
    /// Returns rows with `sent_at` set if the post was already emailed to a user.
    pub async fn create_send_posts(db: &Db, post_id: i64, list_id: i64) -> AppResult<Vec<Email>> {
        let existing = sqlx::query_as!(
            Email,
            r#"
            SELECT e.* FROM emails e
            WHERE e.post_id = ? AND e.list_id = ?
                AND ifnull(e.sent_at, '') = (
                    SELECT ifnull(MAX(ee.sent_at), '')
                    FROM emails ee
                    WHERE ee.address = e.address
                    AND ee.post_id = e.post_id
                    AND ee.list_id = e.list_id
                );
            "#,
            post_id,
            list_id
        )
        .fetch_all(db)
        .await?;

        let new = sqlx::query_as!(
            Email,
            r#"
             INSERT INTO emails (kind, address, post_id, list_id)
                 SELECT ?, lm.email, ?, lm.list_id
                 FROM list_members lm
                 WHERE lm.list_id = ?
                   AND NOT EXISTS (
                       SELECT 1
                       FROM emails e
                       WHERE e.address = lm.email
                         AND e.post_id = ?
                         AND e.list_id = lm.list_id
                   )
             RETURNING *
             "#,
            Email::POST,
            post_id,
            list_id,
            post_id,
        )
        .fetch_all(db)
        .await?;

        let mut all = existing;
        all.extend(new);
        Ok(all)
    }

    /// Create email entries for resending the given post to all users on the given list.
    pub async fn create_resend_posts(db: &Db, post_id: i64, list_id: i64) -> AppResult<Vec<Email>> {
        let existing_unsent = sqlx::query_as!(
            Email,
            r#"
            SELECT e.*
            FROM emails e
            WHERE e.post_id = ? AND e.list_id = ?
              AND e.sent_at IS NULL
              AND e.id = (
                  SELECT MAX(ee.id)
                  FROM emails ee
                  WHERE ee.address = e.address
                    AND ee.post_id = e.post_id
                    AND ee.list_id = e.list_id
                    AND ee.sent_at IS NULL
              );
            "#,
            post_id,
            list_id,
        )
        .fetch_all(db)
        .await?;

        let new = sqlx::query_as!(
            Email,
            r#"
            INSERT INTO emails (kind, address, post_id, list_id)
                SELECT ?, lm.email, ?, lm.list_id
                FROM list_members lm
                WHERE lm.list_id = ?
                  AND NOT EXISTS (
                      SELECT 1
                      FROM emails e
                      WHERE e.address = lm.email
                        AND e.post_id = ?
                        AND e.list_id = lm.list_id
                        AND e.sent_at IS NULL
                  )
            RETURNING *;
            "#,
            Email::POST,
            post_id,
            list_id,
            post_id,
        )
        .fetch_all(db)
        .await?;

        let mut all = existing_unsent;
        all.extend(new);
        Ok(all)
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
