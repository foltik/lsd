use anyhow::Result;
use axum::Router;
use std::sync::Arc;
use tera::Tera;
use tower_http::services::ServeDir;

use crate::utils::{self, config::*, db::Db, email::Email};

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
        templates: utils::tera::templates()?,
        db: Db::connect(&config.app.db).await?,
        mail: Email::connect(config.email).await?,
    };

    let r = Router::new();
    let r = home::register_routes(r);
    let r = auth::register_routes(r);
    let r = posts::register_routes(r);
    let r = events::register_routes(r);

    let r = r.nest_service("/assets", ServeDir::new("assets"));
    let r = utils::tracing::register(r);

    let r = r.with_state(Arc::new(state));

    Ok(r)
}
