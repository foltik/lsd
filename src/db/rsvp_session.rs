use rand::Rng;
use rand::rngs::OsRng;

use crate::db::rsvp::Rsvp;
use crate::db::spot::Spot;
use crate::prelude::*;
use crate::utils::stripe;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct RsvpSession {
    pub id: i64,
    pub event_id: i64,
    pub token: String,
    pub status: String,

    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub user_id: Option<i64>,

    pub stripe_client_secret: Option<String>,
    pub stripe_payment_intent_id: Option<i64>,
    pub stripe_charge_id: Option<i64>,
    pub stripe_refund_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl RsvpSession {
    pub const PENDING: &str = "pending";
    pub const PAID: &str = "paid";

    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<RsvpSession>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvp_sessions WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }
    pub async fn lookup_by_token(db: &Db, token: &str) -> AppResult<Option<RsvpSession>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvp_sessions WHERE token = ?"#, token)
            .fetch_optional(db)
            .await?)
    }
    pub async fn lookup_conflicts(
        db: &Db,
        event_id: i64,
        session_id: i64,
        email: &str,
    ) -> AppResult<Option<Self>> {
        Ok(sqlx::query_as!(
            Self,
            r"SELECT * FROM rsvp_sessions
              WHERE event_id = ? AND id != ? AND email = ?",
            event_id,
            session_id,
            email
        )
        .fetch_optional(db)
        .await?)
    }
    pub async fn lookup_status(db: &Db, id: i64) -> AppResult<Option<String>> {
        #[derive(sqlx::FromRow)]
        pub struct RsvpStatus {
            status: String,
        }
        Ok(
            sqlx::query_as!(RsvpStatus, r#"SELECT status FROM rsvp_sessions WHERE id = ?"#, id)
                .fetch_optional(db)
                .await?
                .map(|r| r.status),
        )
    }

    pub async fn create(db: &Db, event_id: i64, user: &Option<User>) -> AppResult<String> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let user = user.as_ref();
        let first_name = user.map(|u| &u.first_name);
        let last_name = user.map(|u| &u.last_name);
        let email = user.map(|u| &u.email);
        let user_id = user.map(|u| u.id);

        sqlx::query!(
            r#"INSERT INTO rsvp_sessions
               (event_id, token, status, first_name, last_name, email, user_id)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            event_id,
            token,
            Self::PENDING,
            first_name,
            last_name,
            email,
            user_id,
        )
        .execute(db)
        .await?;

        Ok(token)
    }

    pub async fn delete(&self, db: &Db) -> AppResult<()> {
        sqlx::query!("DELETE FROM rsvp_sessions WHERE id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn set_contact(
        &mut self,
        db: &Db,
        first_name: String,
        last_name: String,
        email: String,
    ) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET first_name = ?,
                 last_name = ?,
                 email = ?
             WHERE id = ?",
            first_name,
            last_name,
            email,
            self.id,
        )
        .execute(db)
        .await?;
        self.first_name = Some(first_name);
        self.last_name = Some(last_name);
        self.email = Some(email);
        Ok(())
    }

    pub async fn set_paid(&self, db: &Db, payment_intent_id: Option<&str>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET status = ?,
                 stripe_payment_intent_id = ?
             WHERE id = ?",
            Self::PAID,
            payment_intent_id,
            self.id
        )
        .execute(db)
        .await?;

        sqlx::query!("UPDATE rsvps SET status = ? WHERE session_id = ?", Self::PAID, self.id)
            .execute(db)
            .await?;

        Ok(())
    }

    pub async fn set_stripe_client_secret(db: &Db, id: i64, stripe_client_secret: &str) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions SET stripe_client_secret = ? WHERE id = ?",
            stripe_client_secret,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub fn line_items(&self, spots: &[Spot], rsvps: &[Rsvp]) -> AppResult<Vec<stripe::LineItem>> {
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
