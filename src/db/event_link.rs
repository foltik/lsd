use crate::prelude::*;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct EventLink {
    pub id: i64,
    pub event_id: i64,
    pub slug: String,
    pub description: String,
    pub password: Option<String>,
    pub capacity: i64,
    pub bypass_capacity: bool,
    pub bypass_closed: bool,
    pub spots_per_person: Option<i64>,
    pub closed: bool,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateEventLink {
    pub slug: String,
    pub description: String,
    pub password: Option<String>,
    pub capacity: i64,
    #[serde(default)]
    pub bypass_capacity: bool,
    #[serde(default)]
    pub bypass_closed: bool,
    #[serde(default, deserialize_with = "empty_as_none")]
    pub spots_per_person: Option<i64>,
    #[serde(default)]
    pub closed: bool,
}

fn empty_as_none<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<i64>, D::Error> {
    let s = <String as serde::Deserialize>::deserialize(d)?;
    match s.as_str() {
        "" => Ok(None),
        s => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, serde::Serialize)]
pub struct EventLinkWithUses {
    pub id: i64,
    pub slug: String,
    pub description: String,
    pub password: Option<String>,
    pub capacity: i64,
    pub bypass_capacity: bool,
    pub bypass_closed: bool,
    pub spots_per_person: Option<i64>,
    pub closed: bool,
    pub uses: i64,
    pub rsvp_count: i64,
}

impl EventLink {
    pub async fn lookup_by_id(db: &Db, event_id: i64, id: i64) -> Result<Option<EventLink>> {
        Ok(
            sqlx::query_as!(Self, "SELECT * FROM event_links WHERE event_id = ? AND id = ?", event_id, id)
                .fetch_optional(db)
                .await?,
        )
    }

    pub async fn lookup_by_slug(db: &Db, event_id: i64, slug: &str) -> Result<Option<EventLink>> {
        Ok(sqlx::query_as!(
            Self,
            "SELECT * FROM event_links WHERE event_id = ? AND slug = ?",
            event_id,
            slug,
        )
        .fetch_optional(db)
        .await?)
    }

    /// List an event's links with use counts for the admin page.
    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<EventLinkWithUses>> {
        Ok(sqlx::query_as!(
            EventLinkWithUses,
            r#"SELECT
                 l.id,
                 l.slug,
                 l.description,
                 l.password,
                 l.capacity,
                 l.bypass_capacity,
                 l.bypass_closed,
                 l.spots_per_person,
                 l.closed,
                 SUM(CASE WHEN s.status IN ('contribution', 'payment_pending', 'payment_confirmed', 'refund_pending', 'refund_confirmed') THEN 1 ELSE 0 END) AS "uses!: i64",
                 SUM(CASE WHEN s.status IN ('payment_pending', 'payment_confirmed', 'refund_pending', 'refund_confirmed') THEN 1 ELSE 0 END) AS "rsvp_count!: i64"
               FROM event_links l
               LEFT JOIN (rsvps r JOIN rsvp_sessions s ON s.id = r.session_id) ON s.link_id = l.id
               WHERE l.event_id = ?
               GROUP BY l.id
               ORDER BY l.created_at"#,
            event_id,
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn create(db: &Db, event_id: i64, link: &UpdateEventLink) -> Result<()> {
        sqlx::query!(
            "INSERT INTO event_links (event_id, slug, description, password, capacity, bypass_capacity, bypass_closed, spots_per_person, closed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            event_id,
            link.slug,
            link.description,
            link.password,
            link.capacity,
            link.bypass_capacity,
            link.bypass_closed,
            link.spots_per_person,
            link.closed,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn update(db: &Db, id: i64, link: &UpdateEventLink) -> Result<()> {
        sqlx::query!(
            "UPDATE event_links
               SET slug = ?,
                   description = ?,
                   password = ?,
                   capacity = ?,
                   bypass_capacity = ?,
                   bypass_closed = ?,
                   spots_per_person = ?,
                   closed = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?",
            link.slug,
            link.description,
            link.password,
            link.capacity,
            link.bypass_capacity,
            link.bypass_closed,
            link.spots_per_person,
            link.closed,
            id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Delete a link *and* its rsvps and rsvp_sessions.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query!(
            "DELETE FROM rsvps WHERE session_id IN (SELECT id FROM rsvp_sessions WHERE link_id = ?)",
            id
        )
        .execute(db)
        .await?;
        sqlx::query!("DELETE FROM rsvp_sessions WHERE link_id = ?", id)
            .execute(db)
            .await?;
        sqlx::query!("DELETE FROM event_links WHERE id = ?", id).execute(db).await?;
        Ok(())
    }
}
