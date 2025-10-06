use lettre::message::Mailbox;
use serde::Deserialize;

use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/contact", get(contact_us_page).post(contact_us_form)))
}

async fn contact_us_page(user: Option<User>) -> AppResult<impl IntoResponse> {
    #[derive(Template, WebTemplate)]
    #[template(path = "contact/send.html")]
    struct ContactUsTemplate {
        user: Option<User>,
    };

    Ok(ContactUsTemplate { user })
}

#[derive(Deserialize, Debug)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
}

async fn contact_us_form(
    user: Option<User>,
    State(state): State<SharedAppState>,
    Form(form): Form<ContactForm>,
) -> AppResult<impl IntoResponse> {
    let name = Some(form.name).filter(|n| !n.is_empty());
    let email = Some(form.email).filter(|e| !e.is_empty());

    let to = state.config.email.contact_to.clone();
    let from = state.config.email.from.clone();
    let subject = match &name {
        Some(name) => format!("[{name}]: {}", form.subject),
        None => format!("[Anonymous]: {}", form.subject),
    };
    let reply_to = match email {
        Some(e) => Some(Mailbox::new(name, e.parse().map_err(|_| AppError::BadRequest)?)),
        None => None,
    };

    let mut message = state.mailer.builder().to(to.unwrap_or(from)).subject(subject);
    if let Some(reply_to) = reply_to {
        message = message.reply_to(reply_to);
    }
    let message = message.body(form.message)?;

    state.mailer.send(&message).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "contact/message_sent.html")]
    struct Html {
        user: Option<User>,
    };
    Ok(Html { user })
}
