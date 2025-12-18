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

    /// An event invitation email.
    pub const EVENT_INVITE: &'static str = "event/invite";
    /// An event purchase confirmation email.
    pub const EVENT_CONFIRMATION: &'static str = "event/confirmation";
    /// An event day-of info email.
    pub const EVENT_DAYOF: &'static str = "event/dayof";

    /// Lookup an email by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Email>> {
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
    pub async fn create_login(db: &Db, user: &User) -> Result<i64> {
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
    pub async fn create_send_posts(db: &Db, post_id: i64, list_id: i64) -> Result<Vec<Email>> {
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
             INSERT INTO emails (kind, user_id, user_version, post_id, list_id)
                 SELECT ?, u.id, uh.version, ?, lm.list_id
                 FROM list_members lm
                 JOIN users u ON u.id = lm.user_id
                 JOIN user_history uh ON uh.user_id = u.id
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

    /// Create email entries for sending the given post to all users on the given list.
    /// Returns rows with `sent_at` set if the post was already emailed to a user.
    pub async fn create_send_invites(db: &Db, event_id: i64, list_id: i64) -> Result<Vec<Email>> {
        let existing = sqlx::query_as!(
            Email,
            r#"
            SELECT e.*, u.email as address FROM emails e
            JOIN users u ON u.id = e.user_id
            WHERE e.kind = ? AND e.event_id = ? AND e.list_id = ?
                AND ifnull(e.sent_at, '') = (
                    SELECT ifnull(MAX(ee.sent_at), '')
                    FROM emails ee
                    WHERE ee.kind = e.kind
                    AND ee.list_id = e.list_id
                    AND ee.user_id = e.user_id
                    AND ee.event_id = e.event_id
                );
            "#,
            Email::EVENT_INVITE,
            event_id,
            list_id
        )
        .fetch_all(db)
        .await?;

        let new = sqlx::query_as!(
            Email,
            r#"
             INSERT INTO emails (kind, user_id, user_version, event_id, list_id)
                 SELECT ?, u.id, uh.version, ?, ?
                 FROM list_members lm
                 JOIN users u ON u.id = lm.user_id
                 JOIN user_history uh ON uh.user_id = u.id
                 WHERE lm.list_id = ?
                   AND NOT EXISTS (
                       SELECT 1
                       FROM emails ee
                       WHERE ee.kind = ?
                         AND ee.user_id = u.id
                         AND ee.event_id = ?
                         AND ee.list_id = ?
                   )
             RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
             ) AS address
             "#,
            Email::EVENT_INVITE,
            event_id,
            list_id,
            list_id,
            Email::EVENT_INVITE,
            event_id,
            list_id,
        )
        .fetch_all(db)
        .await?;

        let mut all = existing;
        all.extend(new);
        Ok(all)
    }

    pub async fn have_sent_confirmation(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        let row = sqlx::query!(
            "SELECT id FROM emails WHERE kind = ? AND event_id = ? AND user_id = ?",
            Email::EVENT_CONFIRMATION,
            event_id,
            user_id
        )
        .fetch_optional(db)
        .await?;
        Ok(row.is_some())
    }

    pub async fn create_confirmation(db: &Db, event_id: i64, user_id: i64) -> Result<Email> {
        let row = sqlx::query_as!(
            Email,
            r#"
             INSERT INTO emails (kind, user_id, user_version, event_id)
                 SELECT ?, u.id, uh.version, ?
                 FROM users u
                 JOIN user_history uh ON uh.user_id = u.id
                 WHERE u.id = ?
             RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
             ) AS "address!"
             "#,
            Email::EVENT_CONFIRMATION,
            event_id,
            user_id,
        )
        .fetch_one(db)
        .await?;
        Ok(row)
    }

    pub async fn create_send_dayof_single(db: &Db, event_id: i64, user_id: i64) -> Result<Email> {
        let row = sqlx::query_as!(
            Email,
            r#"
             INSERT INTO emails (kind, user_id, user_version, event_id)
                 SELECT ?, u.id, uh.version, ?
                 FROM users u
                 JOIN user_history uh ON uh.user_id = u.id
                 WHERE u.id = ?
             RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
             ) AS "address!"
             "#,
            Email::EVENT_DAYOF,
            event_id,
            user_id,
        )
        .fetch_one(db)
        .await?;
        Ok(row)
    }

    /// Create email entries for sending the given post to all users on the given list.
    /// Returns rows with `sent_at` set if the post was already emailed to a user.
    pub async fn create_send_dayof_batch(db: &Db, event_id: i64) -> Result<Vec<Email>> {
        let existing = sqlx::query_as!(
            Email,
            r#"
            SELECT e.*, u.email as address FROM emails e
            JOIN users u ON u.id = e.user_id
            WHERE e.kind = ? AND e.event_id = ?
                AND ifnull(e.sent_at, '') = (
                    SELECT ifnull(MAX(ee.sent_at), '')
                    FROM emails ee
                    WHERE ee.kind = e.kind
                    AND ee.user_id = e.user_id
                    AND ee.event_id = e.event_id
                );
            "#,
            Email::EVENT_DAYOF,
            event_id,
        )
        .fetch_all(db)
        .await?;

        let new = sqlx::query_as!(
            Email,
            r#"
             INSERT INTO emails (kind, user_id, user_version, event_id)
                 SELECT ?, u.id, uh.version, ?
                 FROM rsvps r
                 JOIN rsvp_sessions rs ON rs.id = r.session_id
                 JOIN users u ON u.id = r.user_id
                 JOIN user_history uh ON uh.user_id = u.id
                 WHERE rs.event_id = ?
                   AND NOT EXISTS (
                       SELECT 1
                       FROM emails ee
                       WHERE ee.kind = ?
                         AND ee.user_id = u.id
                         AND ee.event_id = ?
                   )
             RETURNING *, (
                SELECT u.email FROM users u
                WHERE u.id = emails.user_id
             ) AS "address!"
             "#,
            Email::EVENT_INVITE,
            event_id,
            event_id,
            Email::EVENT_INVITE,
            event_id,
        )
        .fetch_all(db)
        .await?;

        let mut all = existing;
        all.extend(new);
        Ok(all)
    }

    /// Create email entries for resending the given post to all users on the given list.
    pub async fn create_resend_posts(db: &Db, post_id: i64, list_id: i64) -> Result<Vec<Email>> {
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
            INSERT INTO emails (kind, user_id, user_version, post_id, list_id)
                SELECT ?, u.id, uh.version, ?, lm.list_id
                FROM list_members lm
                JOIN users u ON u.id = lm.user_id
                JOIN user_history uh ON uh.user_id = u.id
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
    pub async fn mark_sent(db: &Db, id: i64) -> Result<()> {
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
    pub async fn mark_error(db: &Db, id: i64, error: &str) -> Result<()> {
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
    pub async fn mark_opened(db: &Db, id: i64) -> Result<()> {
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
