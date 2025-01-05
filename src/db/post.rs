use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(serde::Deserialize)]
pub struct UpdatePost {
    pub title: String,
    pub url: String,
    pub author: String,
    pub content: String,
}

impl Post {
    /// Create the `posts` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS posts ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                title TEXT NOT NULL, \
                url TEXT NOT NULL, \
                author TEXT NOT NULL, \
                content TEXT NOT NULL, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new post.
    pub async fn create(db: &Db, post: &UpdatePost) -> Result<i64> {
        let row = sqlx::query("INSERT INTO posts (title, url, author, content) VALUES (?, ?, ?, ?)")
            .bind(&post.title)
            .bind(&post.url)
            .bind(&post.author)
            .bind(&post.content)
            .execute(db)
            .await?;
        Ok(row.last_insert_rowid())
    }

    /// Update an existing post.
    pub async fn update(db: &Db, id: i64, post: &UpdatePost) -> Result<()> {
        sqlx::query(
            "UPDATE posts
             SET title = ?,
                 url = ?,
                 author = ?,
                 content = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(&post.title)
        .bind(&post.url)
        .bind(&post.author)
        .bind(&post.content)
        .bind(id)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Lookup a post by URL, if one exists.
    pub async fn lookup_by_url(db: &Db, url: &str) -> Result<Option<Post>> {
        let row = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE url = ?")
            .bind(url)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
}
