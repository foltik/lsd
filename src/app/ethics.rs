use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use lettre::Message;
use lettre::message::Mailbox;

use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/ethics", get(ethics_page).post(ethics_form)))
}

async fn ethics_page(user: Option<User>) -> HtmlResult {
    #[derive(Template, WebTemplate)]
    #[template(path = "ethics.html")]
    struct Html {
        user: Option<User>,
    }
    Ok(Html { user }.into_response())
}

#[derive(serde::Deserialize, Debug)]
struct EthicsForm {
    name: String,
    email: String,
    statement: String,

    /// Hidden honeypot field to catch bots.
    #[serde(default, rename = "website")]
    honeypot: String,
}

async fn ethics_form(
    user: Option<User>, State(state): State<SharedAppState>, ConnectInfo(client): ConnectInfo<SocketAddr>,
    Form(form): Form<EthicsForm>,
) -> HtmlResult {
    if !form.honeypot.is_empty() {
        tracing::info!("Honeypot caught client_ip={}: {:?}", client.ip(), form);
        bail_invalid!();
    }

    let name = form.name.trim();
    let email = form.email.trim();
    let reply_to = Mailbox::new(Some(name.to_string()), email.parse().map_err(|_| invalid())?);
    let to = format!("secretary@{}", state.config.app.domain).parse::<Mailbox>().unwrap();
    let from = state.config.email.from.clone();

    let message = Message::builder()
        .from(from.clone())
        .to(to)
        .reply_to(reply_to)
        .subject(format!("[Ethics nomination] {name}"))
        .body(format!("{name}\n{email}\n\n{}", form.statement.trim()))?;
    state.mailer.send(&message).await?;

    #[derive(Template, WebTemplate)]
    #[template(path = "ethics_sent.html")]
    struct Html {
        user: Option<User>,
    }
    Ok(Html { user }.into_response())
}
