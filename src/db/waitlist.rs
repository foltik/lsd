use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Waitlist {
    pub id: i64,
    pub event_id: i64,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateWaitlist {
    pub event_id: i64,
    pub user_id: i64,
}

impl Waitlist {
    /// Create the `waitlist` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS waitlist ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                event_id INTEGER NOT NULL, \
                user_id INTEGER NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                notified_at TIMESTAMP, \
                FOREIGN KEY (event_id) REFERENCES events(id), \
                FOREIGN KEY (user_id) REFERENCES users(id), \
                UNIQUE (event_id, user_id) \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new waitlist entry.
    pub async fn create(db: &Db, waitlist: &CreateWaitlist) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO waitlist (event_id, user_id) \
             VALUES (?, ?)",
        )
        .bind(waitlist.event_id)
        .bind(waitlist.user_id)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a waitlist entry by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Waitlist>> {
        let waitlist = sqlx::query_as::<_, Waitlist>("SELECT * FROM waitlist WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(waitlist)
    }

    /// Check if a user is on the waitlist for an event.
    pub async fn is_waitlisted(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        let waitlist = sqlx::query_as::<_, Waitlist>(
            "SELECT * FROM waitlist WHERE event_id = ? AND user_id = ?",
        )
        .bind(event_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(waitlist.is_some())
    }

    /// Get all waitlist entries for an event.
    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<Waitlist>> {
        let waitlist = sqlx::query_as::<_, Waitlist>(
            "SELECT * FROM waitlist \
             WHERE event_id = ? \
             ORDER BY created_at",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;
        Ok(waitlist)
    }

    /// Get all unnotified waitlist entries for an event.
    pub async fn list_unnotified_for_event(db: &Db, event_id: i64) -> Result<Vec<Waitlist>> {
        let waitlist = sqlx::query_as::<_, Waitlist>(
            "SELECT * FROM waitlist \
             WHERE event_id = ? AND notified_at IS NULL \
             ORDER BY created_at",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;
        Ok(waitlist)
    }

    /// Mark a waitlist entry as notified.
    pub async fn mark_notified(&self, db: &Db) -> Result<()> {
        sqlx::query(
            "UPDATE waitlist SET \
                notified_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Remove a user from the waitlist.
    pub async fn remove(db: &Db, event_id: i64, user_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM waitlist WHERE event_id = ? AND user_id = ?")
            .bind(event_id)
            .bind(user_id)
            .execute(db)
            .await?;
        Ok(())
    }
} 