use crate::{db::user::User, views::filters};
use std::collections::HashMap;

use askama::Template;

use crate::db::{list::List, post::Post};

#[derive(Template)]
#[template(path = "posts/edit.html")]
pub struct PostEdit {
    pub user: Option<User>,
    pub post: Post,
}

#[derive(Template, Clone)]
#[template(path = "posts/email.html")]
pub struct PostEmail {
    pub post: Post,
    pub opened_url: String,
    pub unsub_url: String,
}

#[derive(Template)]
#[template(path = "posts/list.html")]
pub struct PostList {
    pub user: Option<User>,
    pub posts: Vec<Post>,
}

#[derive(Template)]
#[template(path = "posts/send.html")]
pub struct PostSend {
    pub user: Option<User>,
    pub post: Post,
    pub lists: Vec<List>,
}

#[derive(Template)]
#[template(path = "posts/sent.html")]
pub struct PostSent {
    pub user: Option<User>,
    pub post_title: String,
    pub list_name: String,
    pub num_sent: i64,
    pub num_skipped: i64,
    pub errors: HashMap<String, String>,
}

#[derive(Template)]
#[template(path = "posts/view.html")]
pub struct PostView {
    pub user: Option<User>,
    pub post: Post,
}
