use std::sync::Arc;

/// The global shared application state.
pub use crate::app::AppState;
pub type SharedAppState = Arc<AppState>;

/// The global router type, with our shared application state.
pub type AppRouter = axum::Router<SharedAppState>;
