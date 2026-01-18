use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ManualRsvp {
    pub event_id: i64,
    pub user_id: i64,
    pub creator_user_id: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

impl ManualRsvp {
    pub async fn create(db: &Db, event_id: i64, user_id: i64, creator_user_id: i64) -> Result<()> {
        sqlx::query!(
            "INSERT INTO manual_rsvps (event_id, user_id, creator_user_id) VALUES (?, ?, ?)",
            event_id,
            user_id,
            creator_user_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete(db: &Db, event_id: i64, user_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM manual_rsvps WHERE event_id = ? AND user_id = ?", event_id, user_id,)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn exists(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        let row = sqlx::query!(
            "SELECT event_id FROM manual_rsvps WHERE event_id = ? AND user_id = ?",
            event_id,
            user_id,
        )
        .fetch_optional(db)
        .await?;
        Ok(row.is_some())
    }

    pub async fn set_checkin_at(db: &Db, event_id: i64, user_id: i64) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE manual_rsvps SET checkin_at = CURRENT_TIMESTAMP WHERE event_id = ? AND user_id = ? RETURNING checkin_at AS 'checkin_at!'",
            event_id,
            user_id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.checkin_at)
    }

    pub async fn clear_checkin_at(db: &Db, event_id: i64, user_id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE manual_rsvps SET checkin_at = NULL WHERE event_id = ? AND user_id = ?",
            event_id,
            user_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }
}
