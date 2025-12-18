use std::sync::Arc;

use tokio_schedule::{Job, every};

use crate::Config;
use crate::db::rsvp_session::RsvpSession;
use crate::utils::types::SharedAppState;

pub async fn init(state: SharedAppState, config: Config) {
    let config = Arc::new(config.clone());
    let tz = config.app.tz;

    let state_ = state.clone();
    tokio::spawn(
        every(1)
            .minute()
            .at(0)
            .in_timezone(&tz)
            .perform(move || expire_rsvp_sessions(state_.clone())),
    );
}
async fn expire_rsvp_sessions(state: SharedAppState) {
    let _ = RsvpSession::delete_expired(&state.db).await;
}
