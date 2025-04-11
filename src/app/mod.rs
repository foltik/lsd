use std::sync::Arc;

use axum::{response::Redirect, routing::get, Router};
use tower_http::services::ServeDir;

use crate::{
    db::{self, Db},
    utils::{self, config::*, emailer::Emailer, tracing::WithTracingLayer as _},
};

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
    db: Db,
    mailer: Emailer,
}

pub async fn build(config: Config) -> anyhow::Result<Router> {
    let state = Arc::new(AppState {
        config: config.clone(),
        db: db::init(&config.db).await?,
        mailer: Emailer::connect(config.email).await?,
    });

    let r = Router::new()
        .merge(home::routes())
        .merge(auth::routes())
        .nest("/posts", posts::routes())
        .route("/p/{url}", get(posts::view_post_page))
        .nest("/events", events::routes())
        .nest("/lists", lists::routes())
        .nest("/emails", emails::routes())
        .nest_service("/static", ServeDir::new("frontend/static"))
        // For non-HTML pages without a <link rel="icon">, this is where the browser looks
        .route("/favicon.ico", get(|| async { Redirect::to("/static/favicon.ico") }))
        .fallback(|| async { utils::error::serve_404() })
        .layer(axum::middleware::from_fn_with_state(Arc::clone(&state), auth::middleware))
        .with_tracing_layer()
        .with_state(state);

    Ok(r)
}
