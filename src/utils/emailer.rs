use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::prelude::*;
use crate::EmailConfig;

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
    pub async fn connect(config: EmailConfig) -> anyhow::Result<Self> {
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

    pub fn builder(&self) -> MessageBuilder {
        Message::builder().from(self.from.clone())
    }

    pub async fn send(&self, message: &Message) -> AppResult<()> {
        self.transport.send(message)?;
        Ok(())
    }

    pub async fn send_batch(
        &self,
        state: SharedAppState,
        messages: Vec<Message>,
    ) -> impl Stream<Item = AppResult<Progress>> {
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
