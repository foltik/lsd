use anyhow::{Context, Result};

mod app;
mod utils;

use axum::{handler::HandlerWithoutStateExt, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;
use futures::StreamExt;
use utils::config::*;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    // Load the server config
    let file = std::env::args().nth(1).context("usage: lsd <config.toml>")?;
    let config = Config::load(&file).await?;

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
