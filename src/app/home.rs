use crate::prelude::*;

/// Add all `home` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/", get(home_page)))
}

/// Display the front page.
async fn home_page() -> impl IntoResponse {
    #[derive(Template, WebTemplate)]
    #[template(path = "index.html")]
    pub struct Html;
    Html
}
