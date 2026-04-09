use rand::Rng;
use rand::rngs::OsRng;

use crate::db::event::Event;
use crate::db::rsvp::ContributionRsvp;
use crate::prelude::*;
use crate::utils::stripe::{self, Stripe};

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct RsvpSession {
    pub id: i64,
    pub event_id: i64,
    pub token: String,
    pub status: String,

    pub user_id: Option<i64>,
    pub user_version: Option<i64>,
    pub parent_session_id: Option<i64>,

    pub stripe_checkout_session_id: Option<String>,
    pub stripe_client_secret: Option<String>,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_charge_id: Option<i64>,
    pub stripe_refund_id: Option<String>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl RsvpSession {
    pub const SELECTION: &str = "selection";
    pub const ATTENDEES: &str = "attendees";
    pub const CONTRIBUTION: &str = "contribution";
    pub const PAYMENT_PENDING: &str = "payment_pending";
    pub const PAYMENT_CONFIRMED: &str = "payment_confirmed";
    pub const REFUND_PENDING: &str = "refund_pending";
    pub const REFUND_CONFIRMED: &str = "refund_confirmed";

    pub const EXPIRY_TIME_SQL: &str = "-31 minutes";
    pub const STRIPE_EXPIRY_MINUTES: i64 = 30;

    /// Returns true if the stripe client secret is expired (older than 14 minutes).
    pub fn is_stripe_expired(&self) -> bool {
        let now = Utc::now().naive_utc();
        let age = now - self.updated_at;
        age.num_minutes() >= Self::STRIPE_EXPIRY_MINUTES
    }

    pub fn cookie(&self, path: &str) -> String {
        Cookie::build(("rsvp_session", &self.token))
            .secure(config().acme.is_some())
            .http_only(true)
            .same_site(cookie::SameSite::Strict)
            .domain(&config().app.domain)
            .path(path)
            .to_string()
    }

    pub fn child_cookie(&self, path: &str) -> String {
        Cookie::build(("rsvp_child_session", &self.token))
            .secure(config().acme.is_some())
            .http_only(true)
            .same_site(cookie::SameSite::Strict)
            .domain(&config().app.domain)
            .path(path)
            .to_string()
    }

    pub async fn user(&self, db: &Db) -> Result<Option<User>> {
        Ok(match self.user_id {
            Some(id) => {
                let user = User::lookup_by_id(db, id)
                    .await?
                    .ok_or_else(|| any!("bad user_id={id} in rsvp_session={}", self.token))?;
                Some(user)
            }
            None => None,
        })
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<RsvpSession>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvp_sessions WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }

    pub async fn lookup_by_token(db: &Db, token: &str) -> Result<Option<RsvpSession>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvp_sessions WHERE token = ?"#, token)
            .fetch_optional(db)
            .await?)
    }

    pub async fn lookup_draft_child(db: &Db, parent_id: i64) -> Result<Option<Self>> {
        Ok(sqlx::query_as!(
            Self,
            r#"SELECT * FROM rsvp_sessions
               WHERE parent_session_id = ?
                 AND status IN (?, ?, ?)
               LIMIT 1"#,
            parent_id,
            Self::SELECTION,
            Self::ATTENDEES,
            Self::CONTRIBUTION,
        )
        .fetch_optional(db)
        .await?)
    }

    /// List all sessions (parent + children) for a user.
    pub async fn list_for_user(db: &Db, user_id: i64, event_id: i64) -> Result<Vec<Self>> {
        Ok(sqlx::query_as!(
            Self,
            r#"SELECT * FROM rsvp_sessions
               WHERE user_id = ? AND event_id = ?
                 AND status IN (?, ?)"#,
            user_id,
            event_id,
            Self::PAYMENT_PENDING,
            Self::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn create(db: &Db, event_id: i64, user: &Option<User>) -> Result<Self> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let user = user.as_ref();
        let user_id = user.map(|u| u.id);
        let user_email = user.map(|u| u.email.as_str());
        let user_version = user.map(|u| u.version);

        let session = sqlx::query_as!(
            Self,
            r#"INSERT INTO rsvp_sessions
               (event_id, token, status, user_id, user_version)
               VALUES (?, ?, ?, ?, ?)
               RETURNING *"#,
            event_id,
            token,
            Self::SELECTION,
            user_id,
            user_version,
        )
        .fetch_one(db)
        .await?;

        tracing::info!(
            "Created RSVP session with session_id={} event_id={event_id} user_id={user_id:?} user_email={user_email:?}",
            session.id
        );
        Ok(session)
    }

    pub async fn create_child(db: &Db, parent: &RsvpSession) -> Result<Self> {
        let token = format!("{:08x}", OsRng.r#gen::<u64>());
        let session = sqlx::query_as!(
            Self,
            r#"INSERT INTO rsvp_sessions
               (event_id, token, status, user_id, user_version, parent_session_id)
               VALUES (?, ?, ?, ?, ?, ?)
               RETURNING *"#,
            parent.event_id,
            token,
            Self::SELECTION,
            parent.user_id,
            parent.user_version,
            parent.id,
        )
        .fetch_one(db)
        .await?;

        tracing::info!(
            "Created child RSVP session with session_id={} parent_session_id={} event_id={}",
            session.id,
            parent.id,
            parent.event_id,
        );
        Ok(session)
    }

    pub async fn delete(&self, db: &Db, stripe: &Stripe) -> Result<()> {
        tracing::info!(
            "Deleting RSVP session with session_id={} event_id={} status={:?}",
            self.id,
            self.event_id,
            self.status
        );

        if let Some(checkout_session_id) = self.stripe_checkout_session_id.as_ref() {
            tracing::info!("Expiring stripe_checkout_session_id={checkout_session_id}");
            stripe.expire_session(checkout_session_id).await?;
        }

        sqlx::query!("DELETE FROM rsvps WHERE session_id = ?", self.id)
            .execute(db)
            .await?;
        sqlx::query!("DELETE FROM rsvp_sessions WHERE id = ?", self.id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn takeover_for_event(&self, db: &Db, event: &Event, email: &str) -> Result<()> {
        tracing::info!(
            "RSVP session takeover with session_id={} event_id={} taking_over_email={email:?}",
            self.id,
            event.id,
        );
        sqlx::query!(
            "DELETE FROM rsvp_sessions
             WHERE user_id IN (
                 SELECT u.id
                 FROM users u
                 WHERE u.email = ? COLLATE NOCASE
             )
             AND id != ?
             AND event_id = ?",
            email,
            self.id,
            event.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Update
    pub async fn set_user(&mut self, db: &Db, user: &User) -> Result<()> {
        tracing::info!(
            "Setting user on RSVP session with session_id={} event_id={} user_id={} user_email={:?}",
            self.id,
            self.event_id,
            user.id,
            user.email
        );
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

    pub async fn set_status(&self, db: &Db, status: &str) -> Result<()> {
        tracing::info!(
            "RSVP status transition with session_id={} event_id={} user_id={:?} status={:?} -> {status:?}",
            self.id,
            self.event_id,
            self.user_id,
            self.status,
        );
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET status = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            status,
            self.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_payment_intent_id(&self, db: &Db, payment_intent_id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET stripe_payment_intent_id = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            payment_intent_id,
            self.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_refund_id(&self, db: &Db, refund_id: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET stripe_refund_id = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            refund_id,
            self.id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_stripe_checkout_session(
        &mut self, db: &Db, checkout_session_id: &str, client_secret: &str,
    ) -> Result<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET stripe_checkout_session_id = ?, stripe_client_secret = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            checkout_session_id,
            client_secret,
            self.id
        )
        .execute(db)
        .await?;
        self.stripe_checkout_session_id = Some(checkout_session_id.into());
        self.stripe_client_secret = Some(client_secret.into());
        Ok(())
    }

    pub async fn clear_stripe_client_secret(&mut self, db: &Db) -> Result<()> {
        sqlx::query!(
            "UPDATE rsvp_sessions
             SET stripe_client_secret = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            self.id
        )
        .execute(db)
        .await?;
        self.stripe_client_secret = None;
        Ok(())
    }

    pub async fn delete_expired(db: &Db) -> Result<()> {
        sqlx::query!(
            "DELETE FROM rsvp_sessions
             WHERE status in (?, ?, ?)
             AND updated_at < datetime('now', ?)",
            Self::SELECTION,
            Self::ATTENDEES,
            Self::CONTRIBUTION,
            Self::EXPIRY_TIME_SQL,
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

    pub fn line_items(&self, rsvps: &[ContributionRsvp]) -> Result<Vec<stripe::LineItem>> {
        let mut spot_rsvps: HashMap<String, (i64, i64)> = Default::default();
        for rsvp in rsvps {
            let entry = spot_rsvps.entry(rsvp.spot_name.clone()).or_insert((0, rsvp.contribution));
            entry.0 += 1; // quantity++
        }

        let line_items = spot_rsvps
            .into_iter()
            .map(|(name, (quantity, price))| stripe::LineItem { name, quantity, price })
            .collect::<Vec<_>>();

        Ok(line_items)
    }

    /// List all sessions for debug view (only for events within last 24hrs).
    pub async fn list_debug(db: &Db) -> Result<Vec<DebugSession>> {
        let sessions = sqlx::query_as!(
            DebugSessionRow,
            r#"SELECT
                s.id,
                s.token,
                s.status,
                s.created_at,
                s.updated_at,
                e.title AS event_title,
                e.slug AS event_slug,
                u.email AS user_email,
                ps.token AS parent_token
            FROM rsvp_sessions s
            JOIN events e ON e.id = s.event_id
            LEFT JOIN users u ON u.id = s.user_id
            LEFT JOIN rsvp_sessions ps ON ps.id = s.parent_session_id
            WHERE e.start > datetime('now', '-24 hours')
            ORDER BY s.updated_at DESC"#
        )
        .fetch_all(db)
        .await?;

        let rsvps = sqlx::query_as!(
            DebugRsvp,
            r#"SELECT
                r.session_id,
                sp.name AS spot_name,
                r.contribution,
                u.email
            FROM rsvps r
            JOIN spots sp ON sp.id = r.spot_id
            LEFT JOIN users u ON u.id = r.user_id"#
        )
        .fetch_all(db)
        .await?;

        let now = Utc::now().naive_utc();
        let mut rsvps_by_session: HashMap<i64, Vec<DebugRsvp>> = HashMap::new();
        for rsvp in rsvps {
            rsvps_by_session.entry(rsvp.session_id).or_default().push(rsvp);
        }

        Ok(sessions
            .into_iter()
            .map(|s| {
                let expires_in = 31 - (now - s.updated_at).num_minutes();
                DebugSession {
                    id: s.id,
                    token: s.token,
                    status: s.status,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                    event_title: s.event_title,
                    event_slug: s.event_slug,
                    user_email: s.user_email,
                    parent_token: s.parent_token,
                    rsvps: rsvps_by_session.remove(&s.id).unwrap_or_default(),
                    expires_in,
                }
            })
            .collect())
    }
}

struct DebugSessionRow {
    id: i64,
    token: String,
    status: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    event_title: String,
    event_slug: String,
    user_email: Option<String>,
    parent_token: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DebugSession {
    pub id: i64,
    pub token: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub event_title: String,
    pub event_slug: String,
    pub user_email: Option<String>,
    pub parent_token: Option<String>,
    pub rsvps: Vec<DebugRsvp>,
    pub expires_in: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct DebugRsvp {
    pub session_id: i64,
    pub spot_name: String,
    pub contribution: i64,
    pub email: Option<String>,
}
