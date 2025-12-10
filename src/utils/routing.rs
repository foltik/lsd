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
        mut self, role: &'static str, func: impl FnOnce(AxumRouter) -> AxumRouter,
    ) -> Self {
        let subrouter = func(AxumRouter::new());
        let subrouter = subrouter.route_layer(axum::middleware::from_fn_with_state(
            self.state.clone(),
            move |user: User, req: Request, next: Next| async move {
                if !user.has_role(role) {
                    bail_unauthorized!();
                }
                Ok::<_, HtmlError>(next.run(req).await)
            },
        ));
        self.router = self.router.merge(subrouter);
        self
    }
}
