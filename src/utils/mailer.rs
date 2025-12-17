use lettre::message::{Mailbox, MessageBuilder};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use tokio::sync::{Notify, broadcast};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::EmailConfig;
use crate::db::email_queue::EmailBatch;
use crate::prelude::*;
use crate::utils::sentry;

// DB:
//
// emails:
// + errored_at
// + batch_id
//
// email_batches:
// + id
// + status
// + size
// + errored
// + sent
// + opened
//
// email_queue:
// + batch_id
// + index

// HTTP:
// get(/emails) -> debug view
// get(/emails/{batch_id}) -> debug view one
//
// stream(/emails) -> stream of updates to all batches (all fields of email_batches)
// stream(/emails/{batch_id}) -> stream of updates to one batch

pub struct Mailer {
    /// Wakeup the worker from its slumber
    wakeup: Arc<Notify>,
    /// Channel streaming EmailBatch row updates
    stream: broadcast::Sender<EmailBatch>,

    handle: JoinHandle<()>,
}

impl Mailer {
    pub async fn send(&self, db: &Db, batch: &EmailBatch) -> Result<()> {
        batch.enqueue_back(db).await?;
        self.wakeup.notify_waiters();
        Ok(())
    }

    pub async fn send_prioritized(&self, db: &Db, batch: &EmailBatch) -> Result<()> {
        batch.enqueue_front(db).await?;
        self.wakeup.notify_waiters();
        Ok(())
    }

    pub fn stream(&self) -> impl Stream<Item = EmailBatch> {
        BroadcastStream::new(self.stream.subscribe()).filter_map(|res| async move {
            match res {
                Ok(row) => Some(row),
                Err(BroadcastStreamRecvError::Lagged(_)) => None,
            }
        })
    }
}

impl Mailer {
    pub async fn new(config: EmailConfig, db: Db) -> Result<Self> {
        // `lettre` requires a default provider to be installed to use SMTPS.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let mut transport = SmtpTransport::from_url(&config.smtp_addr)?;
        if let (Some(username), Some(password)) = (config.smtp_username, config.smtp_password) {
            transport = transport.credentials(Credentials::new(username, password));
        }
        let transport = transport.build();

        let wakeup = Arc::new(Notify::new());
        let (stream, _) = broadcast::channel(64);
        let worker = Worker { db, transport, wakeup: wakeup.clone(), stream: stream.clone() };

        let handle = tokio::task::spawn(worker.run());

        Ok(Self { wakeup, stream, handle })
    }
}

struct Worker {
    db: Db,
    transport: SmtpTransport,
    wakeup: Arc<Notify>,
    stream: broadcast::Sender<EmailBatch>,
}

impl Worker {
    pub async fn run(self) {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            // We wake up immediately on being notified of a new queued email, otherwise check every minute.
            // It's important to also poll in case the server restarts, in which case we wouldn't otherwise get woken up.
            tokio::select! {
                _ = self.wakeup.notified() => {},
                _ = tick.tick() => {}
            }

            loop {
                match self.send_queued().await {
                    Ok(_) => break,
                    Err(e) => {
                        sentry::report(format!("Error while processing email queue: {}", e.message()));
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }
    }

    pub async fn send_queued(&self) -> Result<()> {
        let delay_per_email = Duration::from_secs_f64(1.0 / config().email.ratelimit as f64);
        let mut next_send_at = Instant::now();
        loop {
            let email = match EmailBatch::next(&self.db).await? {
                None => return Ok(()), // queue is now empty
                Some(e) => e,
            };

            let now = Instant::now();
            if next_send_at > now {
                tokio::time::sleep_until(next_send_at.into()).await;
            }

            match self.send_one(&email).await {
                Ok(_) => {
                    Email::mark_sent(&self.db, email.id).await?;
                    EmailBatch::inc_sent(&self.db, email.batch_id).await?;
                }
                Err(e) => {
                    let e = format!(
                        "while sending email_id={} in batch_id={}: {}",
                        email.id,
                        email.batch_id,
                        e.message()
                    );
                    Email::mark_error(&self.db, email.id, &e).await?;
                    EmailBatch::inc_errored(&self.db, email.batch_id).await?;
                    sentry::report(e);
                }
            };

            next_send_at += delay_per_email;
        }
    }

    pub async fn send_one(&self, email: &Email) -> Result<()> {
        let message = email.format(&self.db).await?;
        let transport = self.transport.clone();
        tokio::task::spawn_blocking(move || transport.send(&message)).await??;
        Ok(())
    }
}
