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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRole {
    pub user_id: i64,
    pub role: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateUser {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
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

    pub async fn add_role(db: &Db, user_id: i64, role: &str) -> AppResult<()> {
        sqlx::query!(r#"INSERT INTO user_roles (user_id, role) VALUES (?, ?)"#, user_id, role)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Lookup a user by email address, if one exists.
    pub async fn lookup_by_email(db: &Db, email: &str) -> AppResult<Option<User>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM users WHERE email = ?", email)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
    /// Lookup a user by a login token, if it's valid.
    pub async fn lookup_by_login_token(db: &Db, token: &str) -> AppResult<Option<User>> {
        // Weird workaround for sqlx incorrectly inferring nullability for joins
        // not sure why this is needed here and not below
        // use the "!" syntax to force the column to be interpreted as non-null
        // https://github.com/launchbadge/sqlx/issues/2127
        let row = sqlx::query_as!(
            User,
            r#"SELECT u.id as "id!", u.first_name as "first_name!", u.last_name as "last_name!", u.email as "email!", u.created_at as "created_at!"
               FROM login_tokens t
               JOIN users u on u.email = t.email
               WHERE token = ?"#,
            token
        )
        .fetch_optional(db)
        .await?;
        Ok(row)
    }
    /// Lookup a user by a session token, if it's valid.
    pub async fn lookup_by_session_token(db: &Db, token: &str) -> AppResult<Option<User>> {
        let user = sqlx::query_as!(
            Self,
            r#"SELECT u.*
               FROM session_tokens t
               JOIN users u on u.id = t.user_id
               WHERE token = ?"#,
            token
        )
        .fetch_optional(db)
        .await?;
        Ok(user)
    }

    pub async fn has_role(&self, db: &Db, role: &str) -> AppResult<bool> {
        let row = sqlx::query!("SELECT * FROM user_roles WHERE user_id = ? AND role = ?", self.id, role)
            .fetch_optional(db)
            .await?;
        Ok(row.is_some())
    }
}
