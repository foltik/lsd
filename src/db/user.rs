use crate::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub version: i64,
    pub roles: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct CreateUser {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct UpdateUser {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
}

/// A user matched by the add attendee form autocomplete.
#[derive(serde::Serialize)]
pub struct AttendeeSearchResult {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub rsvped: bool,
}
// Which field the user is searching in.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendeeSearchField {
    FirstName,
    LastName,
    Email,
}

#[macro_export]
macro_rules! map_row {
    ($row:expr) => {
        User {
            id: $row.id,
            email: $row.email,
            first_name: $row.first_name,
            last_name: $row.last_name,
            phone: $row.phone,
            created_at: $row.created_at,
            updated_at: $row.updated_at,

            version: $row.version,
            roles: $row.roles.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        }
    };
}

macro_rules! map_row_fuck {
    ($row:expr) => {
        User {
            id: $row.id.unwrap(),
            email: $row.email.unwrap(),
            first_name: $row.first_name,
            last_name: $row.last_name,
            phone: $row.phone,
            created_at: $row.created_at.unwrap(),
            updated_at: $row.updated_at.unwrap(),

            version: $row.version,
            roles: $row.roles.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        }
    };
}

impl User {
    /// Full access to everything.
    pub const ADMIN: &'static str = "admin";
    /// Can manage posts.
    pub const WRITER: &'static str = "writer";

    pub async fn get_or_create(db: &Db, info: &CreateUser) -> Result<User> {
        Ok(match Self::lookup_by_email(db, &info.email).await? {
            Some(user) => user,
            None => Self::create(db, info).await?,
        })
    }

    pub async fn update_or_create(db: &Db, info: &CreateUser) -> Result<User> {
        Ok(match Self::lookup_by_email(db, &info.email).await? {
            Some(user) => {
                user.update(
                    db,
                    &UpdateUser {
                        email: info.email.clone(),
                        first_name: info.first_name.clone(),
                        last_name: info.last_name.clone(),
                        phone: info.phone.clone(),
                    },
                )
                .await?
            }
            None => Self::create(db, info).await?,
        })
    }

    /// Upsert a user encountered during an RSVP.
    pub async fn upsert_for_rsvp(
        db: &Db, info: &CreateUser, session_created_at: NaiveDateTime,
    ) -> Result<User> {
        // Always allow creating new users
        let Some(existing) = Self::lookup_by_email(db, &info.email).await? else {
            return Self::create(db, info).await;
        };

        // Only overwrite already-populated fields when the user was created in this session
        let allow_overwrite = existing.created_at >= session_created_at;
        let maybe_overwrite = |old: &Option<String>, new: &Option<String>| match old {
            Some(_) if !allow_overwrite => old.clone(),
            _ => new.clone(),
        };

        existing
            .update(
                db,
                &UpdateUser {
                    email: info.email.clone(),
                    first_name: maybe_overwrite(&existing.first_name, &info.first_name),
                    last_name: maybe_overwrite(&existing.last_name, &info.last_name),
                    phone: maybe_overwrite(&existing.phone, &info.phone),
                },
            )
            .await
    }

    /// Create a new user.
    pub async fn create(db: &Db, user: &CreateUser) -> Result<User> {
        let row = sqlx::query!(
            r#"INSERT INTO users
               (email, first_name, last_name, phone)
               VALUES (?, ?, ?, ?)
               RETURNING *, 0 as version, '' as roles
            "#,
            user.email,
            user.first_name,
            user.last_name,
            user.phone
        )
        .fetch_one(db)
        .await?;

        sqlx::query!(
            r#"INSERT INTO user_history (user_id, version, email, first_name, last_name, phone, created_at)
               VALUES (?, 0, ?, ?, ?, ?, ?)"#,
            row.id,
            user.email,
            user.first_name,
            user.last_name,
            user.phone,
            row.created_at
        )
        .execute(db)
        .await?;

        Ok(map_row!(row))
    }

    pub async fn update(&self, db: &Db, info: &UpdateUser) -> Result<Self> {
        let unchanged = info.first_name == self.first_name
            && info.last_name == self.last_name
            && info.phone == self.phone;
        if unchanged {
            return Ok(self.clone());
        }

        let new_version = self.version + 1;

        sqlx::query!(
            r#"INSERT INTO user_history (user_id, version, email, first_name, last_name, phone)
               VALUES (?, ?, ?, ?, ?, ?)"#,
            self.id,
            new_version,
            info.email,
            info.first_name,
            info.last_name,
            info.phone,
        )
        .execute(db)
        .await?;

        sqlx::query!(
            r#"UPDATE users
               SET email = ?,
                   first_name = ?,
                   last_name = ?,
                   phone = ?,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            info.email,
            info.first_name,
            info.last_name,
            info.phone,
            self.id
        )
        .execute(db)
        .await?;

        // Get new version
        Ok(Self::lookup_by_id(db, self.id).await?.unwrap())
    }

    // pub async fn add_role(db: &Db, user_id: i64, role: &str) -> Result<()> {
    //     sqlx::query!(r#"INSERT INTO user_roles (user_id, role) VALUES (?, ?)"#, user_id, role)
    //         .execute(db)
    //         .await?;
    //     Ok(())
    // }

    /// Lookup a user by id, if one exists.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            WHERE u.id = ?
            GROUP BY u.id
            "#,
            id
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row_fuck!(r)))
    }

    /// Lookup a user by email address, if one exists.
    pub async fn lookup_by_email(db: &Db, email: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            WHERE u.email = ? COLLATE NOCASE
            GROUP BY u.id
            "#,
            email
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row_fuck!(r)))
    }

    /// Search users by a substring of one field, flagging those already RSVPed to `event_id`.
    ///
    /// Results are ranked: exact matches first, then prefix matches, then other substring
    /// hits, alphabetically within each tier.
    pub async fn search_for_event(
        db: &Db, event_id: i64, field: AttendeeSearchField, query: &str,
    ) -> Result<Vec<AttendeeSearchResult>> {
        // Note we're only escaping for the LIKE clause, sqlx handles escaping for sql injection.
        let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        let contains = format!("%{escaped}%");
        let prefix = format!("{escaped}%");

        let key = match field {
            AttendeeSearchField::FirstName if query.contains(' ') => "full_name",
            AttendeeSearchField::FirstName => "first_name",
            AttendeeSearchField::LastName => "last_name",
            AttendeeSearchField::Email => "email",
        };

        let rows = sqlx::query_as!(
            AttendeeSearchResult,
            r#"
            SELECT m.first_name, m.last_name, m.email, m.rsvped AS "rsvped!: bool"
            FROM (
                SELECT
                    u.first_name, u.last_name, u.email,
                    (EXISTS(SELECT 1 FROM manual_rsvps mr WHERE mr.event_id = ? AND mr.user_id = u.id)
                     OR EXISTS(SELECT 1 FROM rsvps r JOIN rsvp_sessions rs ON rs.id = r.session_id
                               WHERE rs.event_id = ? AND r.user_id = u.id
                                 AND rs.status IN ('payment_pending','payment_confirmed','refund_pending','refund_confirmed'))
                    ) AS rsvped,
                    (CASE ?
                       WHEN 'first_name' THEN u.first_name
                       WHEN 'last_name'  THEN u.last_name
                       WHEN 'full_name'  THEN TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, ''))
                       ELSE u.email
                     END) AS match_col
                FROM users u
            ) m
            WHERE m.match_col LIKE ? ESCAPE '\' COLLATE NOCASE
            ORDER BY
                CASE
                    WHEN m.match_col = ? COLLATE NOCASE THEN 0
                    WHEN m.match_col LIKE ? ESCAPE '\' COLLATE NOCASE THEN 1
                    ELSE 2
                END,
                m.match_col COLLATE NOCASE
            "#,
            event_id, event_id, key, contains, query, prefix
        )
        .fetch_all(db)
        .await?;
        Ok(rows)
    }

    /// Lookup a user by a login token, if it's valid.
    pub async fn lookup_by_login_token(db: &Db, token: &str) -> Result<Option<User>> {
        // Weird workaround for sqlx incorrectly inferring nullability for joins
        // not sure why this is needed here and not below
        // use the "!" syntax to force the column to be interpreted as non-null
        // https://github.com/launchbadge/sqlx/issues/2127
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN login_tokens t ON t.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE t.token = ?
            GROUP BY u.id"#,
            token
        )
        .fetch_optional(db)
        .await?;

        Ok(row.map(|r| map_row_fuck!(r)))
    }
    /// Lookup a user by a session token, if it's valid.
    pub async fn lookup_by_session_token(db: &Db, token: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN session_tokens t ON t.user_id = u.id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE t.token = ?
            GROUP BY u.id
            "#,
            token
        )
        .fetch_optional(db)
        .await?;
        Ok(row.map(|r| map_row!(r)))
    }

    pub async fn lookup_by_list_id(db: &Db, list_id: i64) -> Result<Vec<User>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(MAX(h.version), 0) as "version!: i64",
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM list_members lm
            JOIN users u ON u.id = lm.user_id
            JOIN user_history h ON h.user_id = u.id
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE lm.list_id = ?
            GROUP BY u.id
            "#,
            list_id,
        )
        .fetch_all(db)
        .await?;
        Ok(rows.into_iter().map(|r| map_row!(r)).collect())
    }

    /// Delete orphaned users, typically from expired anonymous RSVP sessions.
    pub async fn delete_orphaned(db: &Db) -> Result<()> {
        let deleted = sqlx::query!(
            r#"
            DELETE FROM users
            WHERE NOT EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM transactions t WHERE t.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM rsvps r WHERE r.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM rsvp_sessions rs WHERE rs.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM manual_rsvps m WHERE m.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM list_members lm WHERE lm.user_id = users.id)
              AND NOT EXISTS (SELECT 1 FROM emails e WHERE e.user_id = users.id)
            RETURNING email
            "#
        )
        .fetch_all(db)
        .await?;

        sqlx::query!("DELETE FROM user_history WHERE user_id NOT IN (SELECT id FROM users)")
            .execute(db)
            .await?;
        sqlx::query!("DELETE FROM user_attrs WHERE user_id NOT IN (SELECT id FROM users)")
            .execute(db)
            .await?;

        for row in &deleted {
            tracing::info!("Deleted orphaned user: {}", row.email);
        }

        Ok(())
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn has_staff_role(&self) -> bool {
        self.roles.iter().any(|r| [Self::ADMIN, Self::WRITER].contains(&&**r))
    }
}
