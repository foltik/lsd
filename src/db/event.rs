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

    pub unlisted: bool,
    pub guest_list_id: Option<i64>,
    pub target_revenue: Option<i64>,

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

    pub unlisted: bool,
    pub guest_list_id: Option<i64>,
    pub target_revenue: Option<i64>,
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
               (title, slug, description, flyer, start, end, unlisted, guest_list_id, target_revenue)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            event.title,
            event.slug,
            event.description,
            event.flyer,
            event.start,
            event.end,
            event.unlisted,
            event.guest_list_id,
            event.target_revenue,
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
                    description = ?,
                    flyer = ?,
                    start = ?,
                    end = ?,
                    unlisted = ?,
                    guest_list_id = ?,
                    target_revenue = ?
                WHERE id = ?"#,
            event.title,
            event.slug,
            event.description,
            event.flyer,
            event.start,
            event.end,
            event.unlisted,
            event.guest_list_id,
            event.target_revenue,
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

    pub async fn is_on_list(&self, db: &Db, id: i64, user: &Option<User>) -> AppResult<bool> {
        // If there's no guest list, anyone is on the list.
        let Some(guest_list_id) = self.guest_list_id else {
            return Ok(true);
        };

        Ok(match user {
            None => false,
            Some(user) => sqlx::query!(
                "SELECT list_id FROM list_members WHERE list_id = ? AND email = ?",
                guest_list_id,
                user.email
            )
            .fetch_optional(db)
            .await?
            .is_some(),
        })
    }

    #[allow(unused)]
    pub fn is_upcoming(&self, now: NaiveDateTime) -> bool {
        // TODO: use with:
        // let now = Utc::now().naive_utc();
        // let past = query.past.unwrap_or(false);

        now <= self.start || self.end.is_some_and(|end| now <= end)
    }
}
