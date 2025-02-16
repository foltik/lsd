use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Guest {
    pub id: i64,
    pub event_id: i64,
    pub inviter_id: i64,
    pub guest_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateGuest {
    pub event_id: i64,
    pub inviter_id: i64,
    pub guest_id: i64,
}

impl Guest {
    /// Create the `guests` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS guests ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                event_id INTEGER NOT NULL, \
                inviter_id INTEGER NOT NULL, \
                guest_id INTEGER NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                FOREIGN KEY (event_id) REFERENCES events(id), \
                FOREIGN KEY (inviter_id) REFERENCES users(id), \
                FOREIGN KEY (guest_id) REFERENCES users(id), \
                UNIQUE (event_id, guest_id) \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new guest entry.
    pub async fn create(db: &Db, guest: &CreateGuest) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO guests (event_id, inviter_id, guest_id) \
             VALUES (?, ?, ?)",
        )
        .bind(guest.event_id)
        .bind(guest.inviter_id)
        .bind(guest.guest_id)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a guest entry by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Guest>> {
        let guest = sqlx::query_as::<_, Guest>("SELECT * FROM guests WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(guest)
    }

    /// Check if a user is a guest for an event.
    pub async fn is_guest(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        let guest = sqlx::query_as::<_, Guest>(
            "SELECT * FROM guests WHERE event_id = ? AND guest_id = ?",
        )
        .bind(event_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(guest.is_some())
    }

    /// Get all guests for an event.
    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<Guest>> {
        let guests = sqlx::query_as::<_, Guest>(
            "SELECT * FROM guests WHERE event_id = ? ORDER BY created_at",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;
        Ok(guests)
    }

    /// Get all guests invited by a user for an event.
    pub async fn list_by_inviter(db: &Db, event_id: i64, inviter_id: i64) -> Result<Vec<Guest>> {
        let guests = sqlx::query_as::<_, Guest>(
            "SELECT * FROM guests \
             WHERE event_id = ? AND inviter_id = ? \
             ORDER BY created_at",
        )
        .bind(event_id)
        .bind(inviter_id)
        .fetch_all(db)
        .await?;
        Ok(guests)
    }

    /// Remove a guest from an event.
    pub async fn remove(db: &Db, event_id: i64, guest_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM guests WHERE event_id = ? AND guest_id = ?")
            .bind(event_id)
            .bind(guest_id)
            .execute(db)
            .await?;
        Ok(())
    }
} 