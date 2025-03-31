use anyhow::Result;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::path::Path;

pub type Db = SqlitePool;

pub mod email;
pub mod event;
pub mod list;
pub mod post;
pub mod token;
pub mod user;

/// Create a new db connection pool, initializing and running migrations if necessary.
pub async fn init(file: &Path) -> Result<Db> {
    let url = format!("sqlite://{}", file.display());
    if !Sqlite::database_exists(&url).await? {
        Sqlite::create_database(&url).await?;
    }
    let db = SqlitePool::connect(&url).await?;

    sqlx::migrate!("./migrations");

    Ok(db)
}
