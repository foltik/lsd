use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::Db;
use crate::utils::error::AppResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub created_at: NaiveDateTime,
    pub roles: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUser {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

macro_rules! map_row {
    ($row:ident) => {
        $row.map(|row| User {
            id: row.id,
            first_name: row.first_name,
            last_name: row.last_name,
            email: row.email,
            created_at: row.created_at,
            roles: row.roles.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        })
    };
}

impl User {
    /// Full access to everything.
    pub const ADMIN: &'static str = "admin";
    /// Can manage posts.
    pub const WRITER: &'static str = "writer";

    /// Create a new user.
    pub async fn create(db: &Db, user: &UpdateUser) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO users
               (first_name, last_name, email)
               VALUES (?, ?, ?)"#,
            user.first_name,
            user.last_name,
            user.email
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    // pub async fn add_role(db: &Db, user_id: i64, role: &str) -> AppResult<()> {
    //     sqlx::query!(r#"INSERT INTO user_roles (user_id, role) VALUES (?, ?)"#, user_id, role)
    //         .execute(db)
    //         .await?;
    //     Ok(())
    // }

    /// Lookup a user by email address, if one exists.
    pub async fn lookup_by_email(db: &Db, email: &str) -> AppResult<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            WHERE u.email = ?
            GROUP BY u.id
            "#,
            email
        )
        .fetch_optional(db)
        .await?;
        Ok(map_row!(row))
    }
    /// Lookup a user by a login token, if it's valid.
    pub async fn lookup_by_login_token(db: &Db, token: &str) -> AppResult<Option<User>> {
        // Weird workaround for sqlx incorrectly inferring nullability for joins
        // not sure why this is needed here and not below
        // use the "!" syntax to force the column to be interpreted as non-null
        // https://github.com/launchbadge/sqlx/issues/2127
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            LEFT JOIN login_tokens t ON t.email = u.email
            WHERE t.token = ?
            GROUP BY u.id"#,
            token
        )
        .fetch_optional(db)
        .await?;

        Ok(map_row!(row))
    }
    /// Lookup a user by a session token, if it's valid.
    pub async fn lookup_by_session_token(db: &Db, token: &str) -> AppResult<Option<User>> {
        let row = sqlx::query!(
            r#"
            SELECT
                u.*,
                COALESCE(GROUP_CONCAT(r.role), '') AS "roles!: String"
            FROM users u
            LEFT JOIN user_roles r ON r.user_id = u.id
            LEFT JOIN session_tokens t ON t.user_id = u.id
            WHERE t.token = ?
            GROUP BY u.id
            "#,
            token
        )
        .fetch_optional(db)
        .await?;
        Ok(map_row!(row))
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn has_staff_role(&self) -> bool {
        self.roles.iter().any(|r| [Self::ADMIN, Self::WRITER].contains(&&**r))
    }
}
