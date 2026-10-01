use lettre::message::header::{Header, HeaderName, HeaderValue};
use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::PoolConfig;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::EmailConfig;
use crate::prelude::*;

/// SMTP reply code from SES when we exceed our max send rate.
const THROTTLED_STATUS_CODE: u16 = 454;
const THROTTLE_RETRIES: u32 = 3;
const THROTTLE_BACKOFF: Duration = Duration::from_secs(1);

/// Email client.
#[derive(Clone)]
pub struct Emailer {
    /// Mailbox to send email from.
    from: Mailbox,
    /// Underlying SMTPS transport.
    transport: AsyncSmtpTransport<Tokio1Executor>,
    /// Max emails per second for bulk sending.
    ratelimit: usize,
}

impl Emailer {
    pub async fn connect(config: EmailConfig) -> Result<Self> {
        // `lettre` requires a default provider to be installed to use SMTPS.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        let mut transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(&config.smtp_addr)?
            .pool_config(PoolConfig::new().max_size(config.ratelimit as u32));
        if let (Some(username), Some(password)) = (config.smtp_username, config.smtp_password) {
            transport = transport.credentials(Credentials::new(username, password));
        }
        let transport = transport.build();

        Ok(Self { transport, from: config.from, ratelimit: config.ratelimit })
    }

    pub fn builder(&self, email_token: &str) -> MessageBuilder {
        Message::builder()
            .from(self.from.clone())
            .header(SesMessageTags(format!("token={email_token}")))
    }

    pub async fn send(&self, message: &Message) -> Result<()> {
        let mut retries = 0;
        loop {
            match self.transport.send(message.clone()).await {
                Err(e)
                    if e.status().map(u16::from) == Some(THROTTLED_STATUS_CODE)
                        && retries < THROTTLE_RETRIES =>
                {
                    let backoff = THROTTLE_BACKOFF * 2u32.pow(retries);
                    tracing::warn!("SES: throttled; retrying in {backoff:?}: {e}");
                    tokio::time::sleep(backoff).await;
                    retries += 1;
                }
                result => {
                    result?;
                    return Ok(());
                }
            }
        }
    }

    pub async fn send_batch(
        &self, state: SharedAppState, messages: Vec<Message>,
    ) -> impl Stream<Item = Result<Progress>> + use<> {
        let interval = Duration::from_secs_f64(1.0 / self.ratelimit as f64);
        let start = tokio::time::Instant::now();
        let total = messages.len() as u32;

        futures::stream::iter(messages.into_iter().enumerate())
            .map(move |(i, message)| {
                let state = Arc::clone(&state);
                async move {
                    // Pace by start time to factor out SMTP send latency
                    tokio::time::sleep_until(start + interval * i as u32).await;
                    state.mailer.send(&message).await
                }
            })
            // Buffer at most `ratelimit` concurrent SMTP connections,
            // allowing for up to 1 second of latency per send.
            .buffered(self.ratelimit)
            .enumerate()
            .map(move |(i, result)| {
                let sent = i as u32 + 1;
                result.map(|_| Progress { sent, remaining: total - sent })
            })
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
