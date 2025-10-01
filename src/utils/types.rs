use std::sync::Arc;

/// The global shared application state.
pub use crate::app::AppState;
pub type SharedAppState = Arc<AppState>;

pub use crate::app::Webhooks;
