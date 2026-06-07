mod handlers;
mod screener;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use handlers::AppState;
use stocker_screener::ScreenerService;
use tower_http::cors::{Any, CorsLayer};

/// Build the router used by `stocker-api`.
///
/// `service`, when supplied, exposes the `/api/v1/screener/*` routes and runs
/// the refresh job. Pass `None` to bring up just the legacy `/api/v1/symbols/*`
/// endpoints (useful for tests).
pub fn router(service: Option<Arc<ScreenerService>>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = AppState { screener: service.clone() };

    let mut router = Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/api/v1/symbols/{symbol}/report",
            get(handlers::api_report),
        )
        .with_state(state);

    if let Some(svc) = service {
        router = router.merge(screener::router(svc));
    }

    router.layer(cors)
}
