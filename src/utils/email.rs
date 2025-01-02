use anyhow::Result;
use lettre::{
    message::{header::ContentType, Mailbox, MessageBuilder},
    Message, SmtpTransport, Transport,
};

use crate::EmailConfig;

#[derive(Clone)]
pub struct Email {
    addr: String,
    from: Mailbox,
}

impl Email {
    pub async fn connect(config: EmailConfig) -> Result<Self> {
        // we need this for smtps
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        Ok(Self { addr: config.smtp_addr, from: config.from })
    }

    pub fn builder(&self) -> MessageBuilder {
        Message::builder().from(self.from.clone()).header(ContentType::TEXT_PLAIN)
    }

    pub async fn send(&self, message: Message) -> Result<()> {
        let transport = SmtpTransport::from_url(&self.addr)?.build();
        transport.send(&message)?;
        Ok(())
    }
}
