use sqlx::QueryBuilder;

use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct List {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ListMember {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateList {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub emails: String,
}

#[allow(unused)]
#[derive(serde::Serialize)]
pub struct ListWithCount {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub count: i64,
}

impl List {
    /// List all lists.
    pub async fn list(db: &Db) -> AppResult<Vec<List>> {
        let lists = sqlx::query_as!(Self, "SELECT * FROM lists").fetch_all(db).await?;
        Ok(lists)
    }

    /// List all lists, and count the number of members in each list via list_members join.
    pub async fn list_with_counts(db: &Db) -> AppResult<Vec<ListWithCount>> {
        let lists = sqlx::query_as!(
            ListWithCount,
            "SELECT l.*, COUNT(m.list_id) AS count
             FROM lists l
             LEFT JOIN list_members m ON l.id = m.list_id
             GROUP BY l.id"
        )
        .fetch_all(db)
        .await?;
        Ok(lists)
    }

    /// Create a list.
    pub async fn create(db: &Db, event: &UpdateList) -> AppResult<i64> {
        let res = sqlx::query!(
            r#"INSERT INTO lists
               (name, description)
               VALUES (?, ?)"#,
            event.name,
            event.description
        )
        .execute(db)
        .await?;
        Ok(res.last_insert_rowid())
    }

    /// Update a list.
    pub async fn update(db: &Db, id: i64, event: &UpdateList) -> AppResult<()> {
        sqlx::query!(
            r#"UPDATE lists
               SET name = ?, description = ?
               WHERE id = ?"#,
            event.name,
            event.description,
            id
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Lookup a list by id, if one exists.
    pub async fn lookup_by_id(db: &Db, id: i64) -> AppResult<Option<List>> {
        let list = sqlx::query_as!(
            Self,
            r#"SELECT *
               FROM lists
               WHERE id = ?"#,
            id
        )
        .fetch_optional(db)
        .await?;
        Ok(list)
    }

    /// Lookup the members of a list.
    pub async fn list_members(db: &Db, list_id: i64) -> AppResult<Vec<ListMember>> {
        let members = sqlx::query_as!(
            ListMember,
            r#"SELECT e.email, u.first_name, u.last_name
               FROM list_members e
               LEFT JOIN users u ON u.email = e.email
               WHERE e.list_id = ?
               ORDER BY e.created_at"#,
            list_id
        )
        .fetch_all(db)
        .await?;
        Ok(members)
    }

    /// Add members to a guest list.
    pub async fn add_members(db: &Db, list_id: i64, emails: &[&str]) -> AppResult<()> {
        QueryBuilder::new("INSERT INTO list_members (list_id, email) ")
            .push_values(emails, |mut b, email| {
                b.push_bind(list_id).push_bind(email);
            })
            .push("ON CONFLICT DO NOTHING")
            .build()
            .execute(db)
            .await?;
        Ok(())
    }

    pub async fn remove_member(db: &Db, list_id: i64, email: &str) -> AppResult<()> {
        sqlx::query!(
            r#"DELETE FROM list_members
               WHERE list_id = ? AND email = ?"#,
            list_id,
            email
        )
        .execute(db)
        .await?;
        Ok(())
    }
}
