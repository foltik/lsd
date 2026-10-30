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

    pub async fn current_event_count(db: &Db, event_id: i64) -> Result<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT CAST(COALESCE(SUM(delta), 0) AS INT) AS "count!: i64" FROM clicker_deltas d
               WHERE (SELECT id FROM events WHERE start <= datetime(d.time, '+6 hours') ORDER BY start DESC LIMIT 1) = ?"#,
            event_id
        )
        .fetch_one(db)
        .await?)
    }
}

#[derive(Debug)]
pub struct ClickerStats {
    /// Derived time stats were recorded.
    pub time: NaiveDateTime,
    /// Device count.
    pub count: i64,
    /// Battery voltage in mV
    pub voltage: i64,
    /// Seconds spent awake with the screen on
    pub time_awake: i64,
    /// Seconds spent in light sleep
    pub time_doze: i64,
    /// Seconds spent in deep sleep
    pub time_asleep: i64,

    /// Wifi signal strength in dBm
    pub rssi: i64,
    /// Click backlog while wifi disconnected
    pub backlog: i64,
    /// Number of wifi connections
    pub connects: i64,
    /// Number of wifi disconnections
    pub disconnects: i64,
    /// Number of wifi errors
    pub errors: i64,
    /// Number of resets after 3 wifi errors in a row
    pub error_resets: i64,
    /// Last wifi error code
    pub last_error: i64,

    /// Last boot time
    pub boot: NaiveDateTime,
    /// Firmware sha256.
    pub firmware: String,
}

impl ClickerStats {
    pub async fn create(db: &Db, stats: &ClickerStats) -> Result<()> {
        sqlx::query!(
            "INSERT INTO clicker_stats (time, count, voltage, time_awake, time_doze, time_asleep, rssi, backlog, connects,
                                        disconnects, errors, error_resets, last_error, boot, firmware)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            stats.time, stats.count, stats.voltage, stats.time_awake, stats.time_doze, stats.time_asleep, stats.rssi,
            stats.backlog, stats.connects, stats.disconnects, stats.errors, stats.error_resets, stats.last_error,
            stats.boot, stats.firmware
        )
        .execute(db)
        .await?;
        Ok(())
    }

    pub async fn all(db: &Db) -> Result<Vec<ClickerStats>> {
        Ok(sqlx::query_as!(
            ClickerStats,
            "SELECT time, count, voltage, time_awake, time_doze, time_asleep, rssi, backlog, connects, disconnects,
                    errors, error_resets, last_error, boot, firmware
             FROM clicker_stats ORDER BY time"
        )
        .fetch_all(db)
        .await?)
    }
}
