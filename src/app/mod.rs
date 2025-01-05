use anyhow::Result;
use axum::{response::Redirect, routing::get, Router};
use std::sync::Arc;
use tera::Tera;
use tower_http::services::ServeDir;

use crate::db::Db;
use crate::utils::{self, config::*, email::Email};

mod auth;
mod events;
mod home;
mod posts;

#[derive(Clone)]
#[allow(unused)]
pub struct AppState {
    config: Config,
    templates: Tera,
    db: Db,
    mail: Email,
}

pub async fn build(config: Config) -> Result<Router> {
    let state = AppState {
        config: config.clone(),
        templates: utils::tera::templates(&config)?,
        db: crate::db::init(&config.db.file).await?,
        mail: Email::connect(config.email).await?,
    };

    let r = Router::new();
    let r = home::register_routes(r);
    let r = auth::register_routes(r);
    let r = posts::register_routes(r);
    let r = events::register_routes(r);

    let r = r
        .nest_service("/assets", ServeDir::new("assets"))
        // For non-HTML pages without a <link rel="icon">, this is where the browser looks
        .route("/favicon.ico", get(|| async { Redirect::to("/assets/favicon.ico") }));

    let r = utils::tracing::register(r);

    let r = r.with_state(Arc::new(state));

    Ok(r)
}
