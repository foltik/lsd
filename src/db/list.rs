use crate::db::user::CreateUser;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct List {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(serde::Deserialize)]
pub struct UpdateList {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub emails: String,
}

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
    pub async fn list(db: &Db) -> Result<Vec<List>> {
        let lists = sqlx::query_as!(Self, "SELECT * FROM lists").fetch_all(db).await?;
        Ok(lists)
    }

    /// List all lists, and count the number of members in each list via list_members join.
    pub async fn list_with_counts(db: &Db) -> Result<Vec<ListWithCount>> {
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
    pub async fn create(db: &Db, event: &UpdateList) -> Result<i64> {
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
    pub async fn update(db: &Db, id: i64, event: &UpdateList) -> Result<()> {
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
    pub async fn lookup_by_id(db: &Db, id: i64) -> Result<Option<List>> {
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
    pub async fn list_members(db: &Db, list_id: i64) -> Result<Vec<User>> {
        User::lookup_by_list_id(db, list_id).await
    }

    pub async fn has_user_id(db: &Db, id: i64, user_id: i64) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM list_members
                WHERE list_id = ? AND user_id = ?
            ) AS "exists!: bool"
            "#,
            id,
            user_id,
        )
        .fetch_one(db)
        .await?;
        Ok(exists)
    }

    pub async fn has_email(db: &Db, id: i64, email: &str) -> Result<bool> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM list_members lm
                LEFT JOIN users u ON u.id = lm.user_id
                WHERE lm.list_id = ?
                  AND u.email = ?
            ) AS "exists!: bool"
            "#,
            id,
            email,
        )
        .fetch_one(db)
        .await?;
        Ok(exists)
    }

    /// Add members to a guest list.
    pub async fn add_members(db: &Db, list_id: i64, emails: &[&str]) -> Result<()> {
        // We could technically optimize this, but the common case is 1 signup.
        for email in emails {
            let user = User::get_or_create(
                db,
                &CreateUser { email: email.to_string(), first_name: None, last_name: None, phone: None },
            )
            .await?;

            sqlx::query!(
                "INSERT OR IGNORE INTO list_members (list_id, user_id) VALUES (?, ?)",
                list_id,
                user.id,
            )
            .execute(db)
            .await?;
        }
        Ok(())
    }

    pub async fn remove_member(db: &Db, list_id: i64, user_id: i64) -> Result<()> {
        sqlx::query!(
            r#"DELETE FROM list_members
               WHERE list_id = ? AND user_id = ?"#,
            list_id,
            user_id
        )
        .execute(db)
        .await?;
        Ok(())
    }
}
