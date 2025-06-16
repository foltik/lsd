use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use user::User;

use crate::utils::config::DbConfig;

pub type Db = SqlitePool;

pub mod email;
pub mod event;
pub mod list;
pub mod notification;
pub mod post;
pub mod reservation;
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
        seed_db(&db, seed_data).await?;
    }

    Ok(db)
}

#[derive(Deserialize)]
struct TokenSeed {
    user_id: i64,
    token: String,
}

#[derive(Deserialize)]
struct SeedData {
    users: Vec<user::UpdateUser>,
    user_roles: Vec<user::UserRole>,
    session_tokens: Vec<TokenSeed>,
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

    for session_token in seed_data.session_tokens {
        if sqlx::query!("SELECT * FROM session_tokens WHERE token = ?", session_token.token)
            .fetch_optional(db)
            .await?
            .is_some()
        {
            continue;
        }

        sqlx::query!(
            r#"INSERT INTO session_tokens (user_id, token) VALUES (?, ?)"#,
            session_token.user_id,
            session_token.token
        )
        .execute(db)
        .await?;
    }

    Ok(())
}
