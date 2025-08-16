use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::Db;
use crate::utils::error::AppResult;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
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

#[derive(Debug, serde::Deserialize)]
pub struct UserView {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub is_admin: bool,
    pub is_writer: bool,
}
impl From<User> for UserView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            first_name: user.first_name.unwrap_or_default(),
            last_name: user.last_name.unwrap_or_default(),
            email: user.email,
            is_writer: false,
            is_admin: false,
        }
    }
}
pub struct UserViewList {
    pub users: Vec<UserView>,
    pub has_next_page: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct ListUserQuery {
    pub page: i64,
    pub page_size: i64,
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

    /// Delete a user
    pub async fn remove(db: &Db, user_id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM users WHERE id = ?"#, user_id).execute(db).await?;
        Ok(())
    }

    /// Add role to a user
    pub async fn add_role(db: &Db, user_id: i64, role: &str) -> AppResult<()> {
        sqlx::query!(r#"INSERT INTO user_roles (user_id, role) VALUES (?, ?)"#, user_id, role)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Remove role to a user
    pub async fn remove_role(db: &Db, user_id: i64, role: &str) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM user_roles WHERE user_id=? AND role=?"#, user_id, role)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Look up user by user_id, if one exists.
    pub async fn lookup_by_id(db: &Db, user_id: i64) -> AppResult<Option<User>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM users WHERE id = ?", user_id)
            .fetch_optional(db)
            .await?;
        Ok(row)
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
        let row = sqlx::query!(
            "SELECT * FROM user_roles WHERE user_id = ? AND role = ? OR role = ?",
            self.id,
            role,
            User::ADMIN,
        )
        .fetch_optional(db)
        .await?;
        Ok(row.is_some())
    }

    //Query users based on params
    pub async fn list(db: &Db, query: &ListUserQuery) -> AppResult<UserViewList> {
        let current_page = (query.page - 1) * query.page_size;
        let next_page_check = query.page_size + 1;
        let mut users = sqlx::query_as!(
            User,
            r#"SELECT u.*
               FROM users u
               LIMIT ?
               OFFSET ?"#,
            next_page_check,
            current_page,
        )
        .fetch_all(db)
        .await?;

        let has_next_page = users.len() > query.page_size as usize;
        users = if has_next_page { users[0..query.page_size as usize].to_vec() } else { users };

        let user_ids: Vec<i64> = users.iter().map(|u| u.id).collect();

        let user_roles: Vec<(i64, String)> = if !user_ids.is_empty() {
            let placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query_str =
                format!("SELECT user_id, role FROM user_roles WHERE user_id IN ({})", placeholders);
            let mut query = sqlx::query_as(&query_str);
            for user_id in &user_ids {
                query = query.bind(user_id);
            }
            query.fetch_all(db).await?
        } else {
            Vec::new()
        };

        let user_view_list: Vec<UserView> = users
            .into_iter()
            .map(|user| {
                let user_roles_for_this_user: Vec<String> = user_roles
                    .iter()
                    .filter(|(user_id, _)| *user_id == user.id)
                    .map(|(_, role)| role.clone())
                    .collect();
                UserView {
                    id: user.id,
                    first_name: user.first_name.unwrap_or_default(),
                    last_name: user.last_name.unwrap_or_default(),
                    email: user.email,
                    is_writer: user_roles_for_this_user.contains(&"writer".to_string()),
                    is_admin: user_roles_for_this_user.contains(&"admin".to_string()),
                }
            })
            .collect();

        Ok(UserViewList { users: user_view_list, has_next_page })
    }
}
