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
}

pub struct AdminAttendeesRsvp {
    pub rsvp_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub guest_of: Option<String>,

    pub spot_name: String,
    pub contribution: i64,

    pub created_at: NaiveDateTime,
    pub checkin_at: Option<NaiveDateTime>,
}

impl Rsvp {
    pub async fn list_for_admin_attendees(db: &Db, event_id: i64) -> Result<Vec<AdminAttendeesRsvp>> {
        Ok(sqlx::query_as!(
            AdminAttendeesRsvp,
            r#"
            SELECT
                r.id AS rsvp_id,
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

                r.created_at,
                r.checkin_at
            FROM rsvps r
            JOIN rsvp_sessions rs ON rs.id = r.session_id
            JOIN spots sp ON sp.id = r.spot_id
            JOIN users u  ON u.id  = r.user_id
            JOIN users hu ON hu.id = rs.user_id

            WHERE rs.event_id = ?
            ORDER BY u.last_name;
            "#,
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
            r#"SELECT
                r.id AS rsvp_id,
                r.user_id,
                s.name AS spot_name,
                u.first_name,
                u.last_name,
                u.email,
                u.phone,
                r.contribution
               FROM rsvps r
               JOIN spots s ON s.id = r.spot_id
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               LEFT JOIN users u ON u.id = r.user_id
               WHERE rs.id = ?
            "#,
            session_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_for_contributions(db: &Db, session_id: i64) -> Result<Vec<ContributionRsvp>> {
        Ok(sqlx::query_as!(
            ContributionRsvp,
            r#"SELECT
                s.name AS spot_name,
                u.first_name AS "first_name!: String",
                u.last_name AS "last_name!: String",
                u.email,
                u.phone,
                r.contribution
               FROM rsvps r
               JOIN spots s ON s.id = r.spot_id
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               JOIN users u ON u.id = r.user_id
               WHERE rs.id = ?
            "#,
            session_id
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

    pub async fn list_reserved_users_for_event(
        db: &Db, event: &Event, session: Option<&RsvpSession>,
    ) -> Result<Vec<UserRsvp>> {
        let session_id = session.map(|s| s.id).unwrap_or(0);
        Ok(sqlx::query_as!(
            UserRsvp,
            r#"SELECT rs.status, u.email
               FROM users u
               JOIN rsvps r ON r.user_id = u.id
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               WHERE rs.event_id = ?
                 AND rs.id != ?
            "#,
            event.id,
            session_id,
        )
        .fetch_all(db)
        .await?)
    }

    // pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<Rsvp>> {
    //     Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvps WHERE id = ?"#, id)
    //         .fetch_optional(db)
    //         .await?)
    // }

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

    pub async fn set_checkin_at(db: &Db, rsvp_id: i64) -> Result<NaiveDateTime> {
        let row = sqlx::query!(
            "UPDATE rsvps SET checkin_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING checkin_at AS 'checkin_at!'",
            rsvp_id
        )
        .fetch_one(db)
        .await?;
        Ok(row.checkin_at)
    }

    pub async fn clear_checkin_at(db: &Db, rsvp_id: i64) -> Result<()> {
        sqlx::query!("UPDATE rsvps SET checkin_at = NULL WHERE id = ?", rsvp_id)
            .execute(db)
            .await?;
        Ok(())
    }
}
