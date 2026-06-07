//! Screener routes mounted under `/api/v1/screener/`.
//!
//! Server mode owns the SQLite file and runs the refresh scheduler in-process.
//! In standalone (desktop) mode the same `ScreenerService` is used directly
//! from the frontend without going through HTTP.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use stocker_screener::{
    Error as ScreenerError, MetricSpec, NewSavedScreen, RefreshConfig, ScreenQuery, ScreenerService,
};

#[derive(Clone)]
pub struct ScreenerState {
    pub service: Arc<ScreenerService>,
}

pub fn router(service: Arc<ScreenerService>) -> Router {
    Router::new()
        .route("/api/v1/screener/fields", get(list_fields))
        .route("/api/v1/screener/search", post(search))
        .route("/api/v1/screener/status", get(status))
        .route("/api/v1/screener/coverage", get(coverage))
        .route("/api/v1/screener/screens", get(list_screens).post(create_screen))
        .route(
            "/api/v1/screener/screens/{id}",
            get(get_screen).delete(delete_screen).put(update_screen),
        )
        .route("/api/v1/screener/snapshot/{symbol}", get(get_snapshot))
        .route("/api/v1/screener/symbols", get(search_symbols))
        .route("/api/v1/screener/symbols/{symbol}/pair", get(get_symbol_pair))
        .route("/api/v1/screener/refresh/{symbol}", post(refresh_now))
        .route("/api/v1/screener/recompute", post(recompute_composites))
        .route("/api/v1/screener/scheduler/stop", post(stop_scheduler))
        .route("/api/v1/screener/backfill", post(start_backfill))
        .with_state(ScreenerState { service })
}

#[derive(serde::Serialize)]
struct CatalogEntry {
    id: stocker_screener::MetricId,
    label: &'static str,
    description: &'static str,
    category: stocker_screener::MetricCategory,
    category_label: &'static str,
    unit: stocker_screener::Unit,
    column: &'static str,
    needs_review: bool,
    source_kind: stocker_screener::SourceKind,
}

impl From<&MetricSpec> for CatalogEntry {
    fn from(s: &MetricSpec) -> Self {
        Self {
            id: s.id,
            label: s.label,
            description: s.description,
            category: s.category,
            category_label: s.category.label(),
            unit: s.unit,
            column: s.column,
            needs_review: s.needs_review,
            source_kind: s.source_kind,
        }
    }
}

async fn list_fields(State(s): State<ScreenerState>) -> impl IntoResponse {
    let entries: Vec<CatalogEntry> = s.service.catalog().iter().map(CatalogEntry::from).collect();
    Json(serde_json::json!({ "fields": entries }))
}

fn screener_json<T: Serialize>(result: Result<T, ScreenerError>) -> axum::response::Response {
    match result {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => screener_error_response(e),
    }
}

fn screener_json_status<T: Serialize>(
    result: Result<T, ScreenerError>,
    status: StatusCode,
) -> axum::response::Response {
    match result {
        Ok(v) => (status, Json(v)).into_response(),
        Err(e) => screener_error_response(e),
    }
}

async fn search(State(s): State<ScreenerState>, Json(q): Json<ScreenQuery>) -> impl IntoResponse {
    screener_json(
        s.service
            .run_query(&q)
            .await
            .map(|rows| serde_json::json!({ "rows": rows })),
    )
}

async fn status(State(s): State<ScreenerState>) -> impl IntoResponse {
    screener_json(s.service.status().await)
}

async fn coverage(State(s): State<ScreenerState>) -> impl IntoResponse {
    screener_json(s.service.coverage().await)
}

async fn list_screens(State(s): State<ScreenerState>) -> impl IntoResponse {
    screener_json(
        s.service
            .list_screens()
            .await
            .map(|screens| serde_json::json!({ "screens": screens })),
    )
}

async fn get_screen(State(s): State<ScreenerState>, Path(id): Path<i64>) -> impl IntoResponse {
    screener_json(s.service.get_screen(id).await)
}

async fn create_screen(
    State(s): State<ScreenerState>,
    Json(new): Json<NewSavedScreen>,
) -> impl IntoResponse {
    screener_json_status(s.service.create_screen(&new).await, StatusCode::CREATED)
}

async fn update_screen(
    State(s): State<ScreenerState>,
    Path(id): Path<i64>,
    Json(new): Json<NewSavedScreen>,
) -> impl IntoResponse {
    screener_json(s.service.update_screen(id, &new).await)
}

async fn delete_screen(State(s): State<ScreenerState>, Path(id): Path<i64>) -> impl IntoResponse {
    match s.service.delete_screen(id).await {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => screener_error_response(e),
    }
}

async fn get_snapshot(
    State(s): State<ScreenerState>,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    match s.service.snapshot_for(&symbol).await {
        Ok(Some(row)) => (StatusCode::OK, Json(row)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "no snapshot for symbol" })),
        )
            .into_response(),
        Err(e) => screener_error_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct SymbolSearchQuery {
    search: Option<String>,
    exchange: Option<String>,
    limit: Option<i64>,
}

async fn search_symbols(
    State(s): State<ScreenerState>,
    Query(q): Query<SymbolSearchQuery>,
) -> impl IntoResponse {
    let search = q.search.unwrap_or_default();
    let limit = q.limit.unwrap_or(50);
    screener_json(
        s.service
            .search_symbols(&search, q.exchange.as_deref(), limit)
            .await
            .map(|rows| serde_json::json!({ "rows": rows })),
    )
}

async fn get_symbol_pair(
    State(s): State<ScreenerState>,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    match s.service.symbol_pair(&symbol).await {
        Ok(Some(listing)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "listing": listing })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "symbol not found" })),
        )
            .into_response(),
        Err(e) => screener_error_response(e),
    }
}

async fn refresh_now(
    State(s): State<ScreenerState>,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    let resolved = match s.service.resolve_symbol(&symbol).await {
        Ok(sym) => sym,
        Err(e) => return screener_error_response(e),
    };
    match s.service.refresh_now(&symbol).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "symbol": resolved })),
        )
            .into_response(),
        Err(e) => screener_error_response(e),
    }
}

async fn stop_scheduler(State(s): State<ScreenerState>) -> impl IntoResponse {
    s.service.stop_scheduler();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "message": "scheduler stop signalled" })),
    )
        .into_response()
}

async fn start_backfill(State(s): State<ScreenerState>) -> impl IntoResponse {
    let cfg = RefreshConfig::from_env();
    match s.service.try_start_backfill(cfg) {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "ok": true, "message": "backfill started in background" })),
        )
            .into_response(),
        Err(ScreenerError::AlreadyRunning) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "Stock data refresh is already running" })),
        )
            .into_response(),
        Err(e) => screener_error_response(e),
    }
}

async fn recompute_composites(State(s): State<ScreenerState>) -> impl IntoResponse {
    screener_json(
        s.service.recompute_composites().await.map(|stats| {
            serde_json::json!({ "ok": true, "rows_touched": stats.rows_touched })
        }),
    )
}

fn screener_error_response(e: ScreenerError) -> axum::response::Response {
    let (status, msg) = match &e {
        ScreenerError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
        ScreenerError::InvalidQuery(m) => (StatusCode::BAD_REQUEST, m.clone()),
        ScreenerError::AlreadyRunning => {
            (StatusCode::CONFLICT, "Stock data refresh is already running".to_string())
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

// `Deserialize` for axum extractors: the raw JSON body is `ScreenQuery` directly,
// no wrapper. Re-export the shape for clarity in OpenAPI tooling.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SearchBody(ScreenQuery);
