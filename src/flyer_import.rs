//! One-off: import eventcreate flyers using the app's own image pipeline.
//!
//! Reads eventcreate/manifest.json (produced by scripts/correlate_events.js +
//! resolve_flyers.js), and for every event whose `flyer` is set, decodes the image and stores
//! it via EventFlyer::create_or_update — i.e. the same encode_jpeg (progressive q85, 5 sizes)
//! the admin upload uses. Idempotent: events that already have a flyer are skipped. Encoding
//! runs across all cores; DB writes serialize through SQLite's busy timeout.
//!
//! Usage: lsd import-flyers <db.sqlite> <eventcreate/manifest.json>

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::db::event::Event;
use crate::db::event_flyer::EventFlyer;
use crate::prelude::*;

#[derive(serde::Deserialize)]
struct Entry {
    slug: String,
    flyer: Option<FlyerRef>,
}

#[derive(serde::Deserialize)]
struct FlyerRef {
    file: String,
}

#[derive(Default)]
struct Counts {
    added: AtomicU32,
    existing: AtomicU32,
    missing: AtomicU32,
    failed: AtomicU32,
}

pub async fn run(db_path: &str, manifest_path: &str) -> Result<()> {
    let concurrency = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    // busy_timeout lets the parallel encoders' tiny INSERTs wait on SQLite's single writer
    // instead of erroring; max_connections caps concurrent DB work at the core count.
    let opts =
        SqliteConnectOptions::from_str(&format!("sqlite://{db_path}"))?.busy_timeout(Duration::from_secs(30));
    let db: Db = SqlitePoolOptions::new()
        .max_connections(concurrency as u32)
        .connect_with(opts)
        .await?;

    let dir: PathBuf = Path::new(manifest_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let entries: Vec<Entry> = serde_json::from_str(&std::fs::read_to_string(manifest_path)?)?;

    let counts = Arc::new(Counts::default());
    let sem = Arc::new(Semaphore::new(concurrency));
    let mut tasks = JoinSet::new();

    for entry in entries {
        let Some(flyer) = entry.flyer else { continue };
        let (db, dir, counts) = (db.clone(), dir.clone(), counts.clone());
        // Acquire before spawning so at most `concurrency` encodes are in flight at once.
        let permit = sem.clone().acquire_owned().await.expect("semaphore open");
        tasks.spawn(async move {
            let _permit = permit;
            let Some(event) = Event::lookup_by_slug(&db, &entry.slug).await.unwrap_or(None) else {
                tracing::warn!("no event for slug {}", entry.slug);
                counts.missing.fetch_add(1, Ordering::Relaxed);
                return;
            };
            if EventFlyer::exists_for_event(&db, event.id).await.unwrap_or(false) {
                counts.existing.fetch_add(1, Ordering::Relaxed);
                return;
            }
            // image is built with png+jpeg only; gif/webp/tiff (and unreadable files) fail here.
            let image = match std::fs::read(dir.join(&flyer.file))
                .map_err(|e| e.to_string())
                .and_then(|bytes| image::load_from_memory(&bytes).map_err(|e| e.to_string()))
            {
                Ok(image) => image,
                Err(err) => {
                    tracing::warn!("{} ({}): {err}", flyer.file, entry.slug);
                    counts.failed.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            };
            if let Err(err) = EventFlyer::create_or_update(&db, event.id, &image).await {
                tracing::warn!("store {} ({}): {}", flyer.file, entry.slug, err.message());
                counts.failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let n = counts.added.fetch_add(1, Ordering::Relaxed) + 1;
            if n % 50 == 0 {
                tracing::info!("...{n} flyers encoded");
            }
        });
    }
    while tasks.join_next().await.is_some() {}

    tracing::info!(
        "flyers: {} added, {} already present, {} no matching event, {} read/decode failed",
        counts.added.load(Ordering::Relaxed),
        counts.existing.load(Ordering::Relaxed),
        counts.missing.load(Ordering::Relaxed),
        counts.failed.load(Ordering::Relaxed),
    );
    Ok(())
}
