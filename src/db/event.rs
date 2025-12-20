use image::DynamicImage;

use crate::db::event_flyer::EventFlyer;
use crate::db::rsvp::EventRsvp;
use crate::db::spot::Spot;
use crate::prelude::*;

#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub capacity: i64,
    pub unlisted: bool,
    pub closed: bool,
    pub guest_list_id: Option<i64>,
    pub spots_per_person: Option<i64>,

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
    pub title: String,
    pub slug: String,

    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,

    pub capacity: i64,
    pub unlisted: bool,
    pub closed: bool,
    pub guest_list_id: Option<i64>,
    pub spots_per_person: Option<i64>,
}

impl Event {
    pub async fn list(db: &Db) -> Result<Vec<Event>> {
        let events = sqlx::query_as!(Self, "SELECT * FROM events").fetch_all(db).await?;
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
        let event_id = sqlx::query!(
            r#"INSERT INTO events
               (title, slug, start, end, capacity, unlisted, closed, guest_list_id, spots_per_person)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            event.title,
            event.slug,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.closed,
            event.guest_list_id,
            event.spots_per_person,
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
                    start = ?,
                    end = ?,
                    capacity = ?,
                    unlisted = ?,
                    closed = ?,
                    guest_list_id = ?,
                    spots_per_person = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.start,
            event.end,
            event.capacity,
            event.unlisted,
            event.closed,
            event.guest_list_id,
            event.spots_per_person,
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

    // Delete an event.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
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
}

pub struct EventLimits {
    pub total_limit: i64,
    pub spot_limits: HashMap<i64, i64>,
}

impl Event {
    /// Calculate number of spots available of each type for this event.
    pub fn compute_limits(&self, user: &Option<User>, spots: &[Spot], rsvps: &[EventRsvp]) -> EventLimits {
        // Overall event limits
        let capacity_limit = self.capacity - rsvps.len() as i64;
        let per_person_limit = self.spots_per_person.unwrap_or(i64::MAX);
        let this_user_limit = user.as_ref().map(|_| i64::MAX).unwrap_or(i64::MAX); // TODO
        let limit = capacity_limit.min(per_person_limit).min(this_user_limit);

        // Count rsvps per spot
        let mut spot_num_rsvps: HashMap<i64, i64> = Default::default();
        for rsvp in rsvps {
            *spot_num_rsvps.entry(rsvp.spot_id).or_default() += 1;
        }

        // Per-spot limits
        let mut sum_spot_limits = 0;
        let mut spot_limits = HashMap::default();
        for spot in spots {
            let spot_total_limit = spot.qty_total - spot_num_rsvps.get(&spot.id).unwrap_or(&0);
            let spot_per_person_limit = spot.qty_per_person;
            let spot_limit = spot_total_limit.min(spot_per_person_limit);

            sum_spot_limits += spot_limit;
            spot_limits.insert(spot.id, spot_limit);
        }

        // Final limit is no more than the sum of all per-spot limits
        let limit = limit.min(sum_spot_limits);

        EventLimits { total_limit: limit, spot_limits }
    }
}
