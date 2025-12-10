use sqlx::QueryBuilder;

use crate::prelude::*;

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
}
