use crate::db::event::Event;
use crate::prelude::*;

/// Add all `home` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/", get(home_page)).route("/past", get(past_page)))
}

#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
pub struct HomeHtml {
    pub events: Vec<Event>,
    pub past: bool,
}

/// Display the front page.
async fn home_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    Ok(HomeHtml { events: Event::list_upcoming(&state.db).await?, past: false })
}

async fn past_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    Ok(HomeHtml { events: Event::list_past(&state.db).await?, past: true })
}
