use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub flyer_url: Option<String>,
    pub desc: String,
    pub desc_rendered: String,
    pub brief: String,
    pub brief_rendered: String,
    pub brief_sent: bool,
    pub date: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub capacity: i64,
    pub cancellation_grace_period: i64,
    pub guest_list_id: Option<i64>,
    pub num_guests: Option<i64>,
    pub num_guests_vip: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub url: String,
    pub flyer_url: Option<String>,
    pub desc: String,
    pub desc_rendered: String,
    pub brief: String,
    pub brief_rendered: String,
    pub date: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub capacity: i64,
    pub cancellation_grace_period: i64,
}

impl Event {
    /// Create the `events` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                title TEXT NOT NULL, \
                url TEXT NOT NULL, \
                flyer_url TEXT, \
                desc TEXT NOT NULL, \
                desc_rendered TEXT NOT NULL, \
                brief TEXT NOT NULL, \
                brief_rendered TEXT NOT NULL, \
                brief_sent BOOLEAN NOT NULL DEFAULT FALSE, \
                date TEXT NOT NULL, \
                start_time TEXT NOT NULL, \
                end_time TEXT, \
                capacity INTEGER NOT NULL, \
                cancellation_grace_period INTEGER NOT NULL, \
                guest_list_id INTEGER, \
                num_guests INTEGER, \
                num_guests_vip INTEGER, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new event.
    pub async fn create(db: &Db, event: &UpdateEvent) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO events ( \
                title, url, flyer_url, desc, desc_rendered, brief, brief_rendered, \
                date, start_time, end_time, capacity, cancellation_grace_period \
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event.title)
        .bind(&event.url)
        .bind(&event.flyer_url)
        .bind(&event.desc)
        .bind(&event.desc_rendered)
        .bind(&event.brief)
        .bind(&event.brief_rendered)
        .bind(&event.date)
        .bind(&event.start_time)
        .bind(&event.end_time)
        .bind(event.capacity)
        .bind(event.cancellation_grace_period)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup an event by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Event>> {
        let event = sqlx::query_as::<_, Event>("SELECT * FROM events WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(event)
    }

    /// Lookup an event by URL.
    pub async fn lookup_by_url(db: &Db, url: &str) -> Result<Option<Event>> {
        let event = sqlx::query_as::<_, Event>("SELECT * FROM events WHERE url = ?")
            .bind(url)
            .fetch_optional(db)
            .await?;
        Ok(event)
    }

    /// Update the guest list counts for an event.
    pub async fn update_guest_counts(
        &self,
        db: &Db,
        guest_list_id: i64,
        num_guests: i64,
        num_guests_vip: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE events SET \
                guest_list_id = ?, \
                num_guests = ?, \
                num_guests_vip = ?, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(guest_list_id)
        .bind(num_guests)
        .bind(num_guests_vip)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Mark the brief as sent.
    pub async fn mark_brief_sent(&self, db: &Db) -> Result<()> {
        sqlx::query(
            "UPDATE events SET \
                brief_sent = TRUE, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// List all events.
    pub async fn list(db: &Db) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>("SELECT * FROM events ORDER BY date DESC")
            .fetch_all(db)
            .await?;
        Ok(events)
    }

    /// Lookup an event by id.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<Event>> {
        Self::lookup(db, id).await
    }

    /// Update an event.
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent) -> Result<()> {
        sqlx::query(
            "UPDATE events SET \
                title = ?, \
                url = ?, \
                flyer_url = ?, \
                desc = ?, \
                desc_rendered = ?, \
                brief = ?, \
                brief_rendered = ?, \
                date = ?, \
                start_time = ?, \
                end_time = ?, \
                capacity = ?, \
                cancellation_grace_period = ?, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(&event.title)
        .bind(&event.url)
        .bind(&event.flyer_url)
        .bind(&event.desc)
        .bind(&event.desc_rendered)
        .bind(&event.brief)
        .bind(&event.brief_rendered)
        .bind(&event.date)
        .bind(&event.start_time)
        .bind(&event.end_time)
        .bind(event.capacity)
        .bind(event.cancellation_grace_period)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Delete an event.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM events WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
        Ok(())
    }
}
