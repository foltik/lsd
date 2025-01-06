use anyhow::Result;
use lettre::{
    message::{header::ContentType, Mailbox, MessageBuilder},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};

use crate::EmailConfig;

/// Email client.
#[derive(Clone)]
pub struct Email {
    /// Mailbox to send email from.
    from: Mailbox,
    /// Underlying SMTPS transport.
    transport: SmtpTransport,
}

impl Email {
    pub async fn connect(config: EmailConfig) -> Result<Self> {
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
        Message::builder().from(self.from.clone()).header(ContentType::TEXT_PLAIN)
    }

    pub async fn send(&self, message: Message) -> Result<()> {
        self.transport.send(&message)?;
        Ok(())
    }
}
