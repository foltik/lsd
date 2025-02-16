use anyhow::Result;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::path::Path;

pub type Db = SqlitePool;

pub mod email;
pub mod event;
pub mod guest;
pub mod list;
pub mod migration;
pub mod post;
pub mod promo;
pub mod role;
pub mod rsvp;
pub mod ticket;
pub mod token;
pub mod user;
pub mod waitlist;

/// Create a new db connection pool, initializing and running migrations if necessary.
pub async fn init(file: &Path) -> Result<Db> {
    let url = format!("sqlite://{}", file.display());
    if !Sqlite::database_exists(&url).await? {
        Sqlite::create_database(&url).await?;
    }
    let db = SqlitePool::connect(&url).await?;

    email::Email::migrate(&db).await?;
    event::Event::migrate(&db).await?;
    guest::Guest::migrate(&db).await?;
    list::List::migrate(&db).await?;
    migration::Migration::migrate(&db).await?;
    post::Post::migrate(&db).await?;
    promo::Promo::migrate(&db).await?;
    role::Role::migrate(&db).await?;
    rsvp::Rsvp::migrate(&db).await?;
    ticket::Ticket::migrate(&db).await?;
    token::LoginToken::migrate(&db).await?;
    token::RegisterToken::migrate(&db).await?;
    token::SessionToken::migrate(&db).await?;
    user::User::migrate(&db).await?;
    waitlist::Waitlist::migrate(&db).await?;

    Ok(db)
}
