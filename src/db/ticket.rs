use sqlx::QueryBuilder;

use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Ticket {
    pub id: i64,

    pub name: String,
    pub description: String,
    pub qty_total: i64,
    pub qty_per_person: i64,
    pub kind: String,
    pub sort: i64,

    // kind = 'fixed'
    pub price: Option<i64>,
    // kind = 'variable'
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub price_default: Option<i64>,
    // kind = 'work'
    pub notice_hours: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateTicket {
    pub id: Option<i64>,

    pub name: String,
    pub description: String,
    pub qty_total: i64,
    pub qty_per_person: i64,
    pub kind: String,
    pub sort: i64,

    // kind = 'fixed'
    pub price: Option<i64>,
    // kind = 'variable'
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub price_default: Option<i64>,
    // kind = 'work'
    pub notice_hours: Option<i64>,
}

impl Ticket {
    /// A free ticket.
    pub const FREE: &'static str = "free";
    /// A fixed-price ticket.
    pub const FIXED: &'static str = "fixed";
    /// A variable-price ticket.
    pub const VARIABLE: &'static str = "variable";
    /// A work trade ticket.
    pub const WORK: &'static str = "work";

    /// List all tickets.
    pub async fn list(db: &Db) -> AppResult<Vec<Ticket>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM tickets").fetch_all(db).await?)
    }

    pub async fn list_ids_for_event(db: &Db, event_id: i64) -> AppResult<Vec<i64>> {
        Ok(sqlx::query!("SELECT ticket_id FROM event_tickets WHERE event_id = ?", event_id)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|row| row.ticket_id)
            .collect())
    }

    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<Ticket>> {
        Ok(sqlx::query_as!(
            Ticket,
            r#"SELECT t.*
               FROM tickets t
               JOIN event_tickets et ON et.ticket_id
               WHERE et.event_id = ?
               ORDER BY t.sort
            "#,
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    /// Create a new ticket.
    pub async fn create(db: &Db, ticket: &UpdateTicket) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO tickets
               (name, description, qty_total, qty_per_person, kind, sort, price, price_min, price_max, price_default, notice_hours)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            ticket.name,
            ticket.description,
            ticket.qty_total,
            ticket.qty_per_person,
            ticket.kind,
            ticket.sort,
            ticket.price,
            ticket.price_min,
            ticket.price_max,
            ticket.price_default,
            ticket.notice_hours,
        )
        .execute(db)
        .await?;

        Ok(row.last_insert_rowid())
    }

    /// Update an existing ticket.
    pub async fn update(db: &Db, id: i64, ticket: &UpdateTicket) -> AppResult<()> {
        sqlx::query!(
            "UPDATE tickets
               SET name = ?,
                   description = ?,
                   qty_total = ?,
                   qty_per_person = ?,
                   kind = ?,
                   sort = ?,
                   price = ?,
                   price_min = ?,
                   price_max = ?,
                   price_default = ?,
                   notice_hours = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?",
            ticket.name,
            ticket.description,
            ticket.qty_total,
            ticket.qty_per_person,
            ticket.kind,
            ticket.sort,
            ticket.price,
            ticket.price_min,
            ticket.price_max,
            ticket.price_default,
            ticket.notice_hours,
            id
        )
        .execute(db)
        .await?;

        Ok(())
    }

    pub async fn add_to_event(db: &Db, event_id: i64, ticket_ids: Vec<i64>) -> AppResult<()> {
        if ticket_ids.is_empty() {
            return Ok(());
        }

        QueryBuilder::new("INSERT INTO event_tickets (event_id, ticket_id) ")
            .push_values(ticket_ids, |mut b, ticket_id| {
                b.push_bind(event_id).push_bind(ticket_id);
            })
            .build()
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn remove_from_event(db: &Db, event_id: i64, ticket_ids: Vec<i64>) -> AppResult<()> {
        if ticket_ids.is_empty() {
            return Ok(());
        }

        // Remove the event_tickets associations
        QueryBuilder::new("DELETE FROM event_tickets WHERE event_id = ")
            .push_bind(event_id)
            .push("AND ticket_id IN ")
            .push_tuples(&ticket_ids, |mut b, ticket_id| {
                b.push_bind(ticket_id);
            })
            .build()
            .execute(db)
            .await?;

        // Remove any now unused tickets
        QueryBuilder::new(
            r#"DELETE FROM tickets
               WHERE NOT EXISTS (
                   SELECT 1 FROM rsvps r
                   WHERE r.ticket_id = tickets.id
                   AND r.event_id = "#,
        )
        .push_bind(event_id)
        .push(") AND id IN ")
        .push_tuples(&ticket_ids, |mut b, ticket_id| {
            b.push_bind(ticket_id);
        })
        .build()
        .execute(db)
        .await?;

        Ok(())
    }
}
