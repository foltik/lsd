use std::sync::Arc;
use tokio_schedule::{every, Job};

use crate::{utils::types::SharedAppState, Config};

pub async fn init(state: SharedAppState, config: Config) {
    let config = Arc::new(config.clone());
    let tz = config.app.tz.clone();

    let state_ = state.clone();
    tokio::spawn(
        every(1)
            .minute()
            .at(0)
            .in_timezone(&tz)
            .perform(move || expire_tokens(state_.clone())),
    );
}

async fn expire_tokens(state: SharedAppState) {}
