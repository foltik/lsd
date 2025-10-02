use image::DynamicImage;
use sqlx::Row;

use crate::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum EventFlyerSize {
    Small,
    Medium,
    Large,
    Full,
}

pub struct EventFlyer;

impl EventFlyer {
    pub const CONTENT_TYPE: &'static str = "image/jpeg";

    pub async fn create_or_update(db: &Db, event_id: i64, image: &DynamicImage) -> AppResult<()> {
        let image_full = crate::utils::image::encode_jpeg(image, None).await;
        let image_lg = crate::utils::image::encode_jpeg(image, Some(1200)).await;
        let image_md = crate::utils::image::encode_jpeg(image, Some(600)).await;
        let image_sm = crate::utils::image::encode_jpeg(image, Some(300)).await;

        sqlx::query!(
            r#"INSERT INTO event_flyers (event_id, image_full, image_lg, image_md, image_sm, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
               ON CONFLICT(event_id) DO UPDATE SET
                 image_full = excluded.image_full,
                 image_lg = excluded.image_lg,
                 image_md = excluded.image_md,
                 image_sm = excluded.image_sm,
                 updated_at = CURRENT_TIMESTAMP"#,
            event_id,
            image_full,
            image_lg,
            image_md,
            image_sm
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn lookup_for_event(
        db: &Db,
        event_id: i64,
        size: EventFlyerSize,
    ) -> AppResult<Option<Vec<u8>>> {
        let column = match size {
            EventFlyerSize::Small => "image_sm",
            EventFlyerSize::Medium => "image_md",
            EventFlyerSize::Large => "image_lg",
            EventFlyerSize::Full => "image_full",
        };

        let query = format!("SELECT {column} FROM event_flyers WHERE event_id = ?");
        let flyer = sqlx::query(&query).bind(event_id).fetch_optional(db).await?;

        Ok(flyer.and_then(|row| row.try_get::<Vec<u8>, _>(0).ok()))
    }

    pub async fn exists_for_event(db: &Db, event_id: i64) -> AppResult<bool> {
        let result = sqlx::query!("SELECT COUNT(*) as count FROM event_flyers WHERE event_id = ?", event_id)
            .fetch_one(db)
            .await?;
        Ok(result.count > 0)
    }
}
