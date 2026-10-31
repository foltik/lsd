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
               WHERE (SELECT id FROM events WHERE start <= datetime(d.time, '+6 hours')
                         AND datetime(COALESCE(end, start), '+24 hours') >= d.time ORDER BY start DESC LIMIT 1) = ?"#,
            event_id
        )
        .fetch_one(db)
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
