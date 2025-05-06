use super::Db;
use crate::utils::error::AppResult;
use serde::Deserialize;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct EventTicket {
    pub event_id: i64,
    pub ticket_id: i64,
    pub price: i64,
    pub quantity: i64,
    pub sort: i64,
}

#[derive(Deserialize)]
pub struct UpdateEventTicket {
    pub price: i64,
    pub quantity: i64,
    pub sort: i64,
}

impl EventTicket {
    /// List all event tickets for a given event.
    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<EventTicket>> {
        let tickets = sqlx::query_as!(Self, "SELECT * FROM event_tickets WHERE event_id = ?", event_id)
            .fetch_all(db)
            .await?;
        Ok(tickets)
    }

    /// Create a new event ticket association.
    pub async fn create(
        db: &Db,
        event_id: i64,
        ticket_id: i64,
        price: i64,
        quantity: i64,
        sort: i64,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"INSERT INTO event_tickets (event_id, ticket_id, price, quantity, sort)
               VALUES (?, ?, ?, ?, ?)"#,
            event_id,
            ticket_id,
            price,
            quantity,
            sort,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Update an existing event ticket association.
    pub async fn update(db: &Db, event_id: i64, ticket_id: i64, update: &UpdateEventTicket) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE event_tickets
               SET price = ?, quantity = ?, sort = ?
               WHERE event_id = ? AND ticket_id = ?"#,
            update.price,
            update.quantity,
            update.sort,
            event_id,
            ticket_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Delete an event ticket association.
    pub async fn delete(db: &Db, event_id: i64, ticket_id: i64) -> AppResult<()> {
        sqlx::query!(
            r#"DELETE FROM event_tickets WHERE event_id = ? AND ticket_id = ?"#,
            event_id,
            ticket_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Lookup an event ticket by its composite primary key (event_id, ticket_id).
    pub async fn lookup_by_ids(db: &Db, event_id: i64, ticket_id: i64) -> AppResult<Option<EventTicket>> {
        let ticket = sqlx::query_as!(
            Self,
            r#"SELECT * FROM event_tickets WHERE event_id = ? AND ticket_id = ?"#,
            event_id,
            ticket_id
        )
        .fetch_optional(db)
        .await?;
        Ok(ticket)
    }
}
