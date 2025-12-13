use crate::prelude::*;

#[derive(Clone, Debug, sqlx::FromRow, serde::Serialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub author: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdatePost {
    pub title: String,
    pub slug: String,
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
    pub async fn create(db: &Db, post: &UpdatePost) -> Result<(i64, NaiveDateTime)> {
        let row = sqlx::query!(
            r#"INSERT INTO posts
               (title, slug, author, content)
               VALUES (?, ?, ?, ?)
               RETURNING id, updated_at"#,
            post.title,
            post.slug,
            post.author,
            post.content,
        )
        .fetch_one(db)
        .await?;
        Ok((row.id, row.updated_at))
    }

    /// Update an existing post.
    pub async fn update(db: &Db, id: i64, post: &UpdatePost) -> Result<(i64, NaiveDateTime)> {
        let row = sqlx::query!(
            r#"UPDATE posts
               SET title = ?,
                 slug = ?,
                 author = ?,
                 content = ?,
                 updated_at = CURRENT_TIMESTAMP
               WHERE id = ?
               RETURNING id, updated_at"#,
            post.title,
            post.slug,
            post.author,
            post.content,
            id
        )
        .fetch_one(db)
        .await?;
        Ok((row.id, row.updated_at))
    }

    /// Delete a post.
    pub async fn delete(db: &Db, id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM posts WHERE id = ?", id).execute(db).await?;
        Ok(())
    }

    /// Lookup a post by URL, if one exists.
    pub async fn lookup_by_slug(db: &Db, slug: &str) -> Result<Option<Post>> {
        let row = sqlx::query_as!(Self, "SELECT * FROM posts WHERE slug = ?", slug)
            .fetch_optional(db)
            .await?;
        Ok(row)
    }
}
