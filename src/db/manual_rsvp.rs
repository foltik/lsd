use crate::db::rsvp::AttendeeEdit;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ManualRsvp {
    pub event_id: i64,
    pub user_id: i64,
    pub creator_user_id: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
    pub note: Option<String>,
}

impl ManualRsvp {
    pub async fn create(
        db: &Db, event_id: i64, user_id: i64, creator_user_id: i64, note: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT INTO manual_rsvps (event_id, user_id, creator_user_id, note) VALUES (?, ?, ?, ?)",
            event_id,
            user_id,
            creator_user_id,
            note,
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

    pub async fn count_for_event(db: &Db, event_id: i64) -> Result<i64> {
        let row = sqlx::query!("SELECT COUNT(*) AS 'count!' FROM manual_rsvps WHERE event_id = ?", event_id)
            .fetch_one(db)
            .await?;
        Ok(row.count)
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

    pub async fn update_note(db: &Db, event_id: i64, user_id: i64, note: Option<&str>) -> Result<()> {
        sqlx::query!(
            "UPDATE manual_rsvps SET note = ?, updated_at = CURRENT_TIMESTAMP WHERE event_id = ? AND user_id = ?",
            note,
            event_id,
            user_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn lookup_for_edit(db: &Db, event_id: i64, user_id: i64) -> Result<Option<AttendeeEdit>> {
        Ok(sqlx::query_as!(
            AttendeeEdit,
            "SELECT u.first_name, u.last_name, u.email, mr.note
             FROM manual_rsvps mr
             JOIN users u ON u.id = mr.user_id
             WHERE mr.event_id = ? AND mr.user_id = ?",
            event_id,
            user_id,
        )
        .fetch_optional(db)
        .await?)
    }
}
