use crate::db::event::Event;
use crate::db::user::User;
use crate::views::filters;
use askama::Template;

#[derive(Template)]
#[template(path = "events/create.html")]
pub struct EventCreate {
    pub user: Option<User>,
}

#[derive(Template)]
#[template(path = "events/list.html")]
pub struct EventList {
    pub user: Option<User>,
    pub events: Vec<Event>,
}

#[derive(Template)]
#[template(path = "events/view.html")]
pub struct EventView {
    pub user: Option<User>,
    pub event: Event,
}
