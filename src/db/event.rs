use chrono::NaiveDateTime;
use serde::Deserialize;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    // TODO: Add a pretty url field, for `https://site/e/{url}`.
    // pub url: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub start_date: NaiveDateTime,
    // TODO: Add an end. Maybe rename to just `start` and `end`.
    // pub end_date: NaiveDateTime,
    pub target_revenue: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize)]
pub struct ReservationType {
    pub event_id: i64,
    pub name: String,
    pub details: Option<String>,
    pub quantity: i64,
    pub min_contribution: i64,
    pub max_contribution: i64,
    pub recommended_contribution: i64,
}

#[derive(Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    pub target_revenue: i64,
    pub reservation_types: Vec<ReservationType>,
}

impl Event {
    // List all events.
    pub async fn list(db: &Db) -> AppResult<Vec<Event>> {
        let events = sqlx::query_as!(Self, "SELECT * FROM events").fetch_all(db).await?;
        Ok(events)
    }

    // Create a new event.
    pub async fn create(db: &Db, event: &UpdateEvent) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO events
               (title, description, start_date, target_revenue)
               VALUES (?, ?, ?, ?)"#,
            event.title,
            event.description,
            event.start_date,
            event.target_revenue
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    // Update an event.
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE events
               SET title = ?, description = ?, start_date = ?
               WHERE id = ?"#,
            event.title,
            event.description,
            event.start_date,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    // Delete an event.
    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!("DELETE FROM events WHERE id = ?", id).execute(db).await?;
        Ok(())
    }

    // Lookup an event by id, if one exists.
    pub async fn lookup_by_slug(db: &Db, slug: &str) -> AppResult<Option<Event>> {
        let event = sqlx::query_as!(
            Self,
            r#"SELECT e.*
              FROM events e
              WHERE url = ?"#,
            slug,
        )
        .fetch_optional(db)
        .await?;
        Ok(event)
    }

    pub async fn lookup_reservation_type_by_name(
        db: &Db,
        event_id: i64,
        name: &str,
    ) -> AppResult<Option<ReservationType>> {
        let event = sqlx::query_as!(
            ReservationType,
            r#"SELECT *
              FROM reservation_types
              WHERE name = ? AND event_id = ?"#,
            name,
            event_id
        )
        .fetch_optional(db)
        .await?;
        Ok(event)
    }
}
