use chrono::{Days, TimeZone};
use image::DynamicImage;
use rand::Rng;
use rand::rngs::OsRng;

use crate::db::event_flyer::EventFlyer;
use crate::db::rsvp::EventRsvp;
use crate::db::spot::Spot;
use crate::prelude::*;

#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub token: String,
    pub kind: String,
    pub title: String,
    pub slug: String,
    pub url: Option<String>,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub capacity: i64,
    pub unlisted: bool,
    pub closed: bool,
    pub guest_list_id: Option<i64>,
    pub spots_per_person: Option<i64>,
    pub artist_share: i64,

    pub description_html: Option<String>,
    pub description_updated_at: Option<NaiveDateTime>,

    pub invite_subject: Option<String>,
    pub invite_html: Option<String>,
    pub invite_updated_at: Option<NaiveDateTime>,
    pub invite_sent_at: Option<NaiveDateTime>,

    pub confirmation_subject: Option<String>,
    pub confirmation_html: Option<String>,
    pub confirmation_updated_at: Option<NaiveDateTime>,

    pub dayof_subject: Option<String>,
    pub dayof_html: Option<String>,
    pub dayof_updated_at: Option<NaiveDateTime>,
    pub dayof_sent_at: Option<NaiveDateTime>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateEvent {
    pub kind: String,
    pub title: String,
    pub slug: String,
    pub url: Option<String>,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,
    pub closed: bool,
    pub guest_list_id: Option<i64>,
    pub spots_per_person: Option<i64>,
    pub artist_share: i64,
}

/// Event with RSVP count for the admin list page.
#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct EventWithStats {
    pub id: i64,
    pub token: String,
    pub kind: String,
    pub title: String,
    pub slug: String,
    pub url: Option<String>,
    pub start: NaiveDateTime,
    pub guest_list_id: Option<i64>,
    pub capacity: i64,
    pub rsvp_count: i64,
    pub total_contributions: i64,
}

impl EventWithStats {
    pub fn is_external(&self) -> bool {
        self.kind == Event::EXTERNAL
    }
}

impl Event {
    pub const INTERNAL: &'static str = "internal";
    pub const EXTERNAL: &'static str = "external";

    pub fn is_external(&self) -> bool {
        self.kind == Self::EXTERNAL
    }

    /// List events with RSVP stats, newest first. When `recent`, only the last 3 months.
    pub async fn list(db: &Db, recent: bool) -> Result<Vec<EventWithStats>> {
        let events = sqlx::query_as!(
            EventWithStats,
            r#"SELECT
                 e.id, e.token, e.kind, e.title, e.slug, e.url, e.start, e.guest_list_id, e.capacity,
                 CAST(
                   COALESCE(sr.session_rsvp_count, 0) + COALESCE(mr.manual_rsvp_count, 0)
                   AS INT
                 ) AS "rsvp_count!: i64",
                 CAST(
                   COALESCE(sr.session_contributions, 0)
                   AS INT
                 ) AS "total_contributions!: i64"
               FROM events e
               LEFT JOIN (
                 SELECT
                   rs.event_id,
                   COUNT(r.id) AS session_rsvp_count,
                   SUM(r.contribution) AS session_contributions
                 FROM rsvp_sessions rs
                 JOIN rsvps r ON r.session_id = rs.id
                 WHERE rs.status IN ('payment_pending', 'payment_confirmed')
                 GROUP BY rs.event_id
               ) sr ON sr.event_id = e.id
               LEFT JOIN (
                 SELECT
                   m.event_id,
                   COUNT(*) AS manual_rsvp_count
                 FROM manual_rsvps m
                 GROUP BY m.event_id
               ) mr ON mr.event_id = e.id
               WHERE NOT ?1 OR e.start >= DATETIME(CURRENT_TIMESTAMP, '-3 months')
               ORDER BY e.start DESC"#,
            recent
        )
        .fetch_all(db)
        .await?;
        Ok(events)
    }

    pub async fn list_upcoming(db: &Db) -> Result<Vec<Event>> {
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

    pub async fn list_past(db: &Db) -> Result<Vec<Event>> {
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
    pub async fn create(db: &Db, event: &UpdateEvent, flyer: &Option<DynamicImage>) -> Result<i64> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let event_id = sqlx::query!(
            r#"INSERT INTO events
               (token, kind, title, slug, url, start, end, capacity, unlisted, closed, guest_list_id, spots_per_person, artist_share)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            token,
            event.kind,
            event.title,
            event.slug,
            event.url,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.closed,
            event.guest_list_id,
            event.spots_per_person,
            event.artist_share,
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
    pub async fn update(db: &Db, id: i64, event: &UpdateEvent, flyer: &Option<DynamicImage>) -> Result<()> {
        sqlx::query!(
            r#"UPDATE events
                SET title = ?,
                    slug = ?,
                    url = ?,
                    start = ?,
                    end = ?,
                    capacity = ?,
                    unlisted = ?,
                    closed = ?,
                    guest_list_id = ?,
                    spots_per_person = ?,
                    artist_share = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.url,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.closed,
            event.guest_list_id,
            event.spots_per_person,
            event.artist_share,
            id
        )
        .execute(db)
        .await?;

        if let Some(image) = flyer {
            EventFlyer::create_or_update(db, id, image).await?;
        }

        Ok(())
    }

    pub async fn update_invite(db: &Db, id: i64, subject: String, html: String) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE EVENTS
             SET invite_subject = ?,
                 invite_html = ?,
                 invite_updated_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?
             RETURNING invite_updated_at",
            subject,
            html,
            id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.invite_updated_at.unwrap())
    }

    pub async fn mark_sent_invites(&self, db: &Db) -> Result<()> {
        sqlx::query!("UPDATE events SET invite_sent_at = CURRENT_TIMESTAMP WHERE id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn update_confirmation(
        db: &Db, id: i64, subject: String, html: String,
    ) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE EVENTS
             SET confirmation_subject = ?,
                 confirmation_html = ?,
                 confirmation_updated_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?
             RETURNING confirmation_updated_at",
            subject,
            html,
            id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.confirmation_updated_at.unwrap())
    }

    pub async fn update_dayof(db: &Db, id: i64, subject: String, html: String) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE EVENTS
             SET dayof_subject = ?,
                 dayof_html = ?,
                 dayof_updated_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?
             RETURNING dayof_updated_at",
            subject,
            html,
            id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.dayof_updated_at.unwrap())
    }

    pub async fn update_description(db: &Db, id: i64, html: String) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE EVENTS
             SET description_html = ?,
                 description_updated_at = CURRENT_TIMESTAMP,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?
             RETURNING description_updated_at",
            html,
            id,
        )
        .fetch_one(db)
        .await?;
        Ok(row.description_updated_at.unwrap())
    }

    pub async fn mark_sent_dayof(&self, db: &Db) -> Result<()> {
        sqlx::query!("UPDATE events SET dayof_sent_at = CURRENT_TIMESTAMP WHERE id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Delete an event and all related records (cascade delete).
    /// Deletes: rsvps, rsvp_sessions, manual_rsvps, event_spots, event_flyers, then the event itself.
    /// Note: emails are NOT deleted (kept for history).
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        // Delete RSVPs for this event (via sessions)
        sqlx::query!(
            "DELETE FROM rsvps WHERE session_id IN (SELECT id FROM rsvp_sessions WHERE event_id = ?)",
            id
        )
        .execute(db)
        .await?;
        // Delete RSVP sessions for this event
        sqlx::query!("DELETE FROM rsvp_sessions WHERE event_id = ?", id)
            .execute(db)
            .await?;
        // Delete manual RSVPs for this event
        sqlx::query!("DELETE FROM manual_rsvps WHERE event_id = ?", id)
            .execute(db)
            .await?;
        // Delete event-spot associations
        sqlx::query!("DELETE FROM event_spots WHERE event_id = ?", id)
            .execute(db)
            .await?;
        // Delete event flyer
        sqlx::query!("DELETE FROM event_flyers WHERE event_id = ?", id)
            .execute(db)
            .await?;
        // Finally delete the event itself
        sqlx::query!("DELETE FROM events WHERE id = ?", id).execute(db).await?;
        Ok(())
    }

    /// Lookup a post by id.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<Event>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM events WHERE id = ?", id)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }

    /// Lookup a post by URL, if one exists.
    pub async fn lookup_by_slug(db: &Db, slug: &str) -> Result<Option<Event>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM events WHERE slug = ?", slug)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }

    #[allow(unused)]
    pub fn is_upcoming(&self, now: NaiveDateTime) -> bool {
        // TODO: use with:
        // let now = Utc::now().naive_utc();
        // let past = query.past.unwrap_or(false);

        now <= self.start || self.end.is_some_and(|end| now <= end)
    }

    /// True once the event is past its end time, or past midnight on the start day if none.
    pub fn is_over(&self) -> bool {
        let cutoff = match self.end {
            Some(end) => end.and_utc(),
            None => {
                let tz = config().app.tz;
                let start_day = self.start.and_utc().with_timezone(&tz).date_naive();
                let midnight = (start_day + Days::new(1)).and_hms_opt(0, 0, 0).unwrap();
                // New York has no midnight DST transition, so this is always unambiguous.
                tz.from_local_datetime(&midnight).single().unwrap().to_utc()
            }
        };
        Utc::now() >= cutoff
    }

    /// Whether to allow starting the RSVP process
    pub fn registration_open(&self) -> bool {
        !self.closed && !self.is_over()
    }

    /// Artist's share of `total` dollars per `artist_share` percent, fractional dollars round to the artist.
    pub fn artist_share(&self, total: i64) -> i64 {
        (total * self.artist_share + 99) / 100
    }
}

pub struct EventLimits {
    pub total_limit: i64,
    pub spot_limits: HashMap<i64, i64>,
}

impl Event {
    /// Duplicate an event, including spots and flyer.
    /// Returns the ID of the new event.
    pub async fn duplicate(db: &Db, event_id: i64) -> Result<i64> {
        let event = Event::lookup_by_id(db, event_id)
            .await?
            .ok_or_else(|| any!("Event not found"))?;

        // Generate unique slug by appending an incrementing suffix
        let mut suffix = 1;
        let new_slug = loop {
            let new_slug = format!("{}-{}", event.slug, suffix);
            if Event::lookup_by_slug(db, &new_slug).await?.is_none() {
                break new_slug;
            }
            suffix += 1;
        };

        let new_title = format!("{} (copy)", event.title);

        // Create the new event
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let new_event_id = sqlx::query!(
            r#"INSERT INTO events
               (token, kind, title, slug, url, start, end, capacity, unlisted, closed, guest_list_id, spots_per_person, artist_share,
                description_html, description_updated_at,
                invite_subject, invite_html, invite_updated_at,
                confirmation_subject, confirmation_html, confirmation_updated_at,
                dayof_subject, dayof_html, dayof_updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                       ?, ?,
                       ?, ?, ?,
                       ?, ?, ?,
                       ?, ?, ?)"#,
            token,
            event.kind,
            new_title,
            new_slug,
            event.url,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.closed,
            event.guest_list_id,
            event.spots_per_person,
            event.artist_share,
            event.description_html,
            event.description_updated_at,
            event.invite_subject,
            event.invite_html,
            event.invite_updated_at,
            event.confirmation_subject,
            event.confirmation_html,
            event.confirmation_updated_at,
            event.dayof_subject,
            event.dayof_html,
            event.dayof_updated_at,
        )
        .execute(db)
        .await?
        .last_insert_rowid();

        // Duplicate spots (create new spot records and link to new event)
        Spot::duplicate_for_event(db, event_id, new_event_id).await?;

        // Duplicate flyer if exists
        EventFlyer::duplicate(db, event_id, new_event_id).await?;

        Ok(new_event_id)
    }
}

impl Event {
    /// Calculate number of spots available of each type for this event.
    /// `all_rsvps` = all reserved RSVPs counted toward capacity (may or may not include current session).
    /// `user_rsvps` = this user's reserved RSVPs across all their sessions (for per-person limits).
    /// `manual_count` = all manually added RSVPs counted toward capacity
    pub fn compute_limits(
        &self, spots: &[Spot], all_rsvps: &[EventRsvp], user_rsvps: &[EventRsvp], manual_count: i64,
    ) -> EventLimits {
        // Overall event limits
        let capacity_limit = (self.capacity - all_rsvps.len() as i64 - manual_count).max(0);
        let per_person_limit = self.spots_per_person.unwrap_or(i64::MAX);
        let this_person_limit = per_person_limit - user_rsvps.len() as i64;
        let limit = capacity_limit.min(this_person_limit);

        // Count rsvps per spot
        let mut spot_num_rsvps: HashMap<i64, i64> = Default::default();
        for rsvp in all_rsvps {
            *spot_num_rsvps.entry(rsvp.spot_id).or_default() += 1;
        }

        // Count user's own rsvps per spot
        let mut user_spot_counts: HashMap<i64, i64> = Default::default();
        for rsvp in user_rsvps {
            *user_spot_counts.entry(rsvp.spot_id).or_default() += 1;
        }

        // Per-spot limits
        let mut sum_spot_limits = 0;
        let mut spot_limits = HashMap::default();
        for spot in spots {
            let spot_total_limit = spot.qty_total - spot_num_rsvps.get(&spot.id).unwrap_or(&0);
            let spot_per_person_limit = spot.qty_per_person;
            let spot_this_person_limit = spot_per_person_limit - user_spot_counts.get(&spot.id).unwrap_or(&0);
            let spot_limit = spot_total_limit.min(spot_this_person_limit);

            sum_spot_limits += spot_limit;
            spot_limits.insert(spot.id, spot_limit);
        }

        // Final limit is no more than the sum of all per-spot limits
        let limit = limit.min(sum_spot_limits);

        EventLimits { total_limit: limit, spot_limits }
    }
}
