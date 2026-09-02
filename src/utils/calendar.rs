use chrono::{TimeDelta, Utc};
use url::Url;

use crate::db::event::Event;
use crate::prelude::*;

/// Venue name used for every event.
const LOCATION: &str = "KG's";
/// Assumed length of an event which has no explicit end time.
const DEFAULT_DURATION_HOURS: i64 = 3;

/// `YYYYMMDDTHHMMSSZ`, used by ICS, Google, and Yahoo
const COMPACT_TIME: &str = "%Y%m%dT%H%M%SZ";
/// `YYYY-MM-DDTHH:MM:SSZ`, used by Outlook and Microsoft 365
const ISO_TIME: &str = "%Y-%m-%dT%H:%M:%SZ";

/// Apple Calendar and Outlook desktop have no deep link scheme, so they use `.ics` files
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Google,
    Outlook,
    Office,
    Yahoo,
}

impl Provider {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "google" => Self::Google,
            "outlook" => Self::Outlook,
            "office" => Self::Office,
            "yahoo" => Self::Yahoo,
            _ => return None,
        })
    }
}

pub struct CalendarEvent {
    pub uid: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub url: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

impl CalendarEvent {
    pub fn from_event(event: &Event) -> Self {
        let cfg = config();
        let url = format!("{}/e/{}", cfg.app.url, event.slug);
        Self {
            // Stable per event, so re-adding updates the entry instead of duplicating it.
            uid: format!("{}@{}", event.token, cfg.app.domain),
            title: event.title.clone(),
            description: url.clone(),
            location: LOCATION.into(),
            url,
            start: event.start,
            end: event.end.unwrap_or(event.start + TimeDelta::hours(DEFAULT_DURATION_HOURS)),
        }
    }

    pub fn ics(&self) -> String {
        let dtstamp = Utc::now().naive_utc().format(COMPACT_TIME);
        let start = self.start.format(COMPACT_TIME);
        let end = self.end.format(COMPACT_TIME);

        let mut out = String::new();
        for line in [
            "BEGIN:VCALENDAR".into(),
            "VERSION:2.0".into(),
            "PRODID:-//Light and Sound Design//lsd//EN".into(),
            "CALSCALE:GREGORIAN".into(),
            "METHOD:PUBLISH".into(),
            "BEGIN:VEVENT".into(),
            format!("UID:{}", escape(&self.uid)),
            format!("DTSTAMP:{dtstamp}"),
            format!("DTSTART:{start}"),
            format!("DTEND:{end}"),
            format!("SUMMARY:{}", escape(&self.title)),
            format!("DESCRIPTION:{}", escape(&self.description)),
            format!("LOCATION:{}", escape(&self.location)),
            format!("URL:{}", self.url),
            "STATUS:CONFIRMED".into(),
            "END:VEVENT".into(),
            "END:VCALENDAR".into(),
        ] {
            fold(&line, &mut out);
            out.push_str("\r\n");
        }
        out
    }

    pub fn provider_url(&self, provider: Provider) -> String {
        let compact_start = self.start.format(COMPACT_TIME).to_string();
        let compact_end = self.end.format(COMPACT_TIME).to_string();
        let range = format!("{compact_start}/{compact_end}");
        let start = self.start.format(ISO_TIME).to_string();
        let end = self.end.format(ISO_TIME).to_string();

        let (base, params): (_, Vec<(&str, &str)>) = match provider {
            Provider::Google => (
                "https://calendar.google.com/calendar/render",
                vec![
                    ("action", "TEMPLATE"),
                    ("text", &self.title),
                    ("dates", &range),
                    ("details", &self.description),
                    ("location", &self.location),
                ],
            ),
            Provider::Outlook => (
                "https://outlook.live.com/calendar/0/addfromweb/",
                vec![
                    ("subject", &self.title),
                    ("startdt", &start),
                    ("enddt", &end),
                    ("body", &self.description),
                    ("location", &self.location),
                    ("allday", "false"),
                ],
            ),
            Provider::Office => (
                "https://outlook.office.com/calendar/0/action/compose",
                vec![
                    ("rru", "addevent"),
                    ("subject", &self.title),
                    ("startdt", &start),
                    ("enddt", &end),
                    ("body", &self.description),
                    ("location", &self.location),
                    ("allday", "false"),
                ],
            ),
            Provider::Yahoo => (
                "https://calendar.yahoo.com/",
                vec![
                    ("v", "60"),
                    ("title", &self.title),
                    ("st", &compact_start),
                    ("et", &compact_end),
                    ("desc", &self.description),
                    ("in_loc", &self.location),
                ],
            ),
        };

        Url::parse_with_params(base, params).expect("invalid calendar url").into()
    }
}

/// Escape RFC 5545
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Append `line` to `out` per RFC 5545
fn fold(line: &str, out: &mut String) {
    const LIMIT: usize = 75;

    let mut used = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if used + len > LIMIT {
            out.push_str("\r\n ");
            used = 1;
        }
        out.push(c);
        used += len;
    }
}
