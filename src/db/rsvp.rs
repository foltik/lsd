use crate::db::event::Event;
use crate::db::rsvp_session::RsvpSession;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Rsvp {
    pub id: i64,
    pub session_id: i64,

    pub spot_id: i64,
    pub contribution: i64,
    pub user_id: Option<i64>,
    pub user_version: Option<i64>,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

#[derive(serde::Deserialize)]
pub struct CreateRsvp {
    pub session_id: i64,
    pub spot_id: i64,
    pub contribution: i64,
    pub user_id: Option<i64>,
    pub user_version: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct AttendeeRsvp {
    pub rsvp_id: i64,
    pub user_id: Option<i64>,
    pub spot_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub contribution: i64,
}

#[derive(serde::Serialize)]
pub struct ContributionRsvp {
    pub spot_name: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub contribution: i64,
}

#[derive(Clone)]
pub struct EventRsvp {
    pub rsvp_id: i64,
    pub spot_id: i64,
    pub contribution: i64,
}

pub struct UserRsvp {
    pub status: String,
    pub email: String,
    /// The user_id of the attendee whose email this is.
    pub attendee_user_id: i64,
    /// The user_id who owns the session this attendee is reserved under.
    pub owner_user_id: Option<i64>,
    pub owner_first_name: Option<String>,
    pub owner_last_name: Option<String>,
}

struct UserRsvpRow {
    session_id: i64,
    status: String,
    email: String,
    attendee_user_id: i64,
    owner_user_id: Option<i64>,
    owner_first_name: Option<String>,
    owner_last_name: Option<String>,
}

pub struct AdminAttendeesRsvp {
    pub user_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub guest_of: Option<String>,

    pub spot_name: Option<String>,
    pub contribution: i64,

    pub is_manual: bool,
    pub created_at: NaiveDateTime,

    pub session_id: i64,
    pub session_token: Option<String>,
    pub status: String,
    pub note: Option<String>,
}

impl AdminAttendeesRsvp {
    pub fn is_refunded(&self) -> bool {
        matches!(
            self.status.as_str(),
            RsvpSession::REFUND_PENDING | RsvpSession::REFUND_CONFIRMED
        )
    }

    /// Human label for the status column.
    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            RsvpSession::PAYMENT_CONFIRMED => "Confirmed",
            RsvpSession::PAYMENT_PENDING => "Unpaid",
            RsvpSession::REFUND_PENDING | RsvpSession::REFUND_CONFIRMED => "Refunded",
            "manual" => "Manual",
            _ => "",
        }
    }
}

impl Rsvp {
    pub async fn list_for_admin_attendees(db: &Db, event_id: i64) -> Result<Vec<AdminAttendeesRsvp>> {
        Ok(sqlx::query_as!(
            AdminAttendeesRsvp,
            r#"
            SELECT
                u.id AS user_id,
                u.first_name as "first_name!",
                u.last_name as "last_name!",
                u.email,
                CASE
                    WHEN rs.user_id IS NOT NULL AND rs.user_id != r.user_id
                    THEN hu.first_name || ' ' || hu.last_name
                    ELSE NULL
                END AS guest_of,

                sp.name AS spot_name,
                r.contribution,

                FALSE AS "is_manual!: bool",
                r.created_at,

                rs.id AS "session_id!: i64",
                rs.token AS session_token,
                rs.status AS "status!",
                NULL AS "note?: String"
            FROM rsvps r
            JOIN rsvp_sessions rs ON rs.id = r.session_id
            JOIN spots sp ON sp.id = r.spot_id
            JOIN users u  ON u.id  = r.user_id
            JOIN users hu ON hu.id = rs.user_id
            WHERE rs.event_id = ?
              AND rs.status IN ('payment_pending', 'payment_confirmed', 'refund_pending', 'refund_confirmed')

            UNION ALL

            SELECT
                u.id AS user_id,
                u.first_name as "first_name!",
                u.last_name as "last_name!",
                u.email,
                cu.first_name || ' ' || cu.last_name AS guest_of,

                NULL AS spot_name,
                0 AS contribution,

                TRUE AS "is_manual!: bool",
                mr.created_at,

                0 AS "session_id!: i64",
                NULL AS session_token,
                'manual' AS "status!",
                mr.note
            FROM manual_rsvps mr
            JOIN users u ON u.id = mr.user_id
            JOIN users cu ON cu.id = mr.creator_user_id
            WHERE mr.event_id = ?

            ORDER BY 10;
            "#,
            event_id,
            event_id
        )
        .fetch_all(db)
        .await?)
    }
    pub async fn list_for_session(db: &Db, session_id: i64) -> Result<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            r#"SELECT r.id as rsvp_id, r.spot_id, r.contribution
               FROM rsvps r
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               WHERE rs.id = ?
            "#,
            session_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_for_attendees(db: &Db, session_id: i64) -> Result<Vec<AttendeeRsvp>> {
        Ok(sqlx::query_as!(
            AttendeeRsvp,
            r#"SELECT r.id AS rsvp_id, r.user_id, s.name AS spot_name,
                    u.first_name, u.last_name, u.email, u.phone, r.contribution
             FROM rsvps r
             JOIN spots s ON s.id = r.spot_id
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             LEFT JOIN users u ON u.id = r.user_id
             WHERE rs.id = ?
             ORDER BY r.created_at"#,
            session_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_for_contributions(db: &Db, session_id: i64) -> Result<Vec<ContributionRsvp>> {
        Ok(sqlx::query_as!(
            ContributionRsvp,
            r#"SELECT s.name AS spot_name,
                    u.first_name AS "first_name!: String",
                    u.last_name AS "last_name!: String",
                    u.email, u.phone, r.contribution
             FROM rsvps r
             JOIN spots s ON s.id = r.spot_id
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             JOIN users u ON u.id = r.user_id
             WHERE rs.id = ?
             ORDER BY r.created_at"#,
            session_id
        )
        .fetch_all(db)
        .await?)
    }

    /// List attendee info for all RSVPs across a user's active sessions for an event.
    pub async fn list_family_attendees(db: &Db, event: &Event, user_id: i64) -> Result<Vec<AttendeeRsvp>> {
        Ok(sqlx::query_as!(
            AttendeeRsvp,
            r#"SELECT r.id AS rsvp_id, r.user_id, s.name AS spot_name,
                    u.first_name, u.last_name, u.email, u.phone, r.contribution
             FROM rsvps r
             JOIN spots s ON s.id = r.spot_id
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             LEFT JOIN users u ON u.id = r.user_id
             WHERE rs.event_id = ? AND rs.user_id = ?
               AND rs.status IN (?, ?)
             ORDER BY r.created_at"#,
            event.id,
            user_id,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    /// List contribution info for all RSVPs across a user's active sessions for an event.
    pub async fn list_family_contributions(
        db: &Db, event: &Event, user_id: i64,
    ) -> Result<Vec<ContributionRsvp>> {
        Ok(sqlx::query_as!(
            ContributionRsvp,
            r#"SELECT s.name AS spot_name,
                    u.first_name AS "first_name!: String",
                    u.last_name AS "last_name!: String",
                    u.email, u.phone, r.contribution
             FROM rsvps r
             JOIN spots s ON s.id = r.spot_id
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             JOIN users u ON u.id = r.user_id
             WHERE rs.event_id = ? AND rs.user_id = ?
               AND rs.status IN (?, ?)
             ORDER BY r.created_at"#,
            event.id,
            user_id,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    /// List all RSVPs for a user's family (parent + children) for an event.
    pub async fn list_family_rsvps(db: &Db, event: &Event, user_id: i64) -> Result<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT r.id as rsvp_id, r.spot_id, r.contribution
             FROM rsvps r
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             WHERE rs.event_id = ? AND rs.user_id = ?
               AND rs.status IN (?, ?)",
            event.id,
            user_id,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    /// List reserved spots for an event, excluding a specific session.
    /// Only includes rsvps from sessions at CONTRIBUTION status or later.
    pub async fn list_reserved_for_event(
        db: &Db, event: &Event, session: &RsvpSession,
    ) -> Result<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT r.id as rsvp_id, r.spot_id, r.contribution
             FROM rsvps r
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             WHERE rs.event_id = ?
               AND rs.id != ?
               AND rs.status IN (?, ?, ?)",
            event.id,
            session.id,
            RsvpSession::CONTRIBUTION,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    /// List the user's reserved RSVPs for an event across all their sessions (for per-person limits).
    /// Only includes rsvps from sessions at CONTRIBUTION status or later.
    pub async fn list_user_reserved_for_event(
        db: &Db, event: &Event, user_id: Option<i64>,
    ) -> Result<Vec<EventRsvp>> {
        let Some(user_id) = user_id else { return Ok(vec![]) };
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT r.id as rsvp_id, r.spot_id, r.contribution
             FROM rsvps r
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             WHERE rs.event_id = ? AND rs.user_id = ?
               AND rs.status IN (?, ?, ?)",
            event.id,
            user_id,
            RsvpSession::CONTRIBUTION,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    /// List all reserved spots for an event (no exclusion).
    pub async fn list_all_reserved_for_event(db: &Db, event: &Event) -> Result<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT r.id as rsvp_id, r.spot_id, r.contribution
             FROM rsvps r
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             WHERE rs.event_id = ?
               AND rs.status IN (?, ?, ?)",
            event.id,
            RsvpSession::CONTRIBUTION,
            RsvpSession::PAYMENT_PENDING,
            RsvpSession::PAYMENT_CONFIRMED,
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_reserved_users_for_event(
        db: &Db, event: &Event, exclude_session_ids: &[i64],
    ) -> Result<Vec<UserRsvp>> {
        let rows = sqlx::query_as!(
            UserRsvpRow,
            r#"SELECT rs.id AS session_id, rs.status, u.email,
                      u.id AS attendee_user_id,
                      rs.user_id AS owner_user_id,
                      owner.first_name AS owner_first_name,
                      owner.last_name AS owner_last_name
             FROM users u
             JOIN rsvps r ON r.user_id = u.id
             JOIN rsvp_sessions rs ON rs.id = r.session_id
             LEFT JOIN users owner ON owner.id = rs.user_id
             WHERE rs.event_id = ?"#,
            event.id,
        )
        .fetch_all(db)
        .await?;
        Ok(rows
            .into_iter()
            .filter(|r| !exclude_session_ids.contains(&r.session_id))
            .map(|r| UserRsvp {
                status: r.status,
                email: r.email,
                attendee_user_id: r.attendee_user_id,
                owner_user_id: r.owner_user_id,
                owner_first_name: r.owner_first_name,
                owner_last_name: r.owner_last_name,
            })
            .collect())
    }

    pub async fn create(db: &Db, rsvp: CreateRsvp) -> Result<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO rsvps
               (session_id, spot_id, contribution, user_id, user_version)
               VALUES (?, ?, ?, ?, ?)"#,
            rsvp.session_id,
            rsvp.spot_id,
            rsvp.contribution,
            rsvp.user_id,
            rsvp.user_version,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn set_user(db: &Db, rsvp_id: i64, user: &User) -> Result<()> {
        sqlx::query!(
            "UPDATE rsvps
                SET user_id = ?,
                    user_version = ?,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?",
            user.id,
            user.version,
            rsvp_id,
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete_for_session(db: &Db, session_id: i64) -> Result<()> {
        sqlx::query!(r#"DELETE FROM rsvps WHERE session_id = ?"#, session_id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Set check-in for an attendee by event and user ID.
    /// Works for confirmed RSVPs (payment_pending or payment_confirmed).
    pub async fn set_checkin_at_for_event(db: &Db, event_id: i64, user_id: i64) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            r#"UPDATE rsvps SET checkin_at = CURRENT_TIMESTAMP
               WHERE id = (
                   SELECT r.id FROM rsvps r
                   JOIN rsvp_sessions rs ON rs.id = r.session_id
                   WHERE rs.event_id = ? AND r.user_id = ?
                     AND rs.status IN ('payment_pending', 'payment_confirmed')
               )
               RETURNING checkin_at AS 'checkin_at!'"#,
            event_id,
            user_id
        )
        .fetch_one(db)
        .await?;
        Ok(row.checkin_at)
    }

    /// Clear check-in for an attendee by event and user ID.
    pub async fn clear_checkin_at_for_event(db: &Db, event_id: i64, user_id: i64) -> Result<()> {
        sqlx::query!(
            r#"UPDATE rsvps SET checkin_at = NULL
               WHERE id = (
                   SELECT r.id FROM rsvps r
                   JOIN rsvp_sessions rs ON rs.id = r.session_id
                   WHERE rs.event_id = ? AND r.user_id = ?
                     AND rs.status IN ('payment_pending', 'payment_confirmed')
               )"#,
            event_id,
            user_id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Check if a user has an RSVP for an event.
    pub async fn exists_for_event(db: &Db, event_id: i64, user_id: i64) -> Result<bool> {
        let row = sqlx::query!(
            r#"SELECT EXISTS(
                   SELECT 1 FROM rsvps r
                   JOIN rsvp_sessions rs ON rs.id = r.session_id
                   WHERE rs.event_id = ? AND r.user_id = ?
                     AND rs.status IN ('payment_pending', 'payment_confirmed', 'refund_pending', 'refund_confirmed')
               ) as "exists!: bool""#,
            event_id,
            user_id
        )
        .fetch_one(db)
        .await?;
        Ok(row.exists)
    }

    /// Delete an RSVP by event and user ID.
    pub async fn delete_for_event(db: &Db, event_id: i64, user_id: i64) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM rsvps
               WHERE id = (
                   SELECT r.id FROM rsvps r
                   JOIN rsvp_sessions rs ON rs.id = r.session_id
                   WHERE rs.event_id = ? AND r.user_id = ?
                     AND rs.status IN ('payment_pending', 'payment_confirmed')
               )"#,
            event_id,
            user_id
        )
        .execute(db)
        .await?;
        Ok(())
    }
}
