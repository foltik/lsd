use anyhow::Result;
use chrono::{DateTime, Utc};

use super::Db;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Promo {
    pub id: i64,
    pub code: String,
    pub user_id: Option<i64>,
    pub amount: i64,
    pub uses_remaining: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreatePromoCode {
    pub code: String,
    pub user_id: Option<i64>,
    pub amount: i64,
    pub uses_remaining: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Promo {
    /// Create the `promos` table.
    pub async fn migrate(db: &Db) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS promos ( \
                id INTEGER PRIMARY KEY NOT NULL, \
                code TEXT NOT NULL UNIQUE, \
                user_id INTEGER, \
                amount INTEGER NOT NULL, \
                uses_remaining INTEGER, \
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
                expires_at TIMESTAMP, \
                FOREIGN KEY (user_id) REFERENCES users(id) \
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    /// Create a new promo code.
    pub async fn create(db: &Db, promo: &CreatePromoCode) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO promos (code, user_id, amount, uses_remaining, expires_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&promo.code)
        .bind(promo.user_id)
        .bind(promo.amount)
        .bind(promo.uses_remaining)
        .bind(promo.expires_at)
        .execute(db)
        .await?;
        Ok(row.last_insert_rowid())
    }

    /// Lookup a promo code by code.
    pub async fn lookup_by_code(db: &Db, code: &str) -> Result<Option<Promo>> {
        let promo = sqlx::query_as::<_, Promo>("SELECT * FROM promos WHERE code = ?")
            .bind(code)
            .fetch_optional(db)
            .await?;
        Ok(promo)
    }

    /// Check if a promo code is valid.
    pub async fn is_valid(&self) -> bool {
        // Check if expired
        if let Some(expires_at) = self.expires_at {
            if expires_at < Utc::now() {
                return false;
            }
        }

        // Check if uses remaining
        if let Some(uses_remaining) = self.uses_remaining {
            if uses_remaining <= 0 {
                return false;
            }
        }

        true
    }

    /// Decrement uses remaining.
    pub async fn use_code(&self, db: &Db) -> Result<()> {
        if let Some(uses_remaining) = self.uses_remaining {
            sqlx::query(
                "UPDATE promos SET uses_remaining = ? \
                 WHERE id = ? AND uses_remaining > 0",
            )
            .bind(uses_remaining - 1)
            .bind(self.id)
            .execute(db)
            .await?;
        }
        Ok(())
    }
} 