use crate::prelude::*;

/// A record of a an email which has been sent.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Email {
    pub id: i64,
    pub kind: String,
    pub user_id: i64,
    pub user_version: i64,
    pub address: String,

    pub post_id: Option<i64>,
    pub list_id: Option<i64>,
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
        let res = sqlx::query_as!(
            Self,
            r#"SELECT e.*, u.email as address FROM emails e
               JOIN users u ON u.id = e.user_id
               WHERE e.id = ?
            "#,
            id
        )
        .fetch_optional(db)
        .await?;
        Ok(res)
    }

    /// Create a new email record.
    pub async fn create_login(db: &Db, user: &User) -> AppResult<i64> {
        let res = sqlx::query!(
            "INSERT INTO emails (kind, user_id, user_version) VALUES (?, ?, ?)",
            Email::LOGIN,
            user.id,
            user.version
        )
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
            SELECT e.*, u.email as address FROM emails e
            JOIN users u ON u.id = e.user_id
            WHERE e.post_id = ? AND e.list_id = ?
                AND ifnull(e.sent_at, '') = (
                    SELECT ifnull(MAX(ee.sent_at), '')
                    FROM emails ee
                    WHERE ee.user_id = e.user_id
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
             INSERT INTO emails (kind, user_id, post_id, list_id)
                 SELECT ?, u.email, ?, lm.list_id
                 FROM list_members lm
                 JOIN users u ON u.id = lm.user_id
                 WHERE lm.list_id = ?
                   AND NOT EXISTS (
                       SELECT 1
                       FROM emails ee
                       WHERE ee.user_id = u.id
                         AND ee.post_id = ?
                         AND ee.list_id = lm.list_id
                   )
             RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
             ) AS address
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
            SELECT e.*, u.email as address
            FROM emails e
            JOIN users u ON u.id = e.user_id
            WHERE e.post_id = ? AND e.list_id = ?
              AND e.sent_at IS NULL
              AND e.id = (
                  SELECT MAX(ee.id)
                  FROM emails ee
                  WHERE ee.user_id = e.user_id
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
            INSERT INTO emails (kind, user_id, post_id, list_id)
                SELECT ?, u.id, ?, lm.list_id
                FROM list_members lm
                JOIN users u ON u.id = lm.user_id
                WHERE lm.list_id = ?
                  AND NOT EXISTS (
                      SELECT 1
                      FROM emails e
                      WHERE e.user_id = u.id
                        AND e.post_id = ?
                        AND e.list_id = lm.list_id
                        AND e.sent_at IS NULL
                  )
            RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
            ) as address
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
