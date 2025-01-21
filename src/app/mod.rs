use anyhow::Result;
use axum::{response::Redirect, routing::get, Router};
use std::sync::Arc;
use tera::Tera;
use tower_http::services::ServeDir;

use crate::db::Db;
use crate::utils::{self, config::*, emailer::Emailer};

mod auth;
mod emails;
mod events;
mod home;
mod lists;
mod posts;

#[derive(Clone)]
#[allow(unused)]
pub struct AppState {
    config: Config,
    templates: Tera,
    db: Db,
    mailer: Emailer,
}

pub async fn build(config: Config) -> Result<Router> {
    let state = Arc::new(AppState {
        config: config.clone(),
        templates: utils::tera::templates(&config)?,
        db: crate::db::init(&config.db.file).await?,
        mailer: Emailer::connect(config.email).await?,
    });

    let r = Router::new()
        .nest_service("/assets", ServeDir::new("assets"))
        // For non-HTML pages without a <link rel="icon">, this is where the browser looks
        .route("/favicon.ico", get(|| async { Redirect::to("/assets/favicon.ico") }));

    let r = home::register_routes(r);
    let r = posts::register_routes(r);
    let r = events::register_routes(r);
    let r = lists::register_routes(r);
    let r = emails::register_routes(r);

    let r = auth::register(r, Arc::clone(&state));

    let r = utils::tracing::register(r);
    let r = r.with_state(state);

    Ok(r)
}
