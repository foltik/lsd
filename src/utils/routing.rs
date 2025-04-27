use axum::http::request::Parts;

use crate::prelude::*;

pub type AxumRouter = axum::Router<SharedAppState>;

/// A wrapper around the axum router and the shared state, with some additional helpers.
pub struct AppRouter {
    router: AxumRouter,
    state: SharedAppState,
}

impl AppRouter {
    /// Create a new empty `AppRouter`.
    pub fn new(state: &SharedAppState) -> Self {
        Self { router: Default::default(), state: Arc::clone(state) }
    }

    pub fn finish(self) -> (AxumRouter, SharedAppState) {
        (self.router, self.state)
    }

    /// Add some public routes.
    pub fn public_routes(mut self, func: impl FnOnce(AxumRouter) -> AxumRouter) -> Self {
        let subrouter = func(AxumRouter::new());
        self.router = self.router.merge(subrouter);
        self
    }

    /// Add some routes which require authorization and a specific role.
    pub fn restricted_routes(
        mut self,
        role: &'static str,
        func: impl FnOnce(AxumRouter) -> AxumRouter,
    ) -> Self {
        let subrouter = func(AxumRouter::new());
        let subrouter = subrouter.route_layer(axum::middleware::from_fn_with_state(
            self.state.clone(),
            move |State(state): State<SharedAppState>, user: User, req: Request, next: Next| async move {
                if !user.has_role(&state.db, role).await? {
                    return Err(AppError::NotAuthorized);
                }
                Ok(next.run(req).await)
            },
        ));
        self.router = self.router.merge(subrouter);
        self
    }
}

/// Enable extracting an `Option<User>` in a handler.
impl<S: Send + Sync> axum::extract::OptionalFromRequestParts<S> for User {
    type Rejection = Infallible;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Option<Self>, Self::Rejection> {
        Ok(parts.extensions.get::<User>().cloned())
    }
}
/// Enable extracting a `User` in a handler, returning UNAUTHORIZED if not logged in.
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for User {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = parts.extensions.get::<User>().cloned().ok_or(AppError::NotAuthorized)?;
        Ok(user)
    }
}
