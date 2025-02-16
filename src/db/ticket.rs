use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Ticket {
    pub id: i64,
    pub name: String,
    pub desc: String,
    pub desc_rendered: String,
    pub price: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateTicket {
    pub name: String,
    pub desc: String,
    pub desc_rendered: String,
    pub price: i64,
}

impl Ticket {
    /// Create the `tickets` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tickets ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                name TEXT NOT NULL, \
                desc TEXT NOT NULL, \
                desc_rendered TEXT NOT NULL, \
                price INTEGER NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;

        // Create the event_tickets join table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS event_tickets ( \
                event_id INTEGER NOT NULL, \
                ticket_id INTEGER NOT NULL, \
                index INTEGER NOT NULL, \
                PRIMARY KEY (event_id, ticket_id), \
                FOREIGN KEY (event_id) REFERENCES events(id), \
                FOREIGN KEY (ticket_id) REFERENCES tickets(id) \
            )",
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Create a new ticket.
    pub async fn create(db: &Db, ticket: &CreateTicket) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO tickets (name, desc, desc_rendered, price) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&ticket.name)
        .bind(&ticket.desc)
        .bind(&ticket.desc_rendered)
        .bind(ticket.price)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a ticket by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Ticket>> {
        let ticket = sqlx::query_as::<_, Ticket>("SELECT * FROM tickets WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(ticket)
    }

    /// Add a ticket to an event.
    pub async fn add_to_event(db: &Db, event_id: i64, ticket_id: i64, index: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO event_tickets (event_id, ticket_id, index) \
             VALUES (?, ?, ?)",
        )
        .bind(event_id)
        .bind(ticket_id)
        .bind(index)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Get all tickets for an event.
    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<Ticket>> {
        let tickets = sqlx::query_as::<_, Ticket>(
            "SELECT t.* FROM tickets t \
             JOIN event_tickets et ON et.ticket_id = t.id \
             WHERE et.event_id = ? \
             ORDER BY et.index",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;
        Ok(tickets)
    }
} 