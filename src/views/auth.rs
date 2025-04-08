use askama::Template;

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct Login;

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct Register {
    pub token: String,
}
