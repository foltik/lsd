use crate::db::rsvp::EventRsvp;
use crate::db::rsvp_session::RsvpSession;
use crate::prelude::*;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Coupon {
    pub id: i64,
    pub creator_user_id: i64,
    pub event_id: Option<i64>,
    pub code: String,
    pub description: String,
    pub expires_at: Option<NaiveDateTime>,

    pub kind: String,
    pub percent_off: Option<i64>,
    pub dollars_off: Option<i64>,

    pub max_uses: Option<i64>,
    pub max_uses_per_user: Option<i64>,
    pub max_uses_per_event: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateCoupon {
    pub event_id: Option<i64>,
    pub code: String,
    pub description: String,
    pub expires_at: Option<NaiveDateTime>,

    pub kind: String,
    pub percent_off: Option<i64>,
    pub dollars_off: Option<i64>,

    pub max_uses: Option<i64>,
    pub max_uses_per_user: Option<i64>,
    pub max_uses_per_event: Option<i64>,
}

/// Coupon plus usage/display info for the admin list page.
#[derive(Debug, serde::Serialize)]
pub struct CouponWithUses {
    pub id: i64,
    pub creator: String,
    pub code: String,
    pub kind: String,
    pub description: String,
    pub event_title: Option<String>,
    pub expires_at: Option<NaiveDateTime>,

    pub percent_off: Option<i64>,
    pub dollars_off: Option<i64>,

    pub max_uses: Option<i64>,
    pub uses: i64,

    pub user_count: i64,
    pub first_user: Option<String>,
}

impl CouponWithUses {
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|at| at <= Utc::now().naive_utc())
    }
}

/// A use is one free RSVP for spot coupons, one checkout session otherwise.
pub struct CouponUses {
    pub total: i64,
    pub by_user: i64,
}

#[allow(clippy::large_enum_variant)]
pub enum CouponCheck {
    Usable { coupon: Coupon, available: i64 },
    Expired,
    Exhausted,
    Invalid,
}

impl CouponCheck {
    // Unknown, wrong-event, and user-restricted codes all read "Invalid code."
    pub fn error_message(&self) -> Option<&'static str> {
        match self {
            CouponCheck::Usable { .. } => None,
            CouponCheck::Expired => Some("This code has expired."),
            CouponCheck::Exhausted => Some("This code has already been used up."),
            CouponCheck::Invalid => Some("Invalid code."),
        }
    }
}

impl Coupon {
    /// Grants free RSVPs.
    pub const SPOT: &'static str = "spot";
    /// Dollars off the checkout total.
    pub const FIXED: &'static str = "fixed";
    /// Percent off each ticket.
    pub const PERCENT: &'static str = "percent";

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|at| at <= Utc::now().naive_utc())
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<Coupon>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM coupons WHERE id = ?", id)
            .fetch_optional(db)
            .await?)
    }

    pub async fn lookup_by_code(db: &Db, code: &str) -> Result<Option<Coupon>> {
        let code = code.trim().to_uppercase();
        Ok(sqlx::query_as!(Self, "SELECT * FROM coupons WHERE code = ?", code)
            .fetch_optional(db)
            .await?)
    }

    /// List all coupons with use counts for the admin page.
    pub async fn list(db: &Db) -> Result<Vec<CouponWithUses>> {
        Ok(sqlx::query_as!(
            CouponWithUses,
            r#"SELECT
                 c.id,
                 COALESCE(u.first_name, u.email) AS "creator!: String",
                 e.title AS "event_title?: String",
                 c.code,
                 c.description,
                 c.expires_at, 

                 c.kind,
                 c.percent_off,
                 c.dollars_off,

                 c.max_uses,
                 (CASE
                      WHEN c.kind = 'spot' THEN (
                        SELECT COUNT(*) FROM rsvps r
                        JOIN rsvp_sessions s ON s.id = r.session_id
                        WHERE s.coupon_id = c.id AND r.discount > 0
                      )
                      ELSE (
                          SELECT COUNT(*) FROM rsvp_sessions s
                          WHERE s.coupon_id = c.id
                      )
                 END) AS "uses!: i64",

                 (
                    SELECT COUNT(*) FROM coupon_users cu
                    WHERE cu.coupon_id = c.id
                 ) AS "user_count!: i64",

                 (
                    SELECT TRIM(COALESCE(cu_u.first_name, '') || ' ' || COALESCE(cu_u.last_name, ''))
                    FROM coupon_users cu
                    JOIN users cu_u ON cu_u.id = cu.user_id
                    WHERE cu.coupon_id = c.id LIMIT 1
                 ) AS "first_user?: String"

               FROM coupons c
               JOIN users u ON u.id = c.creator_user_id
               LEFT JOIN events e ON e.id = c.event_id
               ORDER BY c.created_at DESC"#
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn create(db: &Db, creator_user_id: i64, coupon: &UpdateCoupon) -> Result<i64> {
        let code = coupon.code.trim().to_uppercase();
        let row = sqlx::query!(
            r#"INSERT INTO coupons
               (creator_user_id, event_id, code, description, expires_at,
                max_uses, max_uses_per_user, max_uses_per_event, kind, percent_off, dollars_off)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            creator_user_id,
            coupon.event_id,
            code,
            coupon.description,
            coupon.expires_at,
            coupon.max_uses,
            coupon.max_uses_per_user,
            coupon.max_uses_per_event,
            coupon.kind,
            coupon.percent_off,
            coupon.dollars_off,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn update(db: &Db, id: i64, coupon: &UpdateCoupon) -> Result<()> {
        let code = coupon.code.trim().to_uppercase();
        sqlx::query!(
            "UPDATE coupons
               SET event_id = ?,
                   code = ?,
                   description = ?,
                   expires_at = ?,
                   max_uses = ?,
                   max_uses_per_user = ?,
                   max_uses_per_event = ?,
                   kind = ?,
                   percent_off = ?,
                   dollars_off = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?",
            coupon.event_id,
            code,
            coupon.description,
            coupon.expires_at,
            coupon.max_uses,
            coupon.max_uses_per_user,
            coupon.max_uses_per_event,
            coupon.kind,
            coupon.percent_off,
            coupon.dollars_off,
            id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Disable a coupon by expiring it now.
    pub async fn disable(db: &Db, id: i64) -> Result<()> {
        sqlx::query!(
            "UPDATE coupons
             SET expires_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn set_users(db: &Db, coupon_id: i64, user_ids: &[i64]) -> Result<()> {
        sqlx::query!("DELETE FROM coupon_users WHERE coupon_id = ?", coupon_id)
            .execute(db)
            .await?;
        for user_id in user_ids {
            sqlx::query!(
                "INSERT OR IGNORE INTO coupon_users (coupon_id, user_id) VALUES (?, ?)",
                coupon_id,
                user_id,
            )
            .execute(db)
            .await?;
        }
        Ok(())
    }

    // Check if user is in the list for this coupon, if any.
    pub async fn is_usable_by(&self, db: &Db, user_id: i64) -> Result<bool> {
        let row = sqlx::query_scalar!(
            r#"SELECT
                   NOT EXISTS(SELECT 1 FROM coupon_users WHERE coupon_id = ?1)
                   OR EXISTS(SELECT 1 FROM coupon_users WHERE coupon_id = ?1 AND user_id = ?2)
               AS "allowed!: bool"
            "#,
            self.id,
            user_id,
        )
        .fetch_one(db)
        .await?;
        Ok(row)
    }

    /// Count uses, excluding the given session so re-applying isn't self-blocked.
    pub async fn past_uses(&self, db: &Db, user_id: i64, exclude_session_id: i64) -> Result<CouponUses> {
        let row = match self.kind.as_str() {
            Self::SPOT => {
                sqlx::query_as!(
                    CouponUses,
                    r#"SELECT
                         COUNT(*) AS "total!: i64",
                         COALESCE(SUM(s.user_id = ?2), 0) AS "by_user!: i64"
                       FROM rsvps r
                       JOIN rsvp_sessions s ON s.id = r.session_id
                       WHERE s.coupon_id = ?1 AND r.discount > 0 AND s.id != ?3"#,
                    self.id,
                    user_id,
                    exclude_session_id,
                )
                .fetch_one(db)
                .await?
            }
            Self::FIXED | Self::PERCENT => {
                sqlx::query_as!(
                    CouponUses,
                    r#"SELECT
                         COUNT(*) AS "total!: i64",
                         COALESCE(SUM(s.user_id = ?), 0) AS "by_user!: i64"
                       FROM rsvp_sessions s
                       WHERE s.coupon_id = ?1 AND s.id != ?3"#,
                    self.id,
                    user_id,
                    exclude_session_id,
                )
                .fetch_one(db)
                .await?
            }
            kind => panic!("unknown kind: {kind}"),
        };
        Ok(row)
    }

    /// Check a code for a session, returning the number of remaining uses.
    pub async fn check(
        db: &Db, code: &str, event_id: i64, session: &RsvpSession, rsvps: &[EventRsvp],
    ) -> Result<CouponCheck> {
        let user_id = session.user_id.expect("Coupon::check() from session without user_id");

        // No such code
        let Some(coupon) = Self::lookup_by_code(db, code).await? else {
            return Ok(CouponCheck::Invalid);
        };
        // Expired
        if coupon.is_expired() {
            return Ok(CouponCheck::Expired);
        }
        // For a different event
        if coupon.event_id.is_some_and(|id| id != event_id) {
            return Ok(CouponCheck::Invalid);
        }
        // Not in list of allowed users
        if !coupon.is_usable_by(db, user_id).await? {
            return Ok(CouponCheck::Invalid);
        }

        // Compute maximum potential remaining uses by kind.
        let mut remaining_uses = match coupon.kind.as_str() {
            Self::SPOT => {
                // Free RSVP coupons may be applied once per >$0 spot.
                let mut num_eligible_spots = rsvps.iter().filter(|r| r.price() > 0).count() as i64;
                // Subject to optional limit.
                if let Some(max_per_rsvp) = coupon.max_uses_per_event {
                    num_eligible_spots = num_eligible_spots.min(max_per_rsvp);
                }
                num_eligible_spots
            }
            // $ and % off coupons may be applied at most once per RSVP session.
            Self::FIXED | Self::PERCENT => 1,
            kind => panic!("unknown coupon kind: {kind}"),
        };

        // Further constrain via past usage stats.
        let past_uses = coupon.past_uses(db, user_id, session.id).await?;

        // Overall max
        if let Some(max) = coupon.max_uses {
            remaining_uses = remaining_uses.min(max - past_uses.total);
        }
        // Per-user max
        if let Some(max_by_user) = coupon.max_uses_per_user {
            remaining_uses = remaining_uses.min(max_by_user - past_uses.by_user);
        }

        Ok(match remaining_uses {
            n if n > 0 => CouponCheck::Usable { coupon, available: remaining_uses },
            _ => CouponCheck::Exhausted,
        })
    }

    /// Whether any coupon might be usable by this user at this event.
    /// Skips checking limits so user can get feedback on exhaustion.
    pub async fn any_usable(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS(
                 SELECT 1 FROM coupons c
                 WHERE (c.event_id IS NULL OR c.event_id = ?)
                   AND (c.expires_at IS NULL OR c.expires_at > CURRENT_TIMESTAMP)
                   AND (NOT EXISTS(SELECT 1 FROM coupon_users u WHERE u.coupon_id = c.id)
                         OR EXISTS(SELECT 1 FROM coupon_users u WHERE u.coupon_id = c.id AND u.user_id = ?))
                   AND (c.max_uses IS NULL OR c.max_uses >
                        (CASE
                            WHEN c.kind = 'spot'
                            THEN (
                                SELECT COUNT(*) FROM rsvps r
                                JOIN rsvp_sessions s ON s.id = r.session_id
                                WHERE s.coupon_id = c.id AND r.discount > 0
                             )
                             ELSE (
                                SELECT COUNT(*) FROM rsvp_sessions s
                                WHERE s.coupon_id = c.id
                             )
                        END)
                    )
               ) AS "any_usable!: bool""#,
            event_id,
            user_id,
        )
        .fetch_one(db)
        .await?)
    }
}
