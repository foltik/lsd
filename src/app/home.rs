use crate::db::event::Event;
use crate::prelude::*;

/// Add all `home` routes to the router.
pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/", get(home_page)))
}

/// Display the front page.
async fn home_page(State(state): State<SharedAppState>) -> AppResult<impl IntoResponse> {
    #[derive(Template, WebTemplate)]
    #[template(path = "index.html")]
    pub struct Html {
        pub events: Vec<Event>,
    }
    Ok(Html { events: Event::list_upcoming(&state.db).await? })
}
