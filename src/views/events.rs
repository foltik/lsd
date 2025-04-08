use askama::Template;
use askama_web::WebTemplate;

use crate::{db::event::Event, views::filters};

#[derive(Template, WebTemplate)]
#[template(path = "events/create.html")]
pub struct EventCreate;

#[derive(Template, WebTemplate)]
#[template(path = "events/list.html")]
pub struct EventList {
    pub events: Vec<Event>,
}

#[derive(Template, WebTemplate)]
#[template(path = "events/view.html")]
pub struct EventView {
    pub event: Event,
}
