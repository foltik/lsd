use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use super::error::AppResult;
use crate::EmailConfig;

/// Email client.
#[derive(Clone)]
pub struct Emailer {
    /// Mailbox to send email from.
    from: Mailbox,
    /// Underlying SMTPS transport.
    transport: SmtpTransport,
}

impl Emailer {
    pub async fn connect(config: EmailConfig) -> anyhow::Result<Self> {
        // `lettre` requires a default provider to be installed to use SMTPS.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let mut transport = SmtpTransport::from_url(&config.smtp_addr)?;
        if let (Some(username), Some(password)) = (config.smtp_username, config.smtp_password) {
            transport = transport.credentials(Credentials::new(username, password));
        }
        let transport = transport.build();

        Ok(Self { transport, from: config.from })
    }

    pub fn builder(&self) -> MessageBuilder {
        Message::builder().from(self.from.clone())
    }

    pub async fn send(&self, message: Message) -> AppResult<()> {
        self.transport.send(&message)?;
        Ok(())
    }
}
