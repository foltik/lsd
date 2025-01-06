use anyhow::Result;
use std::future::Future;

use super::Db;

/// A record of a database migration
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Migration {
    id: i64,
    name: String,
}

impl Migration {
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS migrations ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                name TEXT NOT NULL \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn run(db: &Db, name: &str, func: impl Future<Output = Result<()>>) -> Result<()> {
        let id = sqlx::query("SELECT id FROM migrations WHERE name = ?")
            .bind(name)
            .fetch_optional(db)
            .await?;

        if id.is_none() {
            tracing::info!("Running migration {name:?}");
            func.await?;
            sqlx::query("INSERT INTO migrations (name) VALUES (?)")
                .bind(name)
                .execute(db)
                .await?;
        }

        Ok(())
    }
}
