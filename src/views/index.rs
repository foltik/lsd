use askama::Template;

use crate::db::user::User;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub user: Option<User>,
}
