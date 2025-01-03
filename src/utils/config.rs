use anyhow::{Context, Result};
use lettre::message::Mailbox;
use std::{net::SocketAddr, path::PathBuf};

impl Config {
    /// Load a `.toml` file from disk and parse it as a [`Config`].
    pub async fn load(file: &str) -> Result<Config> {
        async fn load_inner(file: &str) -> Result<Config> {
            let contents = tokio::fs::read_to_string(file).await?;
            Ok(toml::from_str(&contents)?)
        }
        load_inner(file).await.with_context(|| format!("loading config={file}"))
    }
}

/// Bag of configuration values, parsed from a TOML file with serde.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub net: NetConfig,
    pub acme: Option<AcmeConfig>,
    pub email: EmailConfig,
}

/// Webapp configuration.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct AppConfig {
    pub url: String,
    pub db: PathBuf,
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
    /// Mailbox to send email from.
    pub from: Mailbox,
}
