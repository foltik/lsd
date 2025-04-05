use crate::db::event::Event;
use askama::Template;

#[derive(Template)]
#[template(path = "events/create.html")]
pub struct EventCreate;

#[derive(Template)]
#[template(path = "events/list.html")]
pub struct EventList {
    pub events: Vec<Event>,
}

#[derive(Template)]
#[template(path = "events/view.html")]
pub struct EventView {
    pub event: Option<Event>,
}
