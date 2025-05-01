use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context as _;
use chrono_tz::Tz;
use lettre::message::Mailbox;

impl Config {
    /// Load a `.toml` file from disk and parse it as a [`Config`].
    pub async fn load(file: &str) -> anyhow::Result<Config> {
        async fn load_inner(file: &str) -> anyhow::Result<Config> {
            let contents = tokio::fs::read_to_string(file).await?;
            Ok(toml::from_str(&contents)?)
        }
        load_inner(file).await.with_context(|| format!("loading config={file}"))
    }
}

/// Bag of app configuration values, parsed from a TOML file with serde.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub db: DbConfig,
    pub net: NetConfig,
    pub acme: Option<AcmeConfig>,
    pub email: EmailConfig,
}

/// Webapp configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct AppConfig {
    /// Public facing domain, e.g. `site.com`.
    pub domain: String,
    /// Public facing URL, e.g. `https://site.com`.
    pub url: String,
    /// Local timezone.
    pub tz: Tz,
}

/// Database configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct DbConfig {
    /// Path to sqlite3 database file.
    pub file: PathBuf,
    pub seed_data: Option<PathBuf>,
}

/// Networking configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct NetConfig {
    /// HTTP server bind address.
    pub http_addr: SocketAddr,
    /// HTTS server bind address.
    pub https_addr: SocketAddr,
}

/// LetsEncrypt ACME TLS certificate configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct AcmeConfig {
    /// Domain to request a cert for.
    pub domain: String,
    /// Contact email.
    pub email: String,
    /// Directory to store certs and credentials in.
    pub dir: String,
    /// Whether to use the production or staging ACME server.
    pub prod: bool,
}

/// Email configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct EmailConfig {
    /// SMTP address, starting with `smtp://`.
    pub smtp_addr: String,
    /// SMTP username.
    pub smtp_username: Option<String>,
    /// SMTP password.
    pub smtp_password: Option<String>,
    /// Mailbox to send email from.
    pub from: Mailbox,
    /// Mailbox to put as ReplyTo.
    pub reply_to: Option<Mailbox>,
    /// Maximum number of emails to send per second.
    pub ratelimit: Option<usize>,
}
