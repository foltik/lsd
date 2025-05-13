use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub guest_list_id: Option<i64>,

    pub title: String,
    pub slug: String,
    pub description: String,
    pub flyer: Option<String>,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize, Debug)]
pub struct UpdateEvent {
    pub guest_list_id: Option<i64>,

    pub title: String,
    pub slug: String,
    pub description: String,
    pub flyer: Option<String>,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
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
               (title, slug, description, flyer, start, end, guest_list_id)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            event.title,
            event.slug,
            event.description,
            event.flyer,
            event.start,
            event.end,
            event.guest_list_id,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    // Update an event.
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE events
                SET title = ?,
                    slug = ?,
                    description = ?,
                    flyer = ?,
                    start = ?,
                    end = ?,
                    guest_list_id = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.description,
            event.flyer,
            event.start,
            event.end,
            event.guest_list_id,
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
        let event = sqlx::query_as!(Self, r#"SELECT * FROM events WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?;
        Ok(event)
    }
}
