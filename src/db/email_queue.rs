use crate::prelude::*;

/// A batch of emails to be enqueued.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct EmailBatch {
    pub id: i64,
    pub size: i64,
    pub sent: i64,
    pub errored: i64,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl EmailBatch {
    pub async fn create_single(db: &Db) -> Result<Self> {
        Self::create(db, 1).await
    }
    pub async fn create(db: &Db, size: usize) -> Result<Self> {
        let size = size as i64;
        let batch = sqlx::query_as!(
            Self,
            r#"INSERT INTO email_batches (size, sent, errored)
               VALUES (?, 0, 0)
               RETURNING *"#,
            size,
        )
        .fetch_one(db)
        .await?;
        Ok(batch)
    }

    pub async fn enqueue_front(&self, db: &Db) -> Result<()> {
        sqlx::query!(
            "INSERT INTO email_queue (batch_id, position)
             SELECT ?, COALESCE(MIN(position) - 1, 0) FROM email_queue",
            self.id,
        )
        .execute(db)
        .await?;
        Ok(())
    }
    pub async fn enqueue_back(&self, db: &Db) -> Result<()> {
        sqlx::query!(
            "INSERT INTO email_queue (batch_id, position)
             SELECT ?, COALESCE(MAX(position) + 1, 0) FROM email_queue",
            self.id,
        )
        .execute(db)
        .await?;
        Ok(())
    }
    pub async fn dequeue(&self, db: &Db) -> Result<()> {
        sqlx::query!("DELETE FROM email_queue WHERE batch_id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn next(db: &Db) -> Result<Option<Email>> {
        let rows = sqlx::query_as!(
            Email,
            r#"
            SELECT e.*, u.email as address
            FROM email_queue q
            JOIN email_batches b ON b.id = q.batch_id
            JOIN emails e ON e.batch_id = b.id
            JOIN users u ON u.id = e.user_id
            WHERE e.sent_at IS NULL AND e.errored_at IS NULL
            ORDER BY q.position, e.id
            LIMIT 1
            "#,
        )
        .fetch_optional(db)
        .await?;

        Ok(rows)
    }

    pub async fn inc_sent(db: &Db, batch_id: i64) -> Result<EmailBatch> {
        Self::inc(db, batch_id, 1, 0).await
    }
    pub async fn inc_errored(db: &Db, batch_id: i64) -> Result<EmailBatch> {
        Self::inc(db, batch_id, 0, 1).await
    }
    async fn inc(db: &Db, batch_id: i64, bump_sent: usize, bump_errored: usize) -> Result<EmailBatch> {
        let sent_inc = bump_sent as i64;
        let errored_inc = bump_errored as i64;
        let batch = sqlx::query_as!(
            Self,
            "UPDATE email_batches
             SET sent = sent + ?, errored = errored + ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?
             RETURNING *",
            sent_inc,
            errored_inc,
            batch_id
        )
        .fetch_one(db)
        .await?;

        if batch.sent + batch.errored == batch.size {
            batch.dequeue(db).await?;
        }

        Ok(batch)
    }
}
