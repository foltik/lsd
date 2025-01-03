use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Local};
use lettre::message::Mailbox;
use rand::{rngs::OsRng, Rng as _};
use sqlx::{migrate::MigrateDatabase, sqlite::SqliteQueryResult, Error, Sqlite, SqlitePool};

// +--------------------------------------------------------------------------------+
// | TODO: Separate the individual types into a `models/` module to reduce clutter. |
// +--------------------------------------------------------------------------------+

/// Database client.
#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub created_at: String,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Event {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub description: String,
    pub start_date: DateTime<Local>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub author: String,
    pub body: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl Db {
    pub async fn connect(file: &Path) -> Result<Self> {
        let url = format!("sqlite://{}", file.display());
        if !Sqlite::database_exists(&url).await? {
            Sqlite::create_database(&url).await?;
        }
        let pool = SqlitePool::connect(&url).await?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                first_name TEXT NOT NULL, \
                last_name TEXT NOT NULL, \
                email TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS login_tokens ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                email TEXT NOT NULL, \
                token TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session_tokens ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                user_id INTEGER NOT NULL, \
                token TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                FOREIGN KEY (user_id) REFERENCES users(id) \
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                title TEXT NOT NULL, \
                artist TEXT NOT NULL, \
                description TEXT NOT NULL, \
                start_date TIMESTAMP NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS posts ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                title TEXT NOT NULL, \
                slug TEXT NOT NULL, \
                author TEXT NOT NULL, \
                body TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_user(&self, first_name: &str, last_name: &str, email: &str) -> Result<i64> {
        let row = sqlx::query("INSERT INTO users (first_name, last_name, email) VALUES (?, ?, ?)")
            .bind(first_name)
            .bind(last_name)
            .bind(email)
            .execute(&self.pool)
            .await?;
        Ok(row.last_insert_rowid())
    }
    pub async fn lookup_user_by_email(&self, email: &Mailbox) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email.email.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }
    pub async fn lookup_user_by_login_token(&self, token: &str) -> Result<Option<User>> {
        let row = sqlx::query_as::<_, User>(
            "SELECT u.* \
             FROM login_tokens t \
             LEFT JOIN users u on u.email = t.email \
             WHERE t.token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
    pub async fn lookup_user_from_session_token(&self, token: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT u.* \
             FROM session_tokens t \
             JOIN users u on u.id = t.user_id \
             WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn create_session_token(&self, user_id: i64) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query("INSERT INTO session_tokens (user_id, token) VALUES (?, ?)")
            .bind(user_id)
            .bind(&token)
            .execute(&self.pool)
            .await?;

        Ok(token)
    }
    pub async fn create_login_token(&self, email: &Mailbox) -> Result<String> {
        let token = format!("{:08x}", OsRng.gen::<u64>());

        sqlx::query("INSERT INTO login_tokens (email, token) VALUES (?, ?)")
            .bind(email.email.to_string())
            .bind(&token)
            .execute(&self.pool)
            .await?;

        Ok(token)
    }
    pub async fn lookup_email_by_login_token(&self, token: &str) -> Result<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>("SELECT email FROM login_tokens WHERE token = ?")
            .bind(token)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.0))
    }
    // Lookup Event by id
    pub async fn lookup_event_by_event_id(&self, id: &i64) -> Result<Option<Event>> {
        let event = sqlx::query_as::<_, Event>(
            "SELECT e.* \
            FROM events e \
            WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(event)
    }
    // Get all Events
    pub async fn get_all_events(&self, date: DateTime<Local>, past: bool) -> Result<Vec<Event>, Error> {
        let events = if !past {
            sqlx::query_as::<_, Event>("SELECT e.* FROM events e WHERE start_date >= ?")
                .bind(date)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as::<_, Event>("SELECT e.* FROM events e WHERE start_date < ?")
                .bind(date)
                .fetch_all(&self.pool)
                .await?
        };
        Ok(events)
    }
    // Create Event
    pub async fn create_event(
        &self,
        title: &str,
        artist: &str,
        description: &str,
        start_date: &str,
    ) -> Result<i64> {
        let row =
            sqlx::query("INSERT INTO events (title, artist, description, start_date) VALUES (?, ?, ?, ?)")
                .bind(title)
                .bind(artist)
                .bind(description)
                .bind(start_date)
                .execute(&self.pool)
                .await?;
        Ok(row.last_insert_rowid())
    }
    // Update Event
    pub async fn update_event(
        &self,
        id: i64,
        title: &str,
        artist: &str,
        description: &str,
        start_date: &str,
    ) -> Result<SqliteQueryResult, Error> {
        sqlx::query(
            "UPDATE events
            SET title = ?, artist = ?, description = ?, start_date = ?
            WHERE id = ?",
        )
        .bind(title)
        .bind(artist)
        .bind(description)
        .bind(start_date.to_string())
        .bind(id)
        .execute(&self.pool)
        .await
    }
    // Remove Event
    pub async fn delete_event(&self, id: i64) -> Result<SqliteQueryResult, Error> {
        sqlx::query("DELETE FROM events WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
    }

    pub async fn create_post(&self, title: &str, slug: &str, author: &str, body: &str) -> Result<i64> {
        let row = sqlx::query("INSERT INTO posts (title, slug, author, body) VALUES (?, ?, ?, ?)")
            .bind(title)
            .bind(slug)
            .bind(author)
            .bind(body)
            .execute(&self.pool)
            .await?;
        Ok(row.last_insert_rowid())
    }
    pub async fn lookup_post_by_slug(&self, slug: &str) -> Result<Option<Post>> {
        let row = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }
}
