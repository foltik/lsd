use askama::Template;

use crate::db::user::User;

#[derive(Template)]
#[template(path = "user/edit.html")]
pub struct UserProfile {
    pub user: Option<User>,
}
