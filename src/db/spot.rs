use sqlx::QueryBuilder;

use crate::db::rsvp::EventRsvp;
use crate::prelude::*;

/// RSVP counts per spot split by status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpotCounts {
    /// Confirmed RSVPs (payment_pending or payment_confirmed)
    pub rsvp_count: i64,
    /// In-progress checkouts (selection, attendees, contribution)
    pub cart_count: i64,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Spot {
    pub id: i64,

    pub name: String,
    pub description: String,
    pub qty_total: i64,
    pub qty_per_person: i64,
    pub kind: String,
    pub sort: i64,

    // kind = 'fixed'
    pub required_contribution: Option<i64>,
    // kind = 'variable'
    pub min_contribution: Option<i64>,
    pub max_contribution: Option<i64>,
    pub suggested_contribution: Option<i64>,
    // kind = 'work'
    pub required_notice_hours: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateSpot {
    pub id: Option<i64>,

    pub name: String,
    pub description: String,
    pub qty_total: i64,
    pub qty_per_person: i64,
    pub kind: String,
    pub sort: i64,

    // kind = 'fixed'
    pub required_contribution: Option<i64>,
    // kind = 'variable'
    pub min_contribution: Option<i64>,
    pub max_contribution: Option<i64>,
    pub suggested_contribution: Option<i64>,
    // kind = 'work'
    pub required_notice_hours: Option<i64>,
}

impl Spot {
    /// A free spot.
    pub const FREE: &'static str = "free";
    /// A fixed contribution spot.
    pub const FIXED: &'static str = "fixed";
    /// A variable contribution spot.
    pub const VARIABLE: &'static str = "variable";
    /// A work trade spot.
    pub const WORK: &'static str = "work";

    pub async fn list_ids_for_event(db: &Db, event_id: i64) -> Result<Vec<i64>> {
        Ok(sqlx::query!("SELECT spot_id FROM event_spots WHERE event_id = ?", event_id)
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|row| row.spot_id)
            .collect())
    }

    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<Spot>> {
        Ok(sqlx::query_as!(
            Spot,
            r#"SELECT s.*
               FROM spots s
               JOIN event_spots es ON es.spot_id = s.id
               WHERE es.event_id = ?
               ORDER BY s.sort
            "#,
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    /// Get RSVP counts per spot for an event.
    /// Returns (rsvp_count, cart_count) where:
    /// - rsvp_count: confirmed RSVPs (payment_pending or payment_confirmed)
    /// - cart_count: in-progress checkouts (selection, attendees, contribution)
    pub async fn rsvp_counts_for_event(
        db: &Db, event_id: i64,
    ) -> Result<std::collections::HashMap<i64, SpotCounts>> {
        let rows = sqlx::query!(
            r#"SELECT
                 r.spot_id,
                 SUM(CASE WHEN rs.status IN ('payment_pending', 'payment_confirmed') THEN 1 ELSE 0 END) as "rsvp_count!: i64",
                 SUM(CASE WHEN rs.status IN ('selection', 'attendees', 'contribution') THEN 1 ELSE 0 END) as "cart_count!: i64"
               FROM rsvps r
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               WHERE rs.event_id = ?
               GROUP BY r.spot_id"#,
            event_id
        )
        .fetch_all(db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| (r.spot_id, SpotCounts { rsvp_count: r.rsvp_count, cart_count: r.cart_count }))
            .collect())
    }

    /// Create a new spot.
    pub async fn create(db: &Db, spot: &UpdateSpot) -> Result<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO spots
               (name, description, qty_total, qty_per_person, kind, sort, required_contribution, min_contribution, max_contribution, suggested_contribution, required_notice_hours)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            spot.name,
            spot.description,
            spot.qty_total,
            spot.qty_per_person,
            spot.kind,
            spot.sort,
            spot.required_contribution,
            spot.min_contribution,
            spot.max_contribution,
            spot.suggested_contribution,
            spot.required_notice_hours,
        )
        .execute(db)
        .await?;

        Ok(row.last_insert_rowid())
    }

    /// Update an existing spot.
    pub async fn update(db: &Db, id: i64, spot: &UpdateSpot) -> Result<()> {
        sqlx::query!(
            "UPDATE spots
               SET name = ?,
                   description = ?,
                   qty_total = ?,
                   qty_per_person = ?,
                   kind = ?,
                   sort = ?,
                   required_contribution = ?,
                   min_contribution = ?,
                   max_contribution = ?,
                   suggested_contribution = ?,
                   required_notice_hours = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?",
            spot.name,
            spot.description,
            spot.qty_total,
            spot.qty_per_person,
            spot.kind,
            spot.sort,
            spot.required_contribution,
            spot.min_contribution,
            spot.max_contribution,
            spot.suggested_contribution,
            spot.required_notice_hours,
            id
        )
        .execute(db)
        .await?;

        Ok(())
    }

    pub async fn add_to_event(db: &Db, event_id: i64, spot_ids: Vec<i64>) -> Result<()> {
        if spot_ids.is_empty() {
            return Ok(());
        }

        QueryBuilder::new("INSERT INTO event_spots (event_id, spot_id) ")
            .push_values(spot_ids, |mut b, spot_id| {
                b.push_bind(event_id).push_bind(spot_id);
            })
            .build()
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn remove_from_event(db: &Db, event_id: i64, spot_ids: Vec<i64>) -> Result<()> {
        if spot_ids.is_empty() {
            return Ok(());
        }

        // Remove the event_spots associations
        QueryBuilder::new("DELETE FROM event_spots WHERE event_id = ")
            .push_bind(event_id)
            .push("AND spot_id IN ")
            .push_tuples(&spot_ids, |mut b, spot_id| {
                b.push_bind(spot_id);
            })
            .build()
            .execute(db)
            .await?;

        Ok(())
    }

    /// Duplicate all spots from one event to another.
    /// Creates new spot records (copies) and links them to the new event.
    pub async fn duplicate_for_event(db: &Db, source_event_id: i64, target_event_id: i64) -> Result<()> {
        let spots = Spot::list_for_event(db, source_event_id).await?;
        let mut new_spot_ids = Vec::with_capacity(spots.len());

        for spot in spots {
            let new_spot_id = sqlx::query!(
                r#"INSERT INTO spots
                   (name, description, qty_total, qty_per_person, kind, sort,
                    required_contribution, min_contribution, max_contribution,
                    suggested_contribution, required_notice_hours)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                spot.name,
                spot.description,
                spot.qty_total,
                spot.qty_per_person,
                spot.kind,
                spot.sort,
                spot.required_contribution,
                spot.min_contribution,
                spot.max_contribution,
                spot.suggested_contribution,
                spot.required_notice_hours,
            )
            .execute(db)
            .await?
            .last_insert_rowid();

            new_spot_ids.push(new_spot_id);
        }

        Spot::add_to_event(db, target_event_id, new_spot_ids).await?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SpotStats {
    pub stats: HashMap<i64, Vec<SpotStat>>,
}

// Stat about VARIABLE spot, such as min/median/max contribution.
#[derive(Debug, serde::Serialize)]
pub struct SpotStat {
    pub name: String,
    pub value: i64,
}

impl Spot {
    /// Compute contribution statistics for VARIABLE spots given a list of rsvps.
    pub fn stats(spots: &[Spot], rsvps: &[EventRsvp]) -> SpotStats {
        let mut contributions: HashMap<i64, Vec<i64>> = HashMap::default();
        for rsvp in rsvps {
            contributions.entry(rsvp.spot_id).or_default().push(rsvp.contribution);
        }

        let mut variable_stats = HashMap::default();
        for spot in spots.iter().filter(|s| s.kind == Spot::VARIABLE) {
            let Some(values) = contributions.get_mut(&spot.id) else {
                continue;
            };

            let n = values.len();
            values.sort_unstable();
            let median = if n.is_multiple_of(2) {
                let l = values[n / 2 - 1];
                let r = values[n / 2];
                (l + r) / 2
            } else {
                values[n / 2]
            };
            let max = values.last().copied().unwrap();

            // Only add the max if it's different from the median to avoid clutter
            let mut stats = vec![];
            stats.push(SpotStat { name: "Median".into(), value: median });
            if max > median {
                stats.push(SpotStat { name: "Max".into(), value: max });
            }

            variable_stats.insert(spot.id, stats);
        }

        SpotStats { stats: variable_stats }
    }
}
