use crate::db::rsvp::AttendeeEdit;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ManualRsvp {
    pub id: i64,
    pub event_id: i64,
    pub creator_user_id: i64,
    pub user_id: Option<i64>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
    pub note: Option<String>,
}

impl ManualRsvp {
    pub async fn create(
        db: &Db, event_id: i64, user_id: Option<i64>, first_name: Option<&str>, last_name: Option<&str>,
        creator_user_id: i64, note: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            "INSERT INTO manual_rsvps (event_id, user_id, first_name, last_name, creator_user_id, note)
             VALUES (?, ?, ?, ?, ?, ?)",
            event_id,
            user_id,
            first_name,
            last_name,
            creator_user_id,
            note,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<ManualRsvp>> {
        Ok(sqlx::query_as!(ManualRsvp, "SELECT * FROM manual_rsvps WHERE id = ?", id)
            .fetch_optional(db)
            .await?)
    }

    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM manual_rsvps WHERE id = ?", id).execute(db).await?;
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
            "SELECT id FROM manual_rsvps WHERE event_id = ? AND user_id = ?",
            event_id,
            user_id,
        )
        .fetch_optional(db)
        .await?;
        Ok(row.is_some())
    }

    pub async fn set_checkin_at(db: &Db, id: i64) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE manual_rsvps SET checkin_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING checkin_at AS 'checkin_at!'",
            id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.checkin_at)
    }

    pub async fn clear_checkin_at(db: &Db, id: i64) -> Result<()> {
        sqlx::query!("UPDATE manual_rsvps SET checkin_at = NULL WHERE id = ?", id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn update_name(db: &Db, id: i64, first_name: &str, last_name: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE manual_rsvps SET first_name = ?, last_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            first_name,
            last_name,
            id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn update_note(db: &Db, id: i64, note: Option<&str>) -> Result<()> {
        sqlx::query!(
            "UPDATE manual_rsvps SET note = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            note,
            id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn lookup_for_edit(db: &Db, id: i64) -> Result<Option<AttendeeEdit>> {
        Ok(sqlx::query_as!(
            AttendeeEdit,
            r#"SELECT
                   COALESCE(u.first_name, mr.first_name) AS "first_name?: String",
                   COALESCE(u.last_name, mr.last_name) AS "last_name?: String",
                   u.email AS "email?",
                   mr.note
               FROM manual_rsvps mr
               LEFT JOIN users u ON u.id = mr.user_id
               WHERE mr.id = ?"#,
            id,
        )
        .fetch_optional(db)
        .await?)
    }
}
