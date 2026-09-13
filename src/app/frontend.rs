use crate::prelude::*;

pub fn add_routes(router: AppRouter) -> AppRouter {
    router.public_routes(|r| r.route("/frontend/alert", post(alert)))
}

/// JS error reported by the frontend.
#[derive(serde::Deserialize)]
struct Alert {
    message: String,
    url: String,
    line: u32,
}
async fn alert(Json(Alert { message, url, line }): Json<Alert>) -> JsonResult<()> {
    alerts::alert_frontend(message, &url, line);
    Ok(Json(()))
}
