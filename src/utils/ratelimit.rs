use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::ConnectInfo;

use crate::prelude::*;

// 30req/min on successful requests
const OK_CAPACITY: f64 = 30.0;
const OK_REFILL_PER_SEC: f64 = 0.5;
// 6req/min on requests returning an error
const ERR_CAPACITY: f64 = 10.0;
const ERR_REFILL_PER_SEC: f64 = 0.1;

/// Prune idle limiter state after reaching 1k entries
const PRUNE_THRESHOLD: usize = 1_000;

struct Limiter {
    ok: f64,
    err: f64,
    updated_at: Instant,
}

impl Limiter {
    fn new() -> Self {
        Self { ok: OK_CAPACITY, err: ERR_CAPACITY, updated_at: Instant::now() }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.updated_at).as_secs_f64();
        self.ok = (self.ok + elapsed * OK_REFILL_PER_SEC).min(OK_CAPACITY);
        self.err = (self.err + elapsed * ERR_REFILL_PER_SEC).min(ERR_CAPACITY);
        self.updated_at = now;
    }

    fn is_idle(&self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.updated_at).as_secs_f64();
        let ok = self.ok + elapsed * OK_REFILL_PER_SEC;
        let err = self.err + elapsed * ERR_REFILL_PER_SEC;
        ok >= OK_CAPACITY && err >= ERR_CAPACITY
    }
}

pub fn add_middleware(router: AxumRouter) -> AxumRouter {
    let state: Arc<Mutex<HashMap<IpAddr, Limiter>>> = Arc::default();
    router.layer(axum::middleware::from_fn(
        move |ConnectInfo(addr): ConnectInfo<SocketAddr>, req: Request, next: Next| {
            let state = Arc::clone(&state);
            async move {
                let ip = addr.ip().to_canonical();

                // Check limits pre-response
                {
                    let mut state_ = state.lock().unwrap();
                    let now = Instant::now();
                    if state_.len() >= PRUNE_THRESHOLD {
                        state_.retain(|_, bucket| !bucket.is_idle(now));
                    }

                    let limiter = state_.entry(ip).or_insert_with(Limiter::new);
                    limiter.refill(now);

                    if limiter.ok < 1.0 || limiter.err < 1.0 {
                        return too_many_requests();
                    }
                }

                // Update limits post-response
                let response = next.run(req).await;
                {
                    let mut state_ = state.lock().unwrap();
                    let limiter = state_.entry(ip).or_insert_with(Limiter::new);

                    let status = response.status();
                    if status.is_client_error() || status.is_server_error() {
                        limiter.err -= 1.0;
                    } else {
                        limiter.ok -= 1.0;
                    }
                }
                response
            }
        },
    ))
}

fn too_many_requests() -> Response {
    let html = ErrorHtml {
        user: None,
        title: "Error".into(),
        message: "Too many requests. Please slow down and try again shortly.".into(),
        context: None,
        backtrace: None,
        contact_email: None,
    };
    (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "10")], html).into_response()
}
