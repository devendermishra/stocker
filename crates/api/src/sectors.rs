//! Sector research routes under `/api/v1/sectors`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use stocker_screener::{Error as ScreenerError, ScreenerService};

#[derive(Clone)]
pub struct SectorState {
    pub service: Arc<ScreenerService>,
}

pub fn router(service: Arc<ScreenerService>) -> Router {
    Router::new()
        .route("/api/v1/sectors", get(list_sectors))
        .route("/api/v1/sectors/{sector}", get(get_sector))
        .with_state(SectorState { service })
}

fn screener_json<T: Serialize>(result: Result<T, ScreenerError>) -> axum::response::Response {
    match result {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(ScreenerError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "sector not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn list_sectors(State(s): State<SectorState>) -> impl IntoResponse {
    match s.service.list_sectors().await {
        Ok(items) => (StatusCode::OK, Json(serde_json::json!({ "sectors": items }))).into_response(),
        Err(ScreenerError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "sector not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_sector(
    State(s): State<SectorState>,
    Path(sector): Path<String>,
) -> impl IntoResponse {
    screener_json(s.service.sector_detail(&sector).await)
}
