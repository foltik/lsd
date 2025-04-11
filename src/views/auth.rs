use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "auth/login.html")]
pub struct Login;

#[derive(Template, WebTemplate)]
#[template(path = "auth/register.html")]
pub struct Register {
    pub token: String,
}
