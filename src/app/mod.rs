use axum::Router;
use axum::extract::DefaultBodyLimit;
use tower::ServiceBuilder;
use tower_http::compression::{self, CompressionLayer, Predicate};

use crate::prelude::*;
use crate::utils::cloudflare::Cloudflare;
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
    pub stripe: Stripe,
    pub cloudflare: Cloudflare,
    pub mailer: Emailer,
}

pub async fn build(config: Config) -> Result<(Router<()>, SharedAppState)> {
    let state = Arc::new(AppState {
        config: config.clone(),
        db: crate::db::init(&config.db).await?,
        stripe: Stripe::new(&config),
        cloudflare: Cloudflare::new(&config)?,
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
        use tower_http::services::ServeDir;
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
        // Serve additional static files from disk
        let r = r.nest_service("/static", ServiceBuilder::new().service(ServeDir::new("/home/lsd/static")));
        r
    };
    // For non-HTML pages without a <link rel="icon">, this is where the browser looks
    let r = r.route("/favicon.ico", get(|| async { Redirect::to("/static/favicon.ico") }));
    // Opt out of all crawlers
    let r = r.route("/robots.txt", get(|| async { "User-agent: *\nDisallow: /\n" }));
    let r = r.fallback(|| async { Err::<(), HtmlError>(not_found().into()) });

    // Register middleware
    let r = auth::add_middleware(r, Arc::clone(&state));
    let r = events::add_middleware(r, Arc::clone(&state));
    let r = r.layer(axum::middleware::from_fn(redirect_secondary_hosts));
    let r = crate::utils::tracing::add_middleware(r);
    let r = r.layer(DefaultBodyLimit::max(16 * 1024 * 1024)); // 16MB limit
    // Advertise HTTP/3 so browsers upgrade on subsequent requests
    let r = r.layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
        header::ALT_SVC,
        HeaderValue::from_str(&format!("h3=\":{}\"; ma=86400", config.net.https_addr.port())).unwrap(),
    ));
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

/// Permanently redirect requests for non-canonical hosts to AppConfig::url.
async fn redirect_secondary_hosts(req: Request, next: Next) -> Response {
    // Use HOST for HTTP/1, otherwise URI authority
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .or_else(|| req.uri().host());

    // Strip an explicit port, e.g. localhost:4433 in dev
    let host = host.map(|host| host.split(':').next().unwrap());

    // * Serve hostless requests as-is
    // * Exempt stripe webhooks, which don't support redirects
    let canonical_host = host.is_none_or(|host| host == config().app.domain);
    if canonical_host || req.uri().path() == "/webhooks/stripe" {
        return next.run(req).await;
    }

    let path_and_query = req.uri().path_and_query().map_or("/", |pq| pq.as_str());
    Redirect::permanent(&format!("{}{path_and_query}", config().app.url)).into_response()
}
