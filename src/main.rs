mod app;
mod db;
mod utils;
mod views;

use anyhow::Context as _;
use axum::{handler::HandlerWithoutStateExt, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;
use futures::StreamExt;
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
use utils::config::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO(sam) is it possible to filter the logs from ServeDir?
    let log_filter = tracing_subscriber::filter::Targets::default()
        .with_target("h2", LevelFilter::OFF)
        .with_target("globset", LevelFilter::OFF)
        .with_target("rustls", LevelFilter::OFF)
        .with_default(Level::DEBUG);

    tracing_subscriber::fmt()
        .pretty()
        .with_target(true)
        .with_line_number(true)
        .with_max_level(Level::DEBUG)
        .finish()
        .with(log_filter)
        .try_init()?;

    // Load the server config
    let file = std::env::args().nth(1).context("usage: lsd <config.toml>")?;
    let config = Config::load(&file).await?;
    views::filters::set_timezone(config.app.tz);

    let app = app::build(config.clone()).await?.into_make_service();
    tracing::info!("Live at {}", &config.app.url);

    // Spawn an auxillary HTTP server which just redirects to HTTPS
    tokio::spawn(async move {
        let redirect = move || async move { Redirect::permanent(&config.app.url) };
        axum_server::bind(config.net.http_addr)
            .serve(redirect.into_make_service())
            .await
    });

    // Spawn the main HTTPS server
    match config.acme {
        // If ACME is configured, request a TLS certificate from Let's Encrypt
        Some(acme) => {
            let mut acme = rustls_acme::AcmeConfig::new([&acme.domain])
                .contact_push(format!("mailto:{}", &acme.email))
                .cache(rustls_acme::caches::DirCache::new(acme.dir.clone()))
                .directory_lets_encrypt(acme.prod)
                .state();

            let acceptor = acme.axum_acceptor(acme.default_rustls_config());

            tokio::spawn(async move {
                loop {
                    match acme.next().await.unwrap() {
                        Ok(ok) => tracing::debug!("acme: {:?}", ok),
                        Err(err) => tracing::error!("acme: {}", err),
                    }
                }
            });

            axum_server::bind(config.net.https_addr).acceptor(acceptor).serve(app).await?;
        }
        // Otherwise, use the bundled self-signed TLS cert
        None => {
            let cert = include_bytes!("../config/selfsigned.cert");
            let key = include_bytes!("../config/selfsigned.key");
            let rustls = RustlsConfig::from_pem(cert.into(), key.into()).await?;
            axum_server::bind_rustls(config.net.https_addr, rustls).serve(app).await?;
        }
    }

    Ok(())
}
