use lettre::message::header::{Header, HeaderName, HeaderValue};
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::EmailConfig;
use crate::prelude::*;

/// Email client.
#[derive(Clone)]
pub struct Emailer {
    /// Mailbox to send email from.
    from: Mailbox,
    /// Underlying SMTPS transport.
    transport: SmtpTransport,
    /// Batch size for bulk email sending.
    batch_size: usize,
}

impl Emailer {
    pub async fn connect(config: EmailConfig) -> Result<Self> {
        // `lettre` requires a default provider to be installed to use SMTPS.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let mut transport = SmtpTransport::from_url(&config.smtp_addr)?;
        if let (Some(username), Some(password)) = (config.smtp_username, config.smtp_password) {
            transport = transport.credentials(Credentials::new(username, password));
        }
        let transport = transport.build();
        let batch_size = config.ratelimit;

        Ok(Self { transport, from: config.from, batch_size })
    }

    pub fn builder(&self, email_token: &str) -> MessageBuilder {
        Message::builder()
            .from(self.from.clone())
            .header(SesMessageTags(format!("token={email_token}")))
    }

    pub async fn send(&self, message: &Message) -> Result<()> {
        self.transport.send(message)?;
        Ok(())
    }

    pub async fn send_batch(
        &self, state: SharedAppState, messages: Vec<Message>,
    ) -> impl Stream<Item = Result<Progress>> + use<> {
        async_stream::stream! {
            let mut progress = Progress { sent: 0, remaining: messages.len() as u32 };

            for batch in messages.chunks(state.mailer.batch_size) {
                for message in batch {
                    let result = state.mailer.send(message).await;
                    progress.sent += 1;
                    progress.remaining -= 1;

                    yield result.map(|_| progress);

                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub sent: u32,
    pub remaining: u32,
}

/// SES specific header which is echoed back in events and stripped before delivery.
#[derive(Clone)]
struct SesMessageTags(String);
impl Header for SesMessageTags {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-SES-MESSAGE-TAGS")
    }
    fn parse(s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self(s.to_owned()))
    }
    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), self.0.clone())
    }
}
