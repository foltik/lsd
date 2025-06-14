#![allow(unused)]

use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Rsvp {
    pub id: i64,
    pub user_id: i64,
    pub event_id: i64,
    pub ticket_id: i64,
    pub transaction_id: Option<i64>,
    pub price: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

#[derive(serde::Deserialize)]
pub struct UpdateRsvp {
    pub user_id: i64,
    pub event_id: i64,
    pub ticket_id: i64,
    pub transaction_id: Option<i64>,
    pub price: Option<i64>,

    pub checkin_at: Option<NaiveDateTime>,
}

impl Rsvp {
    pub async fn list(db: &Db) -> AppResult<Vec<Rsvp>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM rsvps").fetch_all(db).await?)
    }

    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<Rsvp>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM rsvps WHERE event_id = ?", event_id)
            .fetch_all(db)
            .await?)
    }

    pub async fn create(db: &Db, rsvp: &UpdateRsvp) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO rsvps
               (user_id, event_id, ticket_id, transaction_id, price, checkin_at)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            rsvp.user_id,
            rsvp.event_id,
            rsvp.ticket_id,
            rsvp.transaction_id,
            rsvp.price,
            rsvp.checkin_at,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn update(db: &Db, id: i64, rsvp: &UpdateRsvp) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE rsvps
               SET user_id = ?,
                   event_id = ?,
                   ticket_id = ?,
                   transaction_id = ?,
                   price = ?,
                   checkin_at = ?
               WHERE id = ?"#,
            rsvp.user_id,
            rsvp.event_id,
            rsvp.ticket_id,
            rsvp.transaction_id,
            rsvp.price,
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

    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Rsvp>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvps WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }
}
