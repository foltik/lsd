use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub message: String,
}
