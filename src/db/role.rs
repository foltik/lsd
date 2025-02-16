use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Role {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct UserRole {
    pub user_id: i64,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateRole {
    pub name: String,
}

impl Role {
    /// Create the `roles` and `user_roles` tables.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS roles ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                name TEXT NOT NULL UNIQUE \
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_roles ( \
                user_id INTEGER NOT NULL, \
                role TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                PRIMARY KEY (user_id, role), \
                FOREIGN KEY (user_id) REFERENCES users(id) \
            )",
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Create a new role.
    pub async fn create(db: &Db, role: &CreateRole) -> Result<i64> {
        let row = sqlx::query("INSERT INTO roles (name) VALUES (?)")
            .bind(&role.name)
            .execute(db)
            .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a role by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Role>> {
        let role = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(role)
    }

    /// Lookup a role by name.
    pub async fn lookup_by_name(db: &Db, name: &str) -> Result<Option<Role>> {
        let role = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE name = ?")
            .bind(name)
            .fetch_optional(db)
            .await?;
        Ok(role)
    }

    /// Get all roles.
    pub async fn list(db: &Db) -> Result<Vec<Role>> {
        let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name")
            .fetch_all(db)
            .await?;
        Ok(roles)
    }

    /// Assign a role to a user.
    pub async fn assign_to_user(db: &Db, user_id: i64, role: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO user_roles (user_id, role) \
             VALUES (?, ?)",
        )
        .bind(user_id)
        .bind(role)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Remove a role from a user.
    pub async fn remove_from_user(db: &Db, user_id: i64, role: &str) -> Result<()> {
        sqlx::query("DELETE FROM user_roles WHERE user_id = ? AND role = ?")
            .bind(user_id)
            .bind(role)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Get all roles for a user.
    pub async fn list_for_user(db: &Db, user_id: i64) -> Result<Vec<UserRole>> {
        let roles = sqlx::query_as::<_, UserRole>(
            "SELECT * FROM user_roles \
             WHERE user_id = ? \
             ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(db)
        .await?;
        Ok(roles)
    }
} 