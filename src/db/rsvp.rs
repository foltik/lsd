use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Rsvp {
    pub id: i64,
    pub event_id: i64,
    pub spot_id: i64,
    pub contribution: i64,

    pub status: String,
    pub session_id: i64,
    pub user_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

#[derive(serde::Deserialize)]
pub struct CreateRsvp {
    pub event_id: i64,
    pub spot_id: i64,
    pub contribution: i64,

    pub status: String,
    pub session_id: i64,
    pub user_id: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct UpdateRsvp {
    pub status: String,
    pub user_id: Option<i64>,
    pub checkin_at: Option<NaiveDateTime>,
}

pub struct EventRsvp {
    pub spot_id: i64,
    pub contribution: i64,
}

impl Rsvp {
    pub async fn list(db: &Db) -> AppResult<Vec<Rsvp>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM rsvps").fetch_all(db).await?)
    }
    pub async fn list_for_session(db: &Db, session_id: i64) -> AppResult<Vec<Rsvp>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM rsvps WHERE session_id = ?", session_id)
            .fetch_all(db)
            .await?)
    }
    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT spot_id, contribution FROM rsvps WHERE event_id = ?",
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Rsvp>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvps WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }

    pub async fn create(db: &Db, rsvp: CreateRsvp) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO rsvps
               (event_id, spot_id, contribution, status, session_id, user_id)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            rsvp.event_id,
            rsvp.spot_id,
            rsvp.contribution,
            rsvp.status,
            rsvp.session_id,
            rsvp.user_id,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn update(db: &Db, id: i64, rsvp: &UpdateRsvp) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE rsvps
               SET status = ?,
                   user_id = ?,
                   checkin_at = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            rsvp.status,
            rsvp.user_id,
            rsvp.checkin_at,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM rsvps WHERE id = ?"#, id).execute(db).await?;
        Ok(())
    }
    pub async fn delete_for_session(db: &Db, session_id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM rsvps WHERE session_id = ?"#, session_id)
            .execute(db)
            .await?;
        Ok(())
    }
}
