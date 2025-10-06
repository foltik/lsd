use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context as _;
use chrono_tz::Tz;
use lettre::message::Mailbox;

impl Config {
    /// Load a `.toml` file from disk and parse it as a [`Config`].
    #[allow(unused)]
    pub async fn load(file: &str) -> anyhow::Result<Config> {
        async fn load_inner(file: &str) -> anyhow::Result<Config> {
            let contents = tokio::fs::read_to_string(file).await?;
            Ok(toml::from_str(&contents)?)
        }
        load_inner(file).await.with_context(|| format!("loading config={file}"))
    }

    /// Parse a string as a [`Config`].
    #[allow(unused)]
    pub fn parse(contents: &str) -> anyhow::Result<Config> {
        fn parse_inner(contents: &str) -> anyhow::Result<Config> {
            Ok(toml::from_str(contents)?)
        }
        parse_inner(contents).context("loading config")
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
    pub stripe: StripeConfig,
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
    /// How long until a login session expires.
    pub session_expiry_days: u32,
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
    /// Maximum number of emails to send per second.
    #[serde(default = "default_ratelimit")]
    pub ratelimit: usize,
    /// Mailbox to send email from.
    pub from: Mailbox,
    /// Mailbox to list as ReplyTo for the newsletter.
    pub newsletter_reply_to: Option<Mailbox>,
    /// Mailbox to send contact form submissions to.
    pub contact_to: Option<Mailbox>,
}
fn default_ratelimit() -> usize {
    10
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct StripeConfig {
    pub publishable_key: String,
    pub secret_key: String,
    pub webhook_key: String,
}
