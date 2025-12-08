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

    // TODO: REMOVE ME!!!
    let state_ = state.clone();
    tokio::spawn(
        every(15)
            .minute()
            .at(0)
            .in_timezone(&tz)
            .perform(move || temp_delete_all_rsvp_sessions(state_.clone())),
    );
}
async fn temp_delete_all_rsvp_sessions(state: SharedAppState) {
    let _ = RsvpSession::temp_delete_all(&state.db).await;
}
async fn expire_rsvp_sessions(state: SharedAppState) {
    let _ = RsvpSession::delete_expired_drafts(&state.db).await;
}
