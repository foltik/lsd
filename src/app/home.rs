use crate::db::event::Event;
use crate::prelude::*;

/// Add all `home` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/", get(home_page)).route("/past", get(past_page)))
}

#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
struct HomeHtml {
    user: Option<User>,
    events: Vec<Event>,
    past: bool,
}

/// Display the front page.
async fn home_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    Ok(HomeHtml { user, events: Event::list_upcoming(&state.db).await?, past: false }.into_response())
}

async fn past_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    Ok(HomeHtml { user, events: Event::list_past(&state.db).await?, past: true }.into_response())
}
