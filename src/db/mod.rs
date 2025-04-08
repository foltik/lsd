use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use user::User;

use crate::utils::config::DbConfig;

pub type Db = SqlitePool;

pub mod email;
pub mod event;
pub mod list;
pub mod post;
pub mod token;
pub mod user;

/// Create a new db connection pool, initializing and running migrations if necessary.
pub async fn init(db_config: &DbConfig) -> anyhow::Result<Db> {
    let url = format!("sqlite://{}", db_config.file.display());
    if !Sqlite::database_exists(&url).await? {
        Sqlite::create_database(&url).await?;
    }
    let db = SqlitePool::connect(&url).await?;

    sqlx::migrate!("./migrations");

    if let Some(seed_data) = &db_config.seed_data {
        seed_db(&db, seed_data).await?;
    }

    Ok(db)
}

#[derive(Deserialize)]
struct SeedData {
    users: Vec<user::UpdateUser>,
    user_roles: Vec<user::UserRole>,
}

impl SeedData {
    pub async fn load(file: &Path) -> anyhow::Result<Self> {
        let contents = tokio::fs::read_to_string(file).await?;
        toml::from_str(&contents).with_context(|| format!("loading config={file:#?}"))
    }
}

async fn seed_db(db: &Db, seed_data_path: &Path) -> anyhow::Result<()> {
    let seed_data = SeedData::load(seed_data_path).await?;

    for user in seed_data.users {
        if User::lookup_by_email(db, &user.email).await?.is_none() {
            User::create(db, &user).await?;
        }
    }

    for user_role in seed_data.user_roles {
        if sqlx::query!(
            "SELECT * FROM user_roles WHERE user_id = ? AND role = ?",
            user_role.user_id,
            user_role.role
        )
        .fetch_optional(db)
        .await?
        .is_none()
        {
            User::add_role(db, user_role.user_id, &user_role.role).await?;
        }
    }

    Ok(())
}
