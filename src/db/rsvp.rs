use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Rsvp {
    pub id: i64,
    pub event_id: i64,
    pub user_id: i64,
    pub price_paid: i64,
    pub stripe_payment_id: Option<String>,
    pub payment_status: Option<String>,
    pub promo_code_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub refund_amount: Option<i64>,
    pub refund_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateRsvp {
    pub event_id: i64,
    pub user_id: i64,
    pub price_paid: i64,
    pub stripe_payment_id: Option<String>,
    pub payment_status: Option<String>,
    pub promo_code_id: Option<i64>,
}

impl Rsvp {
    /// Payment is pending.
    pub const PAYMENT_PENDING: &'static str = "pending";
    /// Payment succeeded.
    pub const PAYMENT_SUCCEEDED: &'static str = "succeeded";
    /// Payment failed.
    pub const PAYMENT_FAILED: &'static str = "failed";

    /// Create the `rsvps` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS rsvps ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                event_id INTEGER NOT NULL, \
                user_id INTEGER NOT NULL, \
                price_paid INTEGER NOT NULL, \
                stripe_payment_id TEXT, \
                payment_status TEXT, \
                promo_code_id INTEGER, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                cancelled_at TIMESTAMP, \
                refund_amount INTEGER, \
                refund_type TEXT, \
                FOREIGN KEY (event_id) REFERENCES events(id), \
                FOREIGN KEY (user_id) REFERENCES users(id), \
                FOREIGN KEY (promo_code_id) REFERENCES promo_codes(id) \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new RSVP.
    pub async fn create(db: &Db, rsvp: &CreateRsvp) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO rsvps ( \
                event_id, user_id, price_paid, stripe_payment_id, payment_status, promo_code_id \
            ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(rsvp.event_id)
        .bind(rsvp.user_id)
        .bind(rsvp.price_paid)
        .bind(&rsvp.stripe_payment_id)
        .bind(&rsvp.payment_status)
        .bind(rsvp.promo_code_id)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup an RSVP by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Rsvp>> {
        let rsvp = sqlx::query_as::<_, Rsvp>("SELECT * FROM rsvps WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(rsvp)
    }

    /// Lookup an RSVP by event and user.
    pub async fn lookup_by_event_and_user(
        db: &Db,
        event_id: i64,
        user_id: i64,
    ) -> Result<Option<Rsvp>> {
        let rsvp = sqlx::query_as::<_, Rsvp>(
            "SELECT * FROM rsvps WHERE event_id = ? AND user_id = ? AND cancelled_at IS NULL",
        )
        .bind(event_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(rsvp)
    }

    /// Update payment status.
    pub async fn update_payment_status(
        &self,
        db: &Db,
        status: &str,
        stripe_payment_id: Option<String>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE rsvps SET \
                payment_status = ?, \
                stripe_payment_id = ? \
             WHERE id = ?",
        )
        .bind(status)
        .bind(stripe_payment_id)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Cancel an RSVP.
    pub async fn cancel(
        &self,
        db: &Db,
        refund_amount: Option<i64>,
        refund_type: Option<String>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE rsvps SET \
                cancelled_at = CURRENT_TIMESTAMP, \
                refund_amount = ?, \
                refund_type = ? \
             WHERE id = ?",
        )
        .bind(refund_amount)
        .bind(refund_type)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Get all RSVPs for an event.
    pub async fn list_for_event(db: &Db, event_id: i64) -> Result<Vec<Rsvp>> {
        let rsvps = sqlx::query_as::<_, Rsvp>(
            "SELECT * FROM rsvps \
             WHERE event_id = ? AND cancelled_at IS NULL \
             ORDER BY created_at",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;
        Ok(rsvps)
    }
} 