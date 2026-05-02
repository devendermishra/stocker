use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn api_report(Path(symbol): Path<String>) -> impl IntoResponse {
    match stocker_core::build_research_report(&symbol).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
