use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use base64::Engine;
use base64::prelude::BASE64_STANDARD as BASE64;
use bytes::Bytes;
use dashmap::DashMap;
use image::DynamicImage;
use sqlx::Row;

use crate::prelude::*;

static CACHE: LazyLock<EventFlyerCache> = LazyLock::new(EventFlyerCache::default);

pub struct EventFlyer {
    pub width: i64,
    pub height: i64,
    pub thumb_base64: String,
    pub version: i64,
}

pub struct GalleryEventFlyer {
    pub slug: String,
    pub title: String,
    pub unlisted: bool,
    pub width: i64,
    pub height: i64,
}

impl EventFlyer {
    pub const CONTENT_TYPE: &'static str = "image/jpeg";

    pub async fn create_or_update(
        db: &Db, event_id: i64, event_slug: &str, image: &DynamicImage,
    ) -> Result<()> {
        let width = image.width();
        let height = image.height();

        let image_full = crate::utils::image::encode_jpeg(image, None).await;
        let image_lg = crate::utils::image::encode_jpeg(image, Some(1200)).await;
        let image_md = crate::utils::image::encode_jpeg(image, Some(600)).await;
        let image_sm = crate::utils::image::encode_jpeg(image, Some(300)).await;
        let image_thumb = crate::utils::image::encode_jpeg(image, Some(60)).await;

        sqlx::query!(
            r#"INSERT INTO event_flyers  (event_id, width, height, image_full, image_lg, image_md, image_sm, image_thumb, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
               ON CONFLICT(event_id) DO UPDATE SET
                width = excluded.width,
                height = excluded.height,
                image_full = excluded.image_full,
                image_lg = excluded.image_lg,
                image_md = excluded.image_md,
                image_sm = excluded.image_sm,
                image_thumb = excluded.image_thumb,
                updated_at = CURRENT_TIMESTAMP"#,
            event_id,
            width,
            height,
            image_full,
            image_lg,
            image_md,
            image_sm,
            image_thumb
        )
        .execute(db)
        .await?;

        CACHE.insert(event_slug, "image_thumb", image_thumb);
        CACHE.insert(event_slug, "image_sm", image_sm);

        Ok(())
    }

    pub async fn delete(db: &Db, event_id: i64, event_slug: &str) -> Result<()> {
        sqlx::query!("DELETE FROM event_flyers WHERE event_id = ?", event_id)
            .execute(db)
            .await?;
        CACHE.remove(event_slug, "image_thumb");
        CACHE.remove(event_slug, "image_sm");
        Ok(())
    }

    // Flyer tiles newest first; `unlisted` includes hidden events (for logged-in viewers).
    pub async fn list_gallery(db: &Db, unlisted: bool) -> Result<Vec<GalleryEventFlyer>> {
        Ok(sqlx::query_as!(
            GalleryEventFlyer,
            r#"SELECT e.slug, e.title, e.unlisted, f.width, f.height
               FROM event_flyers f JOIN events e ON e.id = f.event_id
               WHERE ?1 OR NOT e.unlisted
               ORDER BY e.start DESC"#,
            unlisted
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn lookup(db: &Db, event_id: i64) -> Result<Option<EventFlyer>> {
        let row = sqlx::query!(
            r#"SELECT width, height, image_thumb, strftime('%s', updated_at) as "version!: i64"
               FROM event_flyers WHERE event_id = ?"#,
            event_id
        )
        .fetch_optional(db)
        .await?;

        Ok(row.map(|r| EventFlyer {
            width: r.width,
            height: r.height,
            thumb_base64: format!("data:image/jpeg;base64,{}", BASE64.encode(&r.image_thumb)),
            version: r.version,
        }))
    }

    // Serve a flyer image by slug.
    pub async fn serve(db: &Db, slug: &str, size: Option<&String>) -> HtmlResult {
        let size = match size.map(|s| s.as_str()) {
            Some("thumb") => "image_thumb",
            Some("sm") => "image_sm",
            Some("md") => "image_md",
            Some("lg") => "image_lg",
            Some(_) => bail_invalid!(),
            None => "image_full",
        };

        let bytes = match CACHE.get(slug, size) {
            Some(bytes) => bytes,
            None => {
                let query = format!(
                    "SELECT f.{size} FROM event_flyers f JOIN events e ON e.id = f.event_id WHERE e.slug = ?"
                );
                match sqlx::query(&query).bind(slug).fetch_optional(db).await? {
                    Some(row) => Bytes::from(row.try_get::<Vec<u8>, _>(0)?),
                    None => bail_not_found!(),
                }
            }
        };

        Ok((
            [
                (header::CONTENT_TYPE, EventFlyer::CONTENT_TYPE),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                (HeaderName::from_static("priority"), "u=1"),
            ],
            bytes,
        )
            .into_response())
    }

    /// Returns a counter that increments any time a flyer is added, modified, or removed.
    pub fn cache_generation() -> u64 {
        CACHE.generation()
    }

    pub fn populate_cache(db: &Db) {
        tokio::spawn({
            let db = db.clone();
            async move {
                let _ = CACHE.populate(&db).await;
            }
        });
    }

    pub async fn exists_for_event(db: &Db, event_id: i64) -> Result<bool> {
        let result = sqlx::query!("SELECT COUNT(*) as count FROM event_flyers WHERE event_id = ?", event_id)
            .fetch_one(db)
            .await?;
        Ok(result.count > 0)
    }

    /// Duplicate a flyer from one event to another.
    /// Does nothing if the source event has no flyer.
    pub async fn duplicate(
        db: &Db, old_event_id: i64, old_slug: &str, new_event_id: i64, new_slug: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO event_flyers (event_id, width, height, image_full, image_lg, image_md, image_sm, image_thumb, updated_at)
               SELECT ?, width, height, image_full, image_lg, image_md, image_sm, image_thumb, CURRENT_TIMESTAMP
               FROM event_flyers WHERE event_id = ?"#,
            new_event_id,
            old_event_id
        )
        .execute(db)
        .await?;

        for size in ["image_thumb", "image_sm"] {
            if let Some(bytes) = CACHE.get(old_slug, size) {
                CACHE.insert(new_slug, size, bytes);
            }
        }

        Ok(())
    }
}

// Flyer image bytes cache (slug, size_column) -> Bytes.
// Only "image_thumb" and "image_sm" are cached (~12mb for ~350 flyers at the time of writing).
#[derive(Default)]
struct EventFlyerCache {
    map: DashMap<(&'static str, &'static str), Bytes>,
    generation: AtomicU64,
}

impl EventFlyerCache {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn get(&self, slug: &str, size: &str) -> Option<Bytes> {
        let slug: &'static str = unsafe { &*(slug as *const str) };
        let size: &'static str = unsafe { &*(size as *const str) };
        match self.map.get(&(slug, size)) {
            Some(entry) => Some(entry.value().clone()),
            None => None,
        }
    }

    fn insert(&self, slug: &str, size: &'static str, bytes: impl Into<Bytes>) {
        let boxed_slug = Box::<str>::from(slug);
        let static_slug: &'static str = unsafe { &*Box::into_raw(boxed_slug) };
        self.map.insert((static_slug, size), bytes.into());
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn remove(&self, slug: &str, size: &'static str) {
        let slug: &'static str = unsafe { &*(slug as *const str) };
        if let Some(((static_slug, _), _bytes)) = self.map.remove(&(slug, size)) {
            let boxed_slug: Box<str> = unsafe { Box::from_raw(static_slug as *const str as *mut str) };
            drop(boxed_slug);
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub async fn populate(&self, db: &Db) -> Result<()> {
        let start = Instant::now();

        let rows = sqlx::query!(
            r#"SELECT e.slug, f.image_thumb, f.image_sm
               FROM event_flyers f
               JOIN events e ON e.id = f.event_id"#
        )
        .fetch_all(db)
        .await?;

        for r in rows {
            self.insert(&r.slug, "image_thumb", r.image_thumb);
            self.insert(&r.slug, "image_sm", r.image_sm);
        }

        tracing::info!("Populated flyer cache in {:?}", start.elapsed());
        Ok(())
    }
}

// Incremented on any change to the flyer cache.
