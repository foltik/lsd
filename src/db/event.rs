use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    // TODO: Add a pretty url field, for `https://site/e/{url}`.
    // pub url: String,
    pub title: String,
    pub artist: String,
    pub description: String,
    pub start_date: NaiveDateTime,
    // TODO: Add an end. Maybe rename to just `start` and `end`.
    // pub end_date: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub artist: String,
    pub description: String,
    pub start_date: NaiveDateTime,
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
               (title, artist, description, start_date)
               VALUES (?, ?, ?, ?)"#,
            event.title,
            event.artist,
            event.description,
            event.start_date
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    // Update an event.
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE events
               SET title = ?, artist = ?, description = ?, start_date = ?
               WHERE id = ?"#,
            event.title,
            event.artist,
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
    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Event>> {
        let event = sqlx::query_as!(
            Self,
            r#"SELECT e.*
              FROM events e
              WHERE id = ?"#,
            id,
        )
        .fetch_optional(db)
        .await?;
        Ok(event)
    }
}
