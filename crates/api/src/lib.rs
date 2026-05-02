mod handlers;

use axum::routing::get;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};

pub fn router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/symbols/{symbol}/report", get(handlers::api_report))
        .layer(cors)
}
