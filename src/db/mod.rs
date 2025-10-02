use anyhow::Context as _;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};

use crate::utils::config::DbConfig;

pub type Db = SqlitePool;

pub mod email;
pub mod event;
pub mod event_flyer;
pub mod list;
pub mod notification;
pub mod post;
pub mod rsvp;
pub mod rsvp_session;
pub mod spot;
pub mod token;
pub mod user;

/// Create a new db connection pool, initializing and running migrations if necessary.
pub async fn init(db_config: &DbConfig) -> anyhow::Result<Db> {
    let url = format!("sqlite://{}", db_config.file.display());
    if !Sqlite::database_exists(&url).await? {
        Sqlite::create_database(&url).await?;
    }
    let db = SqlitePool::connect(&url).await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    if let Some(seed_data) = &db_config.seed_data {
        let sql = tokio::fs::read_to_string(seed_data)
            .await
            .with_context(|| format!("reading config.db.seed_data={seed_data:?}"))?;
        sqlx::raw_sql(&sql).execute(&db).await?;
    }

    Ok(db)
}
