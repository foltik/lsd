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

#[derive(serde::Deserialize)]
pub struct UpdateRsvp {
    pub user_id: Option<i64>,
    pub user_version: Option<i64>,
    pub checkin_at: Option<NaiveDateTime>,
}

#[derive(serde::Serialize)]
pub struct SelectionRsvp {
    pub spot_id: i64,
    pub contribution: i64,
}

#[derive(serde::Serialize)]
pub struct AttendeeRsvp {
    pub rsvp_id: i64,
    pub spot_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
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

pub struct EventRsvp {
    pub spot_id: i64,
    pub contribution: i64,
}

impl Rsvp {
    pub async fn list_for_selection(db: &Db, session_id: i64) -> AppResult<Vec<SelectionRsvp>> {
        Ok(sqlx::query_as!(
            SelectionRsvp,
            r#"SELECT r.spot_id, r.contribution
               FROM rsvps r
               JOIN rsvp_sessions rs ON rs.id = r.session_id
               WHERE rs.id = ?
            "#,
            session_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn list_for_attendees(db: &Db, session_id: i64) -> AppResult<Vec<AttendeeRsvp>> {
        Ok(sqlx::query_as!(
            AttendeeRsvp,
            r#"SELECT
                r.id AS rsvp_id,
                s.name AS spot_name,
                u.first_name,
                u.last_name,
                u.email,
                u.phone
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

    pub async fn list_for_contributions(db: &Db, session_id: i64) -> AppResult<Vec<ContributionRsvp>> {
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
    pub async fn list_for_event_excluding_session(
        db: &Db, event_id: i64, session_id: i64,
    ) -> AppResult<Vec<EventRsvp>> {
        Ok(sqlx::query_as!(
            EventRsvp,
            "SELECT spot_id, contribution
             FROM rsvps r
             JOIN rsvp_sessions s ON s.id = r.session_id
             WHERE s.event_id = ? AND s.id != ?",
            event_id,
            session_id,
        )
        .fetch_all(db)
        .await?)
    }

    // pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Rsvp>> {
    //     Ok(sqlx::query_as!(Self, r#"SELECT * FROM rsvps WHERE id = ?"#, id)
    //         .fetch_optional(db)
    //         .await?)
    // }

    pub async fn create(db: &Db, rsvp: CreateRsvp) -> AppResult<i64> {
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

    pub async fn set_user(db: &Db, rsvp_id: i64, user: &User) -> AppResult<()> {
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

    pub async fn update(db: &Db, id: i64, rsvp: UpdateRsvp) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE rsvps
               SET user_id = ?,
                   user_version = ?,
                   checkin_at = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            rsvp.user_id,
            rsvp.user_version,
            rsvp.checkin_at,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn lookup_conflicts(
        db: &Db, session: &RsvpSession, event: &Event, email: &str,
    ) -> AppResult<Option<String>> {
        let row = sqlx::query!(
            "SELECT s.status
             FROM rsvps r
             JOIN rsvp_sessions s ON s.id = r.session_id
             LEFT JOIN users u ON u.id = s.user_id
             WHERE s.id != ?
               AND s.event_id = ?
               AND u.email = ?",
            session.id,
            event.id,
            email
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| r.status))
    }

    pub async fn delete_for_session(db: &Db, session_id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM rsvps WHERE session_id = ?"#, session_id)
            .execute(db)
            .await?;
        Ok(())
    }
}
