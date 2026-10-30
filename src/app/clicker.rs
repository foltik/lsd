use chrono::{TimeZone, Timelike};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::net::UdpSocket;

use crate::db::clicker::{ClickerDelta, ClickerStats};
use crate::db::event::Event;
use crate::prelude::*;
use crate::utils::config::ClickerConfig;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.restricted_routes(User::ADMIN, |r| r.route("/clicker", get(page)))
}

const HMAC_LEN: usize = 16;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct ClickerPacketHeader {
    ticks: u64,
    ticks_per_second: u32,
    firmware: [u8; 4],
    boot_ticks: u64,
    count: u32,
    voltage: u16,
    time_awake: u32,
    time_doze: u32,
    time_asleep: u32,
    rssi: i8,
    backlog: u16,
    connects: u16,
    disconnects: u16,
    error_resets: u16,
    errors: u16,
    last_error: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct ClickerPacketEntry {
    id: u64,
}

impl ClickerPacketEntry {
    const SIGN: u64 = 1 << 63;

    fn rtc_ticks(&self) -> u64 {
        self.id & !Self::SIGN
    }

    fn delta(&self) -> i64 {
        if self.id & Self::SIGN != 0 { -1 } else { 1 }
    }
}

pub fn serve(config: ClickerConfig, db: Db) {
    tokio::spawn(async move {
        let socket = UdpSocket::bind(config.addr).await.expect("clicker::bind()");
        tracing::info!("Listening for clicker at {}", config.addr);

        let hmac = config.hmac;
        let mut boot: Option<(u64, NaiveDateTime)> = None;
        let mut buf = [0u8; 1500];
        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                continue;
            };
            match handle(&db, &hmac, &buf[..len], &mut boot).await {
                Ok(reply) => {
                    let _ = socket.send_to(&reply, from).await;
                }
                Err(e) => tracing::warn!("clicker: {from}: {e:?}"),
            }
        }
    });
}

fn read<T: Copy>(bytes: &[u8]) -> T {
    unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast()) }
}

async fn handle(
    db: &Db, hmac: &Hmac<Sha256>, packet: &[u8], boot: &mut Option<(u64, NaiveDateTime)>,
) -> Result<Vec<u8>> {
    if packet.len() < HMAC_LEN + size_of::<ClickerPacketHeader>() {
        bail!("packet too short");
    }
    let payload_len = packet.len() - HMAC_LEN;

    let (payload, tag) = packet.split_at(payload_len);
    if hmac.clone().chain_update(payload).verify_truncated_left(tag).is_err() {
        bail!("bad tag");
    }

    let Some(event) = Event::current(db).await? else {
        bail!("no current event");
    };

    let h: ClickerPacketHeader = read(payload);
    let ticks_per_second = h.ticks_per_second.max(1) as u64;
    let ago = |ticks: u64| {
        chrono::Duration::milliseconds((h.ticks.saturating_sub(ticks) * 1000 / ticks_per_second) as i64)
    };
    let now = Utc::now();

    for bytes in payload[size_of::<ClickerPacketHeader>()..]
        .as_chunks::<{ size_of::<ClickerPacketEntry>() }>()
        .0
    {
        let entry: ClickerPacketEntry = read(bytes);
        let delta = ClickerDelta {
            id: entry.id as i64,
            time: (now - ago(entry.rtc_ticks())).naive_utc(),
            delta: entry.delta(),
        };
        ClickerDelta::create(db, &delta).await?;
    }
    let count = ClickerDelta::current_event_count(db, event.id).await?.max(0) as u32;

    let boot_time = match *boot {
        Some((ticks, at)) if ticks == h.boot_ticks => at,
        _ => {
            let at = (now - ago(h.boot_ticks)).naive_utc();
            *boot = Some((h.boot_ticks, at));
            at
        }
    };
    ClickerStats::create(
        db,
        &ClickerStats {
            time: now.naive_utc(),
            count: h.count as i64,
            voltage: h.voltage as i64,
            time_awake: h.time_awake as i64,
            time_doze: h.time_doze as i64,
            time_asleep: h.time_asleep as i64,
            rssi: h.rssi as i64,
            backlog: h.backlog as i64,
            connects: h.connects as i64,
            disconnects: h.disconnects as i64,
            errors: h.errors as i64,
            error_resets: h.error_resets as i64,
            last_error: h.last_error as i64,
            boot: boot_time,
            firmware: hex::encode(h.firmware),
        },
    )
    .await?;

    let time = now.with_timezone(&config().app.tz).num_seconds_from_midnight();

    let mut reply = Vec::with_capacity(16 + event.title.len() + HMAC_LEN);
    reply.extend(h.ticks.to_le_bytes());
    reply.extend(time.to_le_bytes());
    reply.extend(count.to_le_bytes());
    reply.extend(event.title.as_bytes());

    let tag = hmac.clone().chain_update(&reply).finalize().into_bytes();
    reply.extend(&tag[..HMAC_LEN]);

    Ok(reply)
}

struct Row {
    time: String,
    count: i64,
    backlog: i64,
    rssi: i64,
    voltage: i64,
    time_awake: String,
    time_doze: String,
    time_asleep: String,
    connects: i64,
    disconnects: i64,
    errors: i64,
    error_resets: i64,
    last_error: i64,
    boot: String,
    firmware: String,
}

fn hhmm(seconds: i64) -> String {
    format!("{}:{:02}", seconds / 3600, seconds / 60 % 60)
}

async fn page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    let tz = state.config.app.tz;
    let day_ago = (Utc::now() - chrono::Duration::hours(24)).naive_utc();

    // Compute rolling stats
    let mut base = [0i64; 7];
    let mut prev = [0i64; 7];
    let mut rows = Vec::new();
    for s in ClickerStats::all(&state.db).await? {
        let now = [
            s.time_awake,
            s.time_doze,
            s.time_asleep,
            s.connects,
            s.disconnects,
            s.errors,
            s.error_resets,
        ];
        if now[..3].iter().sum::<i64>() < prev[..3].iter().sum::<i64>() {
            for (b, p) in base.iter_mut().zip(prev) {
                *b += p;
            }
        }
        prev = now;

        if s.time < day_ago {
            continue;
        }

        let [
            time_awake,
            time_doze,
            time_asleep,
            connects,
            disconnects,
            errors,
            error_resets,
        ] = core::array::from_fn(|i| base[i] + now[i]);
        rows.push(Row {
            time: tz.from_utc_datetime(&s.time).format("%-I:%M:%S %p").to_string(),
            count: s.count,
            backlog: s.backlog,
            rssi: s.rssi,
            voltage: s.voltage,
            time_awake: hhmm(time_awake),
            time_doze: hhmm(time_doze),
            time_asleep: hhmm(time_asleep),
            connects,
            disconnects,
            errors,
            error_resets,
            last_error: s.last_error,
            boot: tz.from_utc_datetime(&s.boot).format("%-m/%-d %-I:%M:%S %p").to_string(),
            firmware: s.firmware,
        });
    }
    rows.reverse();
    let errors = rows.iter().any(|r| r.errors > 0 || r.error_resets > 0);

    #[derive(Template, WebTemplate)]
    #[template(path = "clicker.html")]
    struct Html {
        user: Option<User>,
        rows: Vec<Row>,
        errors: bool,
    }
    Ok(Html { user, rows, errors }.into_response())
}
