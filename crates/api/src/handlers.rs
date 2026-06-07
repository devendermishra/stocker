use std::sync::Arc;

use std::fmt::Display;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use stocker_screener::{
    snapshot_is_fresh, snapshot_to_enrichment, DEFAULT_SNAPSHOT_MAX_AGE_SECS, ScreenerService,
};

#[derive(Clone)]
pub struct AppState {
    pub screener: Option<Arc<ScreenerService>>,
}

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

fn api_error(status: StatusCode, msg: impl Display) -> Response {
    (status, Json(serde_json::json!({ "error": msg.to_string() }))).into_response()
}

async fn resolve_screener_enrichment(
    screener: Option<&Arc<ScreenerService>>,
    symbol: &str,
) -> Option<stocker_core::ScreenerMetricSnapshot> {
    let svc = screener?;
    let mut row = svc.snapshot_for(symbol).await.ok().flatten();
    let fresh = row
        .as_ref()
        .map(|r| snapshot_is_fresh(r.updated_at, DEFAULT_SNAPSHOT_MAX_AGE_SECS))
        .unwrap_or(false);
    if !fresh {
        if svc.refresh_now(symbol).await.is_ok() {
            row = svc.snapshot_for(symbol).await.ok().flatten();
        }
    }
    row.as_ref().map(snapshot_to_enrichment)
}

pub async fn api_report(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    let resolve_ctx = if let Some(svc) = state.screener.as_ref() {
        stocker_screener::universe::india_symbol_context_from_db(svc.pool())
            .await
            .ok()
    } else {
        None
    };
    let enrichment = resolve_screener_enrichment(state.screener.as_ref(), &symbol).await;
    match stocker_core::build_research_report(&symbol, enrichment, resolve_ctx.as_ref()).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => api_error(StatusCode::BAD_REQUEST, e),
    }
}
