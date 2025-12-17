use crate::prelude::*;

/// A "contact us" form submission.
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ContactUs {
    pub id: i64,
    pub name: Option<String>,
    pub reply_to: Option<String>,

    pub subject: String,
    pub message: String,

    pub created_at: NaiveDateTime,
}

pub struct CreateContactUs {
    pub name: Option<String>,
    pub reply_to: Option<String>,

    pub subject: String,
    pub message: String,
}

impl ContactUs {
    pub async fn lookup(db: &Db, id: i64) -> Result<Option<Self>> {
        Ok(sqlx::query_as!(Self, "SELECT * FROM contact_us WHERE id = ?", id)
            .fetch_optional(db)
            .await?)
    }

    /// Create a new session token for a user.
    pub async fn create(db: &Db, form: &CreateContactUs) -> Result<i64> {
        let row = sqlx::query!(
            "INSERT INTO contact_us (name, reply_to, subject, message) VALUES (?, ?, ?, ?)",
            form.name,
            form.reply_to,
            form.subject,
            form.message
        )
        .execute(db)
        .await?;

        Ok(row.last_insert_rowid())
    }
}
