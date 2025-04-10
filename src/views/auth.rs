use askama::Template;

use crate::db::user::User;

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct Login {
    pub user: Option<User>,
}

#[derive(Template)]
#[template(path = "auth/login_email_sent.html")]
pub struct LoginEmailSent<'a> {
    pub user: Option<User>,
    pub email: &'a str,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct Register {
    pub user: Option<User>,
    pub token: String,
}
