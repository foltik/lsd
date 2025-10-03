use lettre::Message;
use lettre::message::Mailbox;
use serde::Deserialize;

use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/contact", get(contact_us_page).post(contact_us_form)))
}

async fn contact_us_page() -> AppResult<impl IntoResponse> {
    #[derive(Template, WebTemplate)]
    #[template(path = "contact/send.html")]
    struct ContactUsTemplate;

    Ok(ContactUsTemplate)
}

#[derive(Deserialize, Debug)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
}

async fn contact_us_form(
    State(state): State<SharedAppState>,
    Form(form): Form<ContactForm>,
) -> AppResult<impl IntoResponse> {
    tracing::info!("Subitting contact form: {:?}", &form);

    let name = if form.name.is_empty() {
        Some("Anonymous".to_owned())
    } else {
        Some(form.name)
    };

    let email: lettre::Address = if form.email.is_empty() {
        "noreply@lightandsound.design".parse().unwrap()
    } else {
        form.email.parse().map_err(|_| AppError::BadRequest)?
    };

    let mailbox = Mailbox::new(name, email);

    let message = Message::builder()
        .from(mailbox.clone())
        .to(Mailbox::new(
            Some("Studio".to_owned()),
            "studio@lightandsound.design".parse().unwrap(),
        ))
        .subject(form.subject)
        .body(form.message)?;

    state.mailer.send(&message).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "contact/message_sent.html")]
    struct MessageSentTemplate;

    Ok(MessageSentTemplate)
}
