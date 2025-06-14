use crate::db::rsvp::Rsvp;
use crate::db::ticket::Ticket;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub description: String,
    pub flyer: Option<String>,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub slug: String,
    pub description: String,
    pub flyer: Option<String>,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct EventStats {
    /// Number of total tickets available.
    pub remaining_capacity: i64,
    /// Number of tickets of each type available (by id)
    pub remaining_tickets: HashMap<i64, i64>,

    /// Statistics about ticket sales (by id)
    pub ticket_stats: HashMap<i64, Vec<TicketStat>>,
}

#[derive(Debug, serde::Serialize)]
pub struct TicketStat {
    name: String,
    value: i64,
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
               (title, slug, flyer, description, start, end, capacity, unlisted)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            event.title,
            event.slug,
            event.flyer,
            event.description,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
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
                    flyer = ?,
                    description = ?,
                    start = ?,
                    end = ?,
                    capacity = ?,
                    unlisted = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.flyer,
            event.description,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
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

    /// Lookup a post by URL, if one exists.
    pub async fn lookup_by_slug(db: &Db, slug: &str) -> AppResult<Option<Event>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM events WHERE slug = ?", slug)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }

    /// Calculate user-facing stats for an event.
    pub async fn stats(&self, db: &Db) -> AppResult<EventStats> {
        let tickets = Ticket::list_for_event(db, self.id).await?;
        let rsvps = Rsvp::list_for_event(db, self.id).await?;

        let mut prices: HashMap<i64, Vec<i64>> = HashMap::default();
        let mut qty_sold: HashMap<i64, i64> = HashMap::default();
        for rsvp in rsvps {
            if let Some(price) = rsvp.price {
                prices.entry(rsvp.ticket_id).or_default().push(price);
                *qty_sold.entry(rsvp.ticket_id).or_insert(0) += 1;
            }
        }

        let remaining_capacity = self.capacity.saturating_sub(qty_sold.values().sum::<i64>());
        let remaining_tickets = tickets
            .iter()
            .map(|t| (t.id, t.qty_total.saturating_sub(*qty_sold.get(&t.id).unwrap_or(&0))))
            .collect();

        let mut ticket_stats: HashMap<i64, Vec<TicketStat>> = HashMap::default();
        for ticket in &tickets {
            let n = qty_sold.get(&ticket.id).copied().unwrap_or(0) as usize;
            if ticket.kind != Ticket::VARIABLE || n == 0 {
                continue;
            }

            let prices = prices.get_mut(&ticket.id).unwrap(); // we checked n > 0
            prices.sort_unstable();

            let median = if n % 2 == 0 { (prices[n / 2 - 1] + prices[n / 2]) / 2 } else { prices[n / 2] };
            let max = prices.last().copied().unwrap();
            ticket_stats.insert(
                ticket.id,
                vec![
                    TicketStat { name: "median".into(), value: median },
                    TicketStat { name: "max".into(), value: max },
                ],
            );
        }

        Ok(EventStats { remaining_capacity, remaining_tickets, ticket_stats })
    }

    #[allow(unused)]
    pub fn is_upcoming(&self, now: NaiveDateTime) -> bool {
        // TODO: use with:
        // let now = Utc::now().naive_utc();
        // let past = query.past.unwrap_or(false);

        now <= self.start || self.end.is_some_and(|end| now <= end)
    }
}
