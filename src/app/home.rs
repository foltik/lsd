use crate::db::event::Event;
use crate::prelude::*;

/// Add all `home` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router
        .public_routes(|r| r.route("/", get(home_page)).route("/past", get(past_page)))
        // TODO: Rethink roles, not WRITER. Template out buttons based on role.
        .restricted_routes(User::WRITER, |r| r.route("/dashboard", get(dashboard_page)))
}

#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
struct HomeHtml {
    user: Option<User>,
    events: Vec<Event>,
    past: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "dashboard.html")]
struct DashboardHtml {
    user: Option<User>,
}

/// Display the front page.
async fn home_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    Ok(HomeHtml { user, events: Event::list_upcoming(&state.db).await?, past: false }.into_response())
}

async fn past_page(user: Option<User>, State(state): State<SharedAppState>) -> HtmlResult {
    Ok(HomeHtml { user, events: Event::list_past(&state.db).await?, past: true }.into_response())
}

async fn dashboard_page(user: User) -> HtmlResult {
    Ok(DashboardHtml { user: Some(user) }.into_response())
}
