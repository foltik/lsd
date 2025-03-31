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
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ListMember {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateList {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub emails: String,
}

impl List {
    /// List all lists.
    pub async fn list(db: &Db) -> Result<Vec<List>> {
        let events = sqlx::query_as::<_, List>("SELECT * FROM lists").fetch_all(db).await?;
        Ok(events)
    }

    /// Create a list.
    pub async fn create(db: &Db, event: &UpdateList) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO lists \
                (name, description) \
                VALUES (?, ?)",
        )
        .bind(&event.name)
        .bind(&event.description)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Update a list.
    pub async fn update(db: &Db, id: i64, event: &UpdateList) -> Result<()> {
        sqlx::query(
            "UPDATE lists \
             SET name = ?, description = ? \
             WHERE id = ?",
        )
        .bind(&event.name)
        .bind(&event.description)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Lookup a list by id, if one exists.
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<List>> {
        let event = sqlx::query_as::<_, List>(
            "SELECT * \
             FROM lists \
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;
        Ok(event)
    }

    /// Lookup the members of a list.
    pub async fn list_members(db: &Db, list_id: i64) -> Result<Vec<ListMember>> {
        let members = sqlx::query_as::<_, ListMember>(
            "SELECT e.email, u.first_name, u.last_name
             FROM list_members e \
             LEFT JOIN users u ON u.email = e.email \
             WHERE e.list_id = ?
             ORDER BY e.created_at",
        )
        .bind(list_id)
        .fetch_all(db)
        .await?;
        Ok(members)
    }

    /// Add members to a guest list.
    pub async fn add_members(db: &Db, list_id: i64, emails: &[&str]) -> Result<()> {
        QueryBuilder::new("INSERT INTO list_members (list_id, email) ")
            .push_values(emails, |mut b, email| {
                b.push_bind(list_id).push_bind(email);
            })
            .push("ON CONFLICT DO NOTHING")
            .build()
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn remove_member(db: &Db, list_id: i64, email: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM list_members \
             WHERE list_id = ? AND email = ?",
        )
        .bind(list_id)
        .bind(email)
        .execute(db)
        .await?;
        Ok(())
    }
}
