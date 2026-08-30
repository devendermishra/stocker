mod handlers;
mod portfolio;
mod screener;
mod sectors;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use handlers::AppState;
use stocker_portfolio::PortfolioService;
use stocker_screener::ScreenerService;
use tower_http::cors::{Any, CorsLayer};

/// Build the router used by `stocker-api`.
pub fn router(
    screener: Option<Arc<ScreenerService>>,
    portfolio: Option<Arc<PortfolioService>>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = AppState {
        screener: screener.clone(),
    };

    let mut router = Router::new()
        .route("/health", get(handlers::health))
        .route(
            "/api/v1/symbols/{symbol}/report",
            get(handlers::api_report),
        )
        .with_state(state);

    if let Some(svc) = screener {
        router = router
            .merge(screener::router(svc.clone()))
            .merge(sectors::router(svc));
    }

    if let Some(svc) = portfolio {
        router = router.merge(portfolio::router(svc));
    }

    router.layer(cors)
}
