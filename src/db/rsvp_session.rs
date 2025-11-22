use rand::Rng;
use rand::rngs::OsRng;

use crate::db::event::Event;
use crate::db::rsvp::ContributionRsvp;
use crate::prelude::*;
use crate::utils::stripe;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct RsvpSession {
    pub id: i64,
    pub event_id: i64,
    pub token: String,
    pub status: String,

    pub user_id: Option<i64>,
    pub user_version: Option<i64>,

    pub stripe_client_secret: Option<String>,
    pub stripe_payment_intent_id: Option<i64>,
    pub stripe_charge_id: Option<i64>,
    pub stripe_refund_id: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl RsvpSession {
    pub const DRAFT: &str = "draft";
    pub const AWAITING_PAYMENT: &str = "awaiting_payment";
    pub const CONFIRMED: &str = "confirmed";

    pub const DRAFT_EXPIRY_TIME_SQL: &str = "-15 minutes";

    fn cookie(&self) -> String {
        Cookie::build(("rsvp_session", &self.token))
            .secure(config().acme.is_some())
            .http_only(true)
            .same_site(cookie::SameSite::Strict)
            .domain(&config().app.domain)
            .to_string()
    }

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

    pub async fn lookup_for_user_and_event(
        db: &Db, user: &User, event: &Event,
    ) -> AppResult<Option<RsvpSession>> {
        let session = sqlx::query_as!(
            Self,
            r#"SELECT * FROM rsvp_sessions
               WHERE user_id = ? AND event_id = ?"#,
            user.id,
            event.id
        )
        .fetch_optional(db)
        .await?;
        Ok(session)
    }

    pub async fn get_or_create(
        db: &Db, user: &Option<User>, session: &Option<RsvpSession>, event_id: i64,
    ) -> AppResult<[(HeaderName, String); 1]> {
        let session = match session {
            Some(session) => session.clone(),
            None => RsvpSession::create(db, event_id, user).await?,
        };

        Ok([(header::SET_COOKIE, session.cookie())])
    }

    pub async fn create(db: &Db, event_id: i64, user: &Option<User>) -> AppResult<Self> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let user = user.as_ref();
        let user_id = user.map(|u| u.id);
        let user_version = user.map(|u| u.version);

        let session = sqlx::query_as!(
            Self,
            r#"INSERT INTO rsvp_sessions
               (event_id, token, status, user_id, user_version)
               VALUES (?, ?, ?, ?, ?)
               RETURNING *"#,
            event_id,
            token,
            Self::DRAFT,
            user_id,
            user_version,
        )
        .fetch_one(db)
        .await?;

        Ok(session)
    }

    pub async fn delete(&self, db: &Db) -> AppResult<()> {
        sqlx::query!("DELETE FROM rsvps WHERE session_id = ?", self.id)
            .execute(db)
            .await?;
        sqlx::query!("DELETE FROM rsvp_sessions WHERE id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Update
    pub async fn set_user(&mut self, db: &Db, user: &User) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
                SET user_id = ?,
                    user_version = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?",
            user.id,
            user.version,
            self.id,
        )
        .execute(db)
        .await?;
        self.user_id = Some(user.id);
        self.user_version = Some(user.version);
        Ok(())
    }

    pub async fn set_awaiting_payment(&self, db: &Db) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET status = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            Self::AWAITING_PAYMENT,
            self.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_confirmed(&self, db: &Db, payment_intent_id: Option<&str>) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET status = ?,
                 stripe_payment_intent_id = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            Self::CONFIRMED,
            payment_intent_id,
            self.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_stripe_client_secret(
        &mut self, db: &Db, id: i64, stripe_client_secret: &str,
    ) -> AppResult<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET stripe_client_secret = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            stripe_client_secret,
            id
        )
        .execute(db)
        .await?;
        self.stripe_client_secret = Some(stripe_client_secret.into());
        Ok(())
    }

    pub async fn delete_expired_drafts(db: &Db) -> AppResult<()> {
        sqlx::query!(
            "DELETE FROM rsvp_sessions
             WHERE status = ?
             AND updated_at < datetime('now', ?)",
            Self::DRAFT,
            Self::DRAFT_EXPIRY_TIME_SQL,
        )
        .execute(db)
        .await?;

        sqlx::query!(
            "DELETE FROM rsvps AS r
             WHERE NOT EXISTS (
                SELECT 1 FROM rsvp_sessions s
                WHERE s.id = r.session_id
            )"
        )
        .execute(db)
        .await?;

        Ok(())
    }

    pub fn line_items(&self, rsvps: &[ContributionRsvp]) -> AppResult<Vec<stripe::LineItem>> {
        let mut spot_rsvps: HashMap<String, (i64, i64)> = Default::default();
        for rsvp in rsvps {
            let entry = spot_rsvps.entry(rsvp.spot_name.clone()).or_insert((1, rsvp.contribution));
            entry.0 += 1; // quantity++
        }

        let line_items = spot_rsvps
            .into_iter()
            .map(|(name, (quantity, price))| stripe::LineItem { name, quantity, price })
            .collect::<Vec<_>>();

        Ok(line_items)
    }
}
