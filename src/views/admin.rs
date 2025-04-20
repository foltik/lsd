use crate::db::user::UserView;
use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard/overview.html")]
pub struct AdminDashboardOverview {
    pub users_count: usize,
}

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard/users.html")]
pub struct AdminDashboardUsersView {
    pub users: Vec<UserView>,
}
