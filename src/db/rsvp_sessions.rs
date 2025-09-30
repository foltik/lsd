use rand::rngs::OsRng;
use rand::Rng;

use crate::db::rsvp::Rsvp;
use crate::db::spot::Spot;
use crate::prelude::*;
use crate::utils::stripe;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct RsvpSession {
    pub id: i64,
    pub event_id: i64,
    pub user_id: Option<i64>,
    pub token: String,

    pub status: String,
    pub transaction_id: Option<i64>,
    pub refund_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdateRsvpSession {
    pub status: String,
    pub transaction_id: Option<i64>,
    pub refund_id: Option<i64>,
}

impl RsvpSession {
    pub async fn lookup_by_token(db: &Db, token: &str) -> AppResult<Option<RsvpSession>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvp_sessions WHERE token = ?"#, token)
            .fetch_optional(db)
            .await?)
    }

    pub async fn create(db: &Db, event_id: i64, user: &Option<User>) -> AppResult<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());
        let user_id = user.as_ref().map(|u| u.id);

        sqlx::query!(
            r#"INSERT INTO rsvp_sessions
               (event_id, user_id, token, status)
               VALUES (?, ?, ?, 'pending')"#,
            event_id,
            user_id,
            token,
        )
        .execute(db)
        .await?;

        Ok(token)
    }

    pub async fn update(db: &Db, id: i64, rsvp: &UpdateRsvpSession) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE rsvp_sessions
               SET status = ?,
                   transaction_id = ?,
                   refund_id = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            rsvp.status,
            rsvp.transaction_id,
            rsvp.refund_id,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn line_items(&self, db: &Db) -> AppResult<Vec<stripe::LineItem>> {
        let spots = Spot::list_for_event(db, self.event_id).await?;
        let rsvps = Rsvp::list_for_session(db, self.id).await?;

        let mut spot_rsvps: HashMap<String, (i64, i64)> = Default::default();
        for rsvp in rsvps {
            // unwrap(): instances of Rsvp are guaranteed to have a valid spot_id on insert.
            let spot = spots.iter().find(|s| s.id == rsvp.spot_id).unwrap();

            let entry = spot_rsvps.entry(spot.name.clone()).or_insert((1, rsvp.contribution));
            entry.0 += 1; // quantity++
        }

        let line_items = spot_rsvps
            .into_iter()
            .map(|(name, (quantity, price))| stripe::LineItem { name, quantity, price })
            .collect::<Vec<_>>();

        Ok(line_items)
    }
}
