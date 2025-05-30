use crate::prelude::*;
use crate::utils::emailer::Emailer;

mod auth;
mod emails;
mod events;
mod home;
mod lists;
mod posts;

#[derive(Clone)]
#[allow(unused)]
pub struct AppState {
    pub config: Config,
    pub db: Db,
    pub mailer: Emailer,
}

pub async fn build(config: Config) -> Result<axum::Router<()>> {
    let state = Arc::new(AppState {
        config: config.clone(),
        db: crate::db::init(&config.db).await?,
        mailer: Emailer::connect(config.email).await?,
    });

    // Register business logic routes
    let r = AppRouter::new(&state);
    let r = home::add_routes(r);
    let r = auth::add_routes(r);
    let r = posts::add_routes(r);
    let r = events::add_routes(r);
    let r = lists::add_routes(r);
    let r = emails::add_routes(r);
    let (r, state) = r.finish();

    // Register app-wide routes
    let r = r.nest_service("/static", tower_http::services::ServeDir::new("frontend/static"));
    // For non-HTML pages without a <link rel="icon">, this is where the browser looks
    let r = r.route("/favicon.ico", get(|| async { Redirect::to("/static/favicon.ico") }));
    let r = r.fallback(|| async { AppError::NotFound });

    // Register middleware
    let r = auth::add_middleware(r, Arc::clone(&state));
    let r = crate::utils::tracing::add_middleware(r);
    let r = r.with_state(state);

    Ok(r)
}
