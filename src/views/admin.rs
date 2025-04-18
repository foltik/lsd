use crate::db::user::UserView;
use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "admin/dashboard.html")]
pub struct AdminDashboard {
    pub users: Vec<UserView>,
}
