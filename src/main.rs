// No idea why this fires...
#![allow(redundant_semicolons)]

mod app;
mod db;
mod jobs;
mod prelude;
mod utils;

use std::net::SocketAddr;

use axum::handler::HandlerWithoutStateExt;
use axum::response::Redirect;
use axum_server::tls_rustls::RustlsConfig;
use futures::StreamExt;
use tracing::Level;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use utils::config::*;

use crate::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // TODO(sam) is it possible to filter the logs from ServeDir?
    let log_filter = tracing_subscriber::filter::Targets::default()
        .with_target("h2", LevelFilter::OFF)
        .with_target("globset", LevelFilter::OFF)
        .with_target("rustls", LevelFilter::OFF)
        .with_default(Level::INFO);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(log_filter)
        .with(utils::sentry::layer())
        .init();

    // Load the server config
    #[cfg(debug_assertions)]
    let config = {
        let Some(file) = std::env::args().nth(1) else {
            bail!("usage: lsd <config.toml>");
        };
        let config = Config::load(&file).await?;
        tracing::info!("Loaded config at {file:?}: {config:#?}");
        config
    };
    #[cfg(not(debug_assertions))]
    let config = {
        let config = Config::parse(include_str!("../config/prod.toml"))?;
        tracing::info!("Loaded embedded config: {config:#?}");
        config
    };
    // Make it visible globally
    CONFIG.set(config.clone()).unwrap_or_else(|_| unreachable!());

    // Setup error logging
    if let Some(config) = &config.sentry {
        tracing::info!("Sentry enabled");
        utils::sentry::init(config);
    }

    let (router, state) = app::build(config.clone()).await?;
    // HTTP/3 sidesteps make_service, so utils::h3 injects ConnectInfo itself
    let router_h3 = router.clone();
    let app = router.into_make_service_with_connect_info::<SocketAddr>();
    tracing::info!("Live at {}", &config.app.url);

    // Spawn periodic jobs
    jobs::init(state, config.clone()).await;

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
            let mut acme = rustls_acme::AcmeConfig::new(&acme.domains)
                .contact_push(format!("mailto:{}", &acme.email))
                .cache(rustls_acme::caches::DirCache::new(acme.dir.clone()))
                .directory_lets_encrypt(acme.prod)
                .state();

            // default_rustls_config() advertises no ALPN protocols, forcing every client onto HTTP/1.1
            let mut rustls_config = (*acme.default_rustls_config()).clone();
            rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            let acceptor = acme.axum_acceptor(Arc::new(rustls_config.clone()));

            // Serve HTTP/3 on the same port over UDP, sharing the ACME certificates
            utils::h3::spawn(config.net.https_addr, rustls_config, router_h3);

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
            utils::h3::spawn(config.net.https_addr, (*rustls.get_inner()).clone(), router_h3);
            axum_server::bind_rustls(config.net.https_addr, rustls).serve(app).await?;
        }
    }

    Ok(())
}
