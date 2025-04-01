use anyhow::Result;
use chrono::NaiveDateTime;

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdatePost {
    pub title: String,
    pub url: String,
    pub author: String,
    pub content: String,
}

impl Post {
    // List all posts.
    pub async fn list(db: &Db) -> Result<Vec<Post>> {
        let posts = sqlx::query_as!(Self, "SELECT * FROM posts ORDER BY updated_at DESC")
            .fetch_all(db)
            .await?;
        Ok(posts)
    }

    /// Create a new post.
    pub async fn create(db: &Db, post: &UpdatePost) -> Result<i64> {
        let row = sqlx::query!(
            r#"INSERT INTO posts
               (title, url, author, content)
               VALUES (?, ?, ?, ?)"#,
            post.title,
            post.url,
            post.author,
            post.content,
        )
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Update an existing post.
    pub async fn update(db: &Db, id: i64, post: &UpdatePost) -> Result<()> {
        sqlx::query!(
            r#"UPDATE posts
               SET title = ?,
                 url = ?,
                 author = ?,
                 content = ?,
                 updated_at = CURRENT_TIMESTAMP
               WHERE id = ?"#,
            post.title,
            post.url,
            post.author,
            post.content,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Delete a post.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM posts WHERE id = ?", id).execute(db).await?;
        Ok(())
    }

    /// Lookup a post by URL, if one exists.
    pub async fn lookup_by_url(db: &Db, url: &str) -> Result<Option<Post>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM posts WHERE url = ?", url)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
}
