use crate::db::event::Event;
use crate::prelude::*;

pub struct ClickerDelta {
    /// RTC tick count at time of click
    pub id: i64,
    /// Derived time of click
    pub time: NaiveDateTime,
    /// +1 or -1
    pub delta: i64,
}

impl ClickerDelta {
    pub async fn create(db: &Db, delta: &ClickerDelta) -> Result<()> {
        sqlx::query!(
            "INSERT OR IGNORE INTO clicker_deltas (id, time, delta) VALUES (?, ?, ?)",
            delta.id,
            delta.time,
            delta.delta
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn count_for_current_event(db: &Db, event_id: i64) -> Result<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT CAST(COALESCE(SUM(d.delta), 0) AS INT) AS "count!: i64" FROM clicker_deltas d, events e
               WHERE e.id = ?
               AND d.time BETWEEN datetime(e.start, '-6 hours') AND datetime(COALESCE(e.end, e.start), '+24 hours')
               AND NOT EXISTS (
                   SELECT 1 FROM events o
                   WHERE (o.start > e.start OR (o.start = e.start AND o.id < e.id))
                   AND o.start <= datetime(d.time, '+6 hours')
                   AND datetime(COALESCE(o.end, o.start), '+24 hours') >= d.time
               )"#,
            event_id
        )
        .fetch_one(db)
        .await?)
    }

    pub async fn for_event(db: &Db, event_id: i64) -> Result<Vec<ClickerDelta>> {
        Ok(sqlx::query_as!(
            ClickerDelta,
            r#"SELECT d.id, d.time, d.delta FROM clicker_deltas d, events e
               WHERE e.id = ? AND d.time BETWEEN datetime(e.start, '-6 hours') AND datetime(COALESCE(e.end, e.start), '+24 hours')
               AND NOT EXISTS (
                   SELECT 1 FROM events o
                   WHERE (o.start > e.start OR (o.start = e.start AND o.id < e.id))
                   AND o.start <= datetime(d.time, '+6 hours')
                   AND datetime(COALESCE(o.end, o.start), '+24 hours') >= d.time
               )
               ORDER BY d.time"#,
            event_id
        )
        .fetch_all(db)
        .await?)
    }

    pub async fn past_events_with_clicks(db: &Db, start: NaiveDateTime) -> Result<Vec<Event>> {
        Ok(sqlx::query_as!(
            Event,
            r#"SELECT * FROM events e WHERE start < ?
               AND EXISTS (
                   SELECT 1 FROM clicker_deltas d
                   WHERE d.time BETWEEN datetime(e.start, '-6 hours') AND datetime(COALESCE(e.end, e.start), '+24 hours')
               )
               ORDER BY start DESC, id"#,
            start
        )
        .fetch_all(db)
        .await?)
    }
}

#[derive(Debug)]
pub struct ClickerStats {
    /// Derived time of sample
    pub time: NaiveDateTime,
    /// Derived boot time, with `ticks` identifying a resent sample
    pub boot: NaiveDateTime,
    /// RTC tick count at time of sample
    pub ticks: i64,
    /// Battery voltage in mV
    pub voltage: i64,
    /// Wifi signal strength in dBm, 0 when disconnected
    pub rssi: i64,
    /// Seconds since power on spent awake with the screen on
    pub time_awake: i64,
    /// Seconds since power on spent in light sleep
    pub time_doze: i64,
    /// Seconds since power on spent in deep sleep
    pub time_asleep: i64,
    /// Firmware sha256.
    pub firmware: String,
}

impl ClickerStats {
    pub async fn create(db: &Db, stats: &ClickerStats) -> Result<()> {
        sqlx::query!(
            "INSERT OR IGNORE INTO clicker_stats (time, boot, ticks, voltage, rssi, time_awake, time_doze, time_asleep, firmware)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            stats.time, stats.boot, stats.ticks, stats.voltage, stats.rssi, stats.time_awake, stats.time_doze,
            stats.time_asleep, stats.firmware
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn all(db: &Db) -> Result<Vec<ClickerStats>> {
        Ok(sqlx::query_as!(
            ClickerStats,
            "SELECT time, boot, ticks, voltage, rssi, time_awake, time_doze, time_asleep, firmware
             FROM clicker_stats ORDER BY time"
        )
        .fetch_all(db)
        .await?)
    }
}
