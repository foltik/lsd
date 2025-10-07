use axum::Router;
use axum::extract::DefaultBodyLimit;
use tower::ServiceBuilder;
use tower_http::compression::{self, CompressionLayer, Predicate};

#[cfg(debug_assertions)]
pub use crate::app::webhooks::Webhooks;
use crate::prelude::*;
use crate::utils::emailer::Emailer;
use crate::utils::stripe::Stripe;

mod auth;
mod contact;
mod emails;
mod events;
mod home;
mod lists;
mod posts;
mod webhooks;

pub struct AppState {
    pub config: Config,
    pub db: Db,
    pub mailer: Emailer,
    pub stripe: Stripe,
    pub webhooks: Webhooks,
}

pub async fn build(config: Config) -> Result<(Router<()>, SharedAppState)> {
    let state = Arc::new(AppState {
        config: config.clone(),
        db: crate::db::init(&config.db).await?,
        stripe: Stripe::new(&config),
        mailer: Emailer::connect(config.email).await?,
        webhooks: Webhooks::default(),
    });

    // Register business logic routes
    let r = AppRouter::new(&state);
    let r = home::add_routes(r);
    let r = auth::add_routes(r);
    let r = posts::add_routes(r);
    let r = events::add_routes(r);
    let r = lists::add_routes(r);
    let r = emails::add_routes(r);
    let r = webhooks::add_routes(r);
    let r = contact::add_routes(r);
    let (r, state) = r.finish();

    // Register app-wide routes
    #[cfg(debug_assertions)]
    let r = {
        use tower_http::services::ServeDir;
        r.nest_service("/static", ServiceBuilder::new().service(ServeDir::new("frontend/static")))
    };
    #[rustfmt::skip]
    #[cfg(not(debug_assertions))]
    let r = {
        use tower_serve_static::{ServeFile, include_file};
        use tower_http::set_header::SetResponseHeaderLayer;
        use axum::http::{HeaderName, HeaderValue};

        let nest_static = |r: Router<Arc<AppState>>, urgency: u8, filename: &str, file: tower_serve_static::File| -> Router<SharedAppState> {
            let service = ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("priority"),
                    HeaderValue::from_str(&format!("u={urgency}")).unwrap(),
                ))
                .service(ServeFile::new(file));
            r.nest_service(&format!("/static/{filename}"), service)
        };

        let r = nest_static(r, 0, "main.css", include_file!("/frontend/static/main.css"));
        let r = nest_static(r, 4, "favicon.ico", include_file!("/frontend/static/favicon.ico"));
        r
    };
    // For non-HTML pages without a <link rel="icon">, this is where the browser looks
    let r = r.route("/favicon.ico", get(|| async { Redirect::to("/static/favicon.ico") }));
    let r = r.fallback(|| async { AppError::NotFound });

    // Register middleware
    let r = auth::add_middleware(r, Arc::clone(&state));
    let r = crate::utils::tracing::add_middleware(r);
    let r = r.layer(DefaultBodyLimit::max(16 * 1024 * 1024)); // 16MB limit
    let r = r.layer(
        CompressionLayer::new().compress_when(
            compression::DefaultPredicate::new()
                .and(compression::predicate::NotForContentType::new("image/jpeg"))
                .and(compression::predicate::NotForContentType::new("font/woff2")),
        ),
    );
    let r = r.with_state(Arc::clone(&state));

    Ok((r, state))
}
