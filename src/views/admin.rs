use crate::db::user::User;
// use crate::views::filters;
use askama::Template;

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboard {
    pub users: Vec<User>,
}
