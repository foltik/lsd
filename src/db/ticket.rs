#![allow(unused)]

use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Ticket {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(serde::Deserialize)]
pub struct UpdateTicket {
    pub name: String,
    pub description: String,
}

impl Ticket {
    /// List all tickets.
    pub async fn list(db: &Db) -> AppResult<Vec<Ticket>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM tickets"#).fetch_all(db).await?)
    }

    /// Create a new ticket.
    pub async fn create(db: &Db, ticket: &UpdateTicket) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO tickets (name, description)
               VALUES (?, ?)"#,
            ticket.name,
            ticket.description,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Update an existing ticket.
    pub async fn update(db: &Db, id: i64, ticket: &UpdateTicket) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE tickets
               SET name = ?, description = ?
               WHERE id = ?"#,
            ticket.name,
            ticket.description,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Remove a ticket.
    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM tickets WHERE id = ?"#, id).execute(db).await?;
        Ok(())
    }

    /// Retrieve a ticket (if any) by its id.
    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Ticket>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM tickets WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }
}
