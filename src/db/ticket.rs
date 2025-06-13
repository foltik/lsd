#![allow(unused)]

use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Ticket {
    pub id: i64,

    pub name: String,
    pub description: String,
    pub quantity: i64,
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
    pub name: String,
    pub description: String,
    pub quantity: i64,
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

#[derive(serde::Serialize)]
pub struct TicketStat {
    name: String,
    value: i64,
}
#[derive(serde::Serialize)]
pub struct TicketWithStats {
    #[serde(flatten)]
    ticket: Ticket,
    remaining: i64,
    stats: Vec<TicketStat>,
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

    pub async fn list_for_event(db: &Db, event_id: i64) -> AppResult<Vec<Ticket>> {
        Ok(sqlx::query_as!(
            Ticket,
            r#"SELECT t.*
               FROM tickets t
               JOIN event_tickets et ON et.ticket_id
               WHERE et.event_id = ?
            "#,
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_for_event_with_stats(db: &Db, event_id: i64) -> AppResult<Vec<TicketWithStats>> {
        // List all tickets for this event.
        let tickets = sqlx::query_as!(
            Ticket,
            r#"SELECT t.*
               FROM tickets t
               JOIN event_tickets et ON et.ticket_id
               WHERE et.event_id = ?
            "#,
            event_id
        )
        .fetch_all(db)
        .await?;

        // List all RSVPs for this ticket.
        #[derive(sqlx::FromRow)]
        struct Rsvp {
            ticket_id: i64,
            price: Option<i64>,
        }
        let rsvps = sqlx::query_as!(Rsvp, "SELECT ticket_id, price FROM rsvps WHERE event_id = ?", event_id)
            .fetch_all(db)
            .await?;

        // Calculate stats for each ticket
        let mut prices: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut counts: HashMap<i64, i64> = HashMap::new();
        for rsvp in rsvps {
            if let Some(price) = rsvp.price {
                prices.entry(rsvp.ticket_id).or_default().push(price);
                *counts.entry(rsvp.ticket_id).or_insert(0) += 1;
            }
        }

        // Build result
        let mut with_stats = vec![];
        for ticket in tickets {
            let count = *counts.get(&ticket.id).unwrap_or(&0);
            let remaining = ticket.quantity - count;

            let stats = match ticket.kind.as_str() {
                Ticket::VARIABLE if count > 0 => {
                    let prices = prices.get_mut(&ticket.id).unwrap(); // we checked count > 0
                    prices.sort_unstable();

                    let n = prices.len();
                    let median =
                        if n % 2 == 0 { (prices[n / 2 - 1] + prices[n / 2]) / 2 } else { prices[n / 2] };
                    let max = prices.last().copied().unwrap();

                    vec![
                        TicketStat { name: "median".into(), value: median },
                        TicketStat { name: "max".into(), value: max },
                    ]
                }
                _ => vec![],
            };

            with_stats.push(TicketWithStats { ticket, remaining, stats })
        }
        Ok(with_stats)
    }

    /// Create a new ticket.
    pub async fn create(db: &Db, ticket: &UpdateTicket) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO tickets
               (name, description, quantity, kind, sort, price, price_min, price_max, price_default)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            ticket.name,
            ticket.description,
            ticket.quantity,
            ticket.kind,
            ticket.sort,
            ticket.price,
            ticket.price_min,
            ticket.price_max,
            ticket.price_default,
        )
        .execute(db)
        .await?;

        Ok(row.last_insert_rowid())
    }

    /// Update an existing ticket.
    pub async fn update(db: &Db, id: i64, ticket: &UpdateTicket) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE tickets
               SET name = ?,
                   description = ?,
                   quantity = ?,
                   kind = ?,
                   sort = ?,
                   price = ?,
                   price_min = ?,
                   price_max = ?,
                   price_default = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            ticket.name,
            ticket.description,
            ticket.quantity,
            ticket.kind,
            ticket.sort,
            ticket.price,
            ticket.price_min,
            ticket.price_max,
            ticket.price_default,
            id
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Remove a ticket.
    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM tickets WHERE id = ?"#, id).execute(db).await?;
        Ok(())
    }

    /// Retrieve a ticket (if any) by its id.
    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Ticket>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM tickets WHERE id = ?", id)
            .fetch_optional(db)
            .await?)
    }
}
