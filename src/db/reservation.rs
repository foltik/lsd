#![allow(unused)]

use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Rsvp {
    pub id: i64,
    pub user_id: i64,
    pub event_id: i64,
    pub spot_id: i64,
    pub transaction_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

pub struct EventRsvp {
    pub spot_id: i64,
    pub price: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct UpdateRsvp {
    pub user_id: i64,
    pub event_id: i64,
    pub spot_id: i64,
    pub transaction_id: Option<i64>,

    pub checkin_at: Option<NaiveDateTime>,
}

impl Rsvp {
    pub async fn list(db: &Db) -> AppResult<Vec<Rsvp>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM reservations").fetch_all(db).await?)
    }

    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT r.spot_id, t.price
             FROM reservations r
             LEFT JOIN transactions t ON t.id = r.transaction_id
             WHERE r.event_id = ?",
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn create(db: &Db, rsvp: &UpdateRsvp) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO reservations
               (user_id, event_id, spot_id, transaction_id, checkin_at)
               VALUES (?, ?, ?, ?, ?)"#,
            rsvp.user_id,
            rsvp.event_id,
            rsvp.spot_id,
            rsvp.transaction_id,
            rsvp.checkin_at,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn update(db: &Db, id: i64, rsvp: &UpdateRsvp) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE reservations
               SET user_id = ?,
                   event_id = ?,
                   spot_id = ?,
                   transaction_id = ?,
                   checkin_at = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            rsvp.user_id,
            rsvp.event_id,
            rsvp.spot_id,
            rsvp.transaction_id,
            rsvp.checkin_at,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM reservations WHERE id = ?"#, id).execute(db).await?;
        Ok(())
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Rsvp>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM reservations WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }
}
