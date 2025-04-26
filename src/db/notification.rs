#![allow(unused)]

use chrono::NaiveDateTime;

use super::Db;
use crate::utils::error::AppResult;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Notification {
    pub id: i64,
    /// Set when this notification is specific to an individual event, rather
    /// than a generically applicable notification for a certain type of event.
    pub event_id: Option<i64>,

    pub name: String,
    pub content: String,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdateNotification {
    pub event_id: Option<i64>,
    pub name: String,
    pub content: String,
}

impl Notification {
    pub async fn list(db: &Db) -> AppResult<Vec<Notification>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM notifications"#).fetch_all(db).await?)
    }

    pub async fn create(db: &Db, n: &UpdateNotification) -> AppResult<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO notifications (event_id, name, content)
               VALUES (?, ?, ?)"#,
            n.event_id,
            n.name,
            n.content,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    pub async fn update(db: &Db, id: i64, n: &UpdateNotification) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE notifications
               SET event_id = ?, name = ?, content = ?
               WHERE id = ?"#,
            n.event_id,
            n.name,
            n.content,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn delete(db: &Db, id: i64) -> AppResult<()> {
        sqlx::query!(r#"DELETE FROM notifications WHERE id = ?"#, id)
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<Notification>> {
        Ok(sqlx::query_as!(Self, r#"SELECT * FROM notifications WHERE id = ?"#, id)
            .fetch_optional(db)
            .await?)
    }
}
