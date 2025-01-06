use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    // TODO: Add a pretty url field, for `https://site/e/{url}`.
    // pub url: String,
    pub title: String,
    pub artist: String,
    pub description: String,
    pub start_date: DateTime<Utc>,
    // TODO: Add an end. Maybe rename to just `start` and `end`.
    // pub end_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(serde::Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub artist: String,
    pub description: String,
    pub start_date: DateTime<Utc>,
}

impl Event {
    /// Create the `events` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                title TEXT NOT NULL, \
                artist TEXT NOT NULL, \
                description TEXT NOT NULL, \
                start_date TIMESTAMP NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    // List all events.
    pub async fn list(db: &Db) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>("SELECT * FROM events").fetch_all(db).await?;
        Ok(events)
    }

    // Create a new event.
    pub async fn create(db: &Db, event: &UpdateEvent) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO events \
                (title, artist, description, start_date) \
                VALUES (?, ?, ?, ?)",
        )
        .bind(&event.title)
        .bind(&event.artist)
        .bind(&event.description)
        .bind(event.start_date)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    // Update an event.
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent) -> Result<()> {
        sqlx::query(
            "UPDATE events \
                SET title = ?, artist = ?, description = ?, start_date = ? \
                WHERE id = ?",
        )
        .bind(&event.title)
        .bind(&event.artist)
        .bind(&event.description)
        .bind(event.start_date)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    // Delete an event.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM events WHERE id = ?").bind(id).execute(db).await?;
        Ok(())
    }

    // Lookup an event by id, if one exists.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<Event>> {
        let event = sqlx::query_as::<_, Event>(
            "SELECT e.* \
            FROM events e \
            WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;
        Ok(event)
    }
}
