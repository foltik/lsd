use image::DynamicImage;

use crate::db::event_flyer::EventFlyer;
use crate::db::rsvp::Rsvp;
use crate::db::spot::Spot;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub description: String,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,
    pub guest_list_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateEvent {
    pub title: String,
    pub slug: String,
    pub description: String,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,
    pub guest_list_id: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct EventStats {
    /// Number of total spots available.
    pub remaining_capacity: i64,
    /// Number of spots of each type available (by id)
    pub remaining_spots: HashMap<i64, i64>,

    /// Statistics about spot reservations (by id)
    pub spot_stats: HashMap<i64, Vec<SpotStat>>,
}

#[derive(Debug, serde::Serialize)]
pub struct SpotStat {
    pub name: String,
    pub value: i64,
}

impl Event {
    pub async fn list(db: &Db) -> AppResult<Vec<Event>> {
        let events = sqlx::query_as!(Self, "SELECT * FROM events").fetch_all(db).await?;
        Ok(events)
    }

    pub async fn list_upcoming(db: &Db) -> AppResult<Vec<Event>> {
        let events = sqlx::query_as!(
            Self,
            r#"SELECT * FROM events
            WHERE start > DATETIME(CURRENT_TIMESTAMP, '-24 hours')
              AND unlisted = FALSE
            ORDER BY start ASC"#
        )
        .fetch_all(db)
        .await?;
        Ok(events)
    }

    pub async fn list_past(db: &Db) -> AppResult<Vec<Event>> {
        let events = sqlx::query_as!(
            Self,
            r#"SELECT * FROM events
               WHERE start <= DATETIME(CURRENT_TIMESTAMP, '-24 hours')
                 AND unlisted = FALSE
               ORDER BY start DESC"#
        )
        .fetch_all(db)
        .await?;
        Ok(events)
    }

    // Create a new event.
    pub async fn create(db: &Db, event: &UpdateEvent, flyer: &Option<DynamicImage>) -> AppResult<i64> {
        let event_id = sqlx::query!(
            r#"INSERT INTO events
               (title, slug, description, start, end, capacity, unlisted, guest_list_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            event.title,
            event.slug,
            event.description,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.guest_list_id,
        )
        .execute(db)
        .await?
        .last_insert_rowid();

        if let Some(image) = flyer {
            EventFlyer::create_or_update(db, event_id, image).await?;
        }

        Ok(event_id)
    }

    // Update an event.
    pub async fn update(
        db: &Db,
        id: i64,
        event: &UpdateEvent,
        flyer: &Option<DynamicImage>,
    ) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE events
                SET title = ?,
                    slug = ?,
                    description = ?,
                    start = ?,
                    end = ?,
                    capacity = ?,
                    unlisted = ?,
                    guest_list_id = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.description,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.guest_list_id,
            id
        )
        .execute(db)
        .await?;

        if let Some(image) = flyer {
            EventFlyer::create_or_update(db, id, image).await?;
        }

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
    pub async fn stats_for_session(&self, db: &Db, session_id: i64) -> AppResult<EventStats> {
        let spots = Spot::list_for_event(db, self.id).await?;
        let rsvps = Rsvp::list_for_event_excluding_session(db, self.id, session_id).await?;

        let mut contributions: HashMap<i64, Vec<i64>> = HashMap::default();
        let mut qty_reserved: HashMap<i64, i64> = HashMap::default();
        for rsvp in rsvps {
            contributions.entry(rsvp.spot_id).or_default().push(rsvp.contribution);
            *qty_reserved.entry(rsvp.spot_id).or_insert(0) += 1;
        }

        let remaining_capacity = self.capacity.saturating_sub(qty_reserved.values().sum::<i64>());
        let remaining_spots = spots
            .iter()
            .map(|t| {
                (
                    t.id,
                    t.qty_total
                        .saturating_sub(*qty_reserved.get(&t.id).unwrap_or(&0))
                        .min(t.qty_per_person),
                )
            })
            .collect();

        let mut spot_stats: HashMap<i64, Vec<SpotStat>> = HashMap::default();
        for spot in &spots {
            let n = qty_reserved.get(&spot.id).copied().unwrap_or(0) as usize;
            if spot.kind != Spot::VARIABLE {
                continue;
            }

            // If the spot hasn't been reserved, just use the suggested contribution as the median
            // so we always have at least one stat to make frontend styling easier.
            if n == 0 {
                spot_stats.insert(
                    spot.id,
                    vec![SpotStat { name: "Median".into(), value: spot.suggested_contribution.unwrap() }],
                );
                continue;
            }

            let prices = contributions.get_mut(&spot.id).unwrap(); // we checked n > 0
            prices.sort_unstable();

            let median = if n.is_multiple_of(2) {
                let l = prices[n / 2 - 1];
                let r = prices[n / 2];
                (l + r) / 2
            } else {
                prices[n / 2]
            };
            let max = prices.last().copied().unwrap();

            // Only add the max if it's different from the median to avoid clutter
            let mut stats = vec![];
            stats.push(SpotStat { name: "Median".into(), value: median });
            if max > median {
                stats.push(SpotStat { name: "Max".into(), value: max });
            }

            spot_stats.insert(spot.id, stats);
        }

        Ok(EventStats { remaining_capacity, remaining_spots, spot_stats })
    }

    #[allow(unused)]
    pub fn is_upcoming(&self, now: NaiveDateTime) -> bool {
        // TODO: use with:
        // let now = Utc::now().naive_utc();
        // let past = query.past.unwrap_or(false);

        now <= self.start || self.end.is_some_and(|end| now <= end)
    }
}
