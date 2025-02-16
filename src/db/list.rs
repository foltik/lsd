use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::QueryBuilder;

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct List {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ListMember {
    pub list_id: i64,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateList {
    pub name: String,
    pub description: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateList {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub emails: String,
}

impl List {
    /// Create the `lists` and `list_members` tables.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS lists ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                name TEXT NOT NULL, \
                description TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS list_members ( \
                list_id INTEGER NOT NULL, \
                user_id INTEGER NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                PRIMARY KEY (list_id, user_id), \
                FOREIGN KEY (list_id) REFERENCES lists(id), \
                FOREIGN KEY (user_id) REFERENCES users(id) \
            )",
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Create a new list.
    pub async fn create(db: &Db, list: &CreateList) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO lists (name, description) \
             VALUES (?, ?)",
        )
        .bind(&list.name)
        .bind(&list.description)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// List all lists.
    pub async fn list(db: &Db) -> Result<Vec<List>> {
        let lists = sqlx::query_as::<_, List>("SELECT * FROM lists ORDER BY name")
            .fetch_all(db)
            .await?;
        Ok(lists)
    }

    /// Lookup a list by id.
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<List>> {
        let list = sqlx::query_as::<_, List>("SELECT * FROM lists WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
        Ok(list)
    }

    /// Lookup a list by id.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<List>> {
        Self::lookup(db, id).await
    }

    /// Update a list.
    pub async fn update(&self, db: &Db, name: &str, description: &str) -> Result<()> {
        sqlx::query(
            "UPDATE lists SET \
                name = ?, \
                description = ?, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(self.id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Get list members.
    pub async fn list_members(db: &Db, list_id: i64) -> Result<Vec<ListMember>> {
        let members = sqlx::query_as::<_, ListMember>(
            "SELECT * FROM list_members \
             WHERE list_id = ? \
             ORDER BY created_at",
        )
        .bind(list_id)
        .fetch_all(db)
        .await?;
        Ok(members)
    }

    /// Add members to a list.
    pub async fn add_members(db: &Db, list_id: i64, user_ids: &[i64]) -> Result<()> {
        if user_ids.is_empty() {
            return Ok(());
        }

        let mut query_builder = QueryBuilder::new(
            "INSERT OR IGNORE INTO list_members (list_id, user_id) ",
        );

        query_builder.push_values(user_ids, |mut b, user_id| {
            b.push_bind(list_id).push_bind(user_id);
        });

        query_builder.build().execute(db).await?;
        Ok(())
    }

    /// Remove a member from a list.
    pub async fn remove_member(db: &Db, list_id: i64, user_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM list_members WHERE list_id = ? AND user_id = ?")
            .bind(list_id)
            .bind(user_id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Check if a user is a member of the list.
    pub async fn is_member(&self, db: &Db, user_id: i64) -> Result<bool> {
        let member = sqlx::query_as::<_, ListMember>(
            "SELECT * FROM list_members \
             WHERE list_id = ? AND user_id = ?",
        )
        .bind(self.id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(member.is_some())
    }
}
