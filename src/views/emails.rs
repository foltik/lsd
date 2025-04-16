use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "emails/unsubscribe.html")]
pub struct Unsubscribe {
    pub email_id: i64,
    pub email_address: String,
}
