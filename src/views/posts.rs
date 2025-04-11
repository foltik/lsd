use std::collections::HashMap;

use askama::Template;
use askama_web::WebTemplate;

use crate::{
    db::{list::List, post::Post},
    views::filters,
};

#[derive(Template, WebTemplate)]
#[template(path = "posts/edit.html")]
pub struct PostEdit {
    pub post: Post,
}

#[derive(Template, Clone)]
#[template(path = "posts/email.html")]
pub struct PostEmail {
    pub post: Post,
    pub opened_url: String,
    pub unsub_url: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "posts/list.html")]
pub struct PostList {
    pub posts: Vec<Post>,
}

#[derive(Template, WebTemplate)]
#[template(path = "posts/send.html")]
pub struct PostSend {
    pub post: Post,
    pub lists: Vec<List>,
}

#[derive(Template, WebTemplate)]
#[template(path = "posts/sent.html")]
pub struct PostSent {
    pub post_title: String,
    pub list_name: String,
    pub num_sent: i64,
    pub num_skipped: i64,
    pub errors: HashMap<String, String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "posts/view.html")]
pub struct PostView {
    pub post: Post,
}
