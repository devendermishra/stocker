//! Portfolio routes mounted under `/api/v1/portfolio/` (no login required).

use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use stocker_portfolio::{
    Error as PortfolioError, ImportApplyRequest, ImportParseBody, LabelEntityType, NewLabel,
    NewPortfolio, NewTransaction, PortfolioService, PortfolioViewOptions, RegisterMfSchedule,
    TransactionFilter, UpdatePortfolio,
};

#[derive(Clone)]
pub struct PortfolioState {
    pub service: Arc<PortfolioService>,
}

pub fn router(service: Arc<PortfolioService>) -> Router {
    Router::new()
        .route(
            "/api/v1/portfolio/portfolios",
            get(list_portfolios).post(create_portfolio),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}",
            get(get_portfolio)
                .put(update_portfolio)
                .delete(delete_portfolio),
        )
        .route(
            "/api/v1/portfolio/labels",
            get(list_labels).post(create_label),
        )
        .route(
            "/api/v1/portfolio/labels/{id}",
            put(update_label).delete(delete_label),
        )
        .route("/api/v1/portfolio/labels/attach", post(attach_label))
        .route("/api/v1/portfolio/labels/detach", post(detach_label))
        .route(
            "/api/v1/portfolio/transactions",
            get(list_transactions).post(create_transaction),
        )
        .route(
            "/api/v1/portfolio/transactions/{id}",
            get(get_transaction)
                .put(update_transaction)
                .delete(delete_transaction),
        )
        .route(
            "/api/v1/portfolio/transactions/{id}/duplicate",
            post(duplicate_transaction),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/dashboard",
            get(dashboard),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/holdings",
            get(holdings),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/summary",
            get(summary),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/allocation/stock",
            get(allocation_stock),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/allocation/label",
            get(allocation_label),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/rebuild",
            post(rebuild_portfolio),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/refresh-prices",
            post(refresh_prices),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/sip/refresh",
            post(refresh_sip_transactions),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/swp/refresh",
            post(refresh_swp_transactions),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/mf-schedule",
            post(register_mf_schedule),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/mf-schedules",
            get(list_mf_schedules),
        )
        .route(
            "/api/v1/portfolio/mf-schedules/{id}/inactivate",
            post(inactivate_mf_schedule),
        )
        .route(
            "/api/v1/portfolio/mf/schemes/{code}",
            get(get_mf_scheme),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/refresh/scan",
            post(scan_portfolio_refresh),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/refresh/apply",
            post(apply_portfolio_refresh),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/transactions/clear",
            post(clear_portfolio_transactions),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/export/holdings.csv",
            get(export_holdings),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/export/transactions.csv",
            get(export_transactions),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/import/parse",
            post(parse_import),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/import/parse-json",
            post(parse_import_json),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/import/preview",
            post(preview_import_route),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/import",
            post(apply_import),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/stock/{symbol}/lots",
            get(fifo_lots),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/realized",
            get(realized_matches),
        )
        .route("/api/v1/portfolio/mf/search", get(search_mutual_funds))
        .with_state(PortfolioState { service })
}

async fn local_user(s: &PortfolioState) -> Result<stocker_portfolio::User, Response> {
    s.service.local_user().await.map_err(portfolio_error_response)
}

#[derive(Deserialize, Default)]
struct ViewQuery {
    refresh_prices: Option<bool>,
}

fn view_options(q: &ViewQuery) -> PortfolioViewOptions {
    PortfolioViewOptions {
        force_refresh_prices: q.refresh_prices.unwrap_or(false),
        ..Default::default()
    }
}

#[derive(Deserialize)]
struct MfSearchQuery {
    q: String,
}

async fn search_mutual_funds(
    State(s): State<PortfolioState>,
    Query(q): Query<MfSearchQuery>,
) -> Response {
    portfolio_response(s.service.search_mutual_funds(&q.q).await)
}

#[derive(Deserialize)]
struct ListPortfoliosQuery {
    include_archived: Option<bool>,
}

async fn list_portfolios(
    State(s): State<PortfolioState>,
    Query(q): Query<ListPortfoliosQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .list_portfolios(user.id, q.include_archived.unwrap_or(false))
            .await
            .map(|p| serde_json::json!({ "portfolios": p })),
    )
}

async fn get_portfolio(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.get_portfolio(user.id, id).await)
}

async fn create_portfolio(
    State(s): State<PortfolioState>,
    Json(input): Json<NewPortfolio>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response_status(
        s.service.create_portfolio(user.id, &input).await,
        StatusCode::CREATED,
    )
}

async fn update_portfolio(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdatePortfolio>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.update_portfolio(user.id, id, &input).await)
}

async fn delete_portfolio(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.delete_portfolio(user.id, id).await)
}

async fn list_labels(State(s): State<PortfolioState>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .list_labels(user.id)
            .await
            .map(|l| serde_json::json!({ "labels": l })),
    )
}

async fn create_label(
    State(s): State<PortfolioState>,
    Json(input): Json<NewLabel>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response_status(
        s.service.create_label(user.id, &input).await,
        StatusCode::CREATED,
    )
}

async fn update_label(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(input): Json<NewLabel>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.update_label(user.id, id, &input).await)
}

async fn delete_label(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.delete_label(user.id, id).await)
}

#[derive(Deserialize)]
struct LabelLinkRequest {
    label_id: i64,
    entity_type: String,
    entity_id: String,
}

async fn attach_label(
    State(s): State<PortfolioState>,
    Json(req): Json<LabelLinkRequest>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    let entity_type = match LabelEntityType::parse(&req.entity_type) {
        Some(t) => t,
        None => {
            return portfolio_error_response(PortfolioError::InvalidInput(
                "invalid entity_type".into(),
            ))
        }
    };
    match s
        .service
        .attach_label(user.id, req.label_id, entity_type, &req.entity_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

async fn detach_label(
    State(s): State<PortfolioState>,
    Json(req): Json<LabelLinkRequest>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    let entity_type = match LabelEntityType::parse(&req.entity_type) {
        Some(t) => t,
        None => {
            return portfolio_error_response(PortfolioError::InvalidInput(
                "invalid entity_type".into(),
            ))
        }
    };
    match s
        .service
        .detach_label(user.id, req.label_id, entity_type, &req.entity_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

async fn list_transactions(
    State(s): State<PortfolioState>,
    Query(filter): Query<TransactionFilter>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .list_transactions(user.id, &filter)
            .await
            .map(|t| serde_json::json!({ "transactions": t })),
    )
}

async fn get_transaction(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.get_transaction(user.id, id).await)
}

async fn create_transaction(
    State(s): State<PortfolioState>,
    Json(input): Json<NewTransaction>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response_status(
        s.service.create_transaction(user.id, &input).await,
        StatusCode::CREATED,
    )
}

async fn update_transaction(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(input): Json<NewTransaction>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.update_transaction(user.id, id, &input).await)
}

async fn delete_transaction(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    match s.service.delete_transaction(user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

async fn duplicate_transaction(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response_status(
        s.service.duplicate_transaction(user.id, id).await,
        StatusCode::CREATED,
    )
}

async fn dashboard(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.dashboard(user.id, id, view_options(&q)).await)
}

async fn holdings(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .holdings(user.id, id, view_options(&q))
            .await
            .map(|h| serde_json::json!({ "holdings": h })),
    )
}

async fn summary(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.summary(user.id, id, view_options(&q)).await)
}

async fn allocation_stock(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .allocation_by_stock(user.id, id, view_options(&q))
            .await
            .map(|a| serde_json::json!({ "allocation": a })),
    )
}

async fn allocation_label(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .allocation_by_label(user.id, id, view_options(&q))
            .await
            .map(|a| serde_json::json!({ "allocation": a })),
    )
}

async fn refresh_prices(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.refresh_prices(user.id, id).await)
}

async fn rebuild_portfolio(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service.rebuild_portfolio(user.id, id).await.map(|r| {
            serde_json::json!({
                "portfolio_id": r.portfolio_id,
                "rebuilt_at": r.rebuilt_at,
                "total_invested": r.total_invested,
                "total_realized": r.total_realized,
                "total_dividend": r.total_dividend,
            })
        }),
    )
}

async fn refresh_sip_transactions(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.refresh_sip_transactions(user.id, id).await)
}

async fn refresh_swp_transactions(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.refresh_swp_transactions(user.id, id).await)
}

async fn register_mf_schedule(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(body): Json<RegisterMfSchedule>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.register_mf_schedule(user.id, id, &body).await)
}

async fn list_mf_schedules(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.list_mf_schedules(user.id, id).await)
}

async fn inactivate_mf_schedule(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.inactivate_mf_schedule(user.id, id).await)
}

async fn get_mf_scheme(State(s): State<PortfolioState>, Path(code): Path<i64>) -> Response {
    portfolio_response(s.service.get_mf_scheme(code).await)
}

#[derive(Deserialize)]
struct PortfolioRefreshApplyBody {
    selections: Vec<String>,
}

async fn scan_portfolio_refresh(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.scan_portfolio_refresh(user.id, id).await)
}

async fn apply_portfolio_refresh(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(body): Json<PortfolioRefreshApplyBody>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .apply_portfolio_refresh(user.id, id, &body.selections)
            .await,
    )
}

async fn clear_portfolio_transactions(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.clear_portfolio_transactions(user.id, id).await)
}

async fn export_holdings(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(q): Query<ViewQuery>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    match s
        .service
        .export_holdings_csv(user.id, id, view_options(&q))
        .await {
        Ok(csv) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv")],
            csv,
        )
            .into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

async fn export_transactions(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Query(mut filter): Query<TransactionFilter>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    filter.portfolio_id = Some(id);
    match s.service.export_transactions_csv(user.id, &filter).await {
        Ok(csv) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv")],
            csv,
        )
            .into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

async fn parse_import(
    State(s): State<PortfolioState>,
    Path(_id): Path<i64>,
    mut multipart: Multipart,
) -> Response {
    let _ = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = "upload.csv".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| filename.clone());
            match field.bytes().await {
                Ok(bytes) => file_bytes = Some(bytes.to_vec()),
                Err(e) => {
                    return portfolio_error_response(PortfolioError::InvalidInput(format!(
                        "read upload: {e}"
                    )));
                }
            }
        }
    }

    let bytes = match file_bytes {
        Some(b) => b,
        None => {
            return portfolio_error_response(PortfolioError::InvalidInput(
                "missing file field".into(),
            ));
        }
    };

    portfolio_response(s.service.parse_import_file(&bytes, &filename))
}

async fn parse_import_json(
    State(s): State<PortfolioState>,
    Path(_id): Path<i64>,
    Json(body): Json<ImportParseBody>,
) -> Response {
    let _ = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    let bytes = match base64_decode(&body.data_base64) {
        Ok(b) => b,
        Err(e) => return portfolio_error_response(PortfolioError::InvalidInput(e)),
    };
    portfolio_response(s.service.parse_import_file(&bytes, &body.filename))
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 256] = &{
        let mut t = [255u8; 256];
        let mut i = 0u8;
        while i < 64 {
            t[b"A"[0] as usize + i as usize] = i;
            t[b"a"[0] as usize + i as usize] = i;
            t[b"0"[0] as usize + i as usize] = i + 52;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in &bytes {
        if b == b'=' {
            break;
        }
        let v = TABLE[b as usize];
        if v == 255 {
            return Err("invalid base64".into());
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

async fn preview_import_route(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(body): Json<ImportApplyRequest>,
) -> Response {
    let _ = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    let rows = s
        .service
        .preview_import(id, body.header_row, &body.column_mapping, &body.grid)
        .await;
    portfolio_response(Ok(serde_json::json!({ "rows": rows })))
}

async fn apply_import(
    State(s): State<PortfolioState>,
    Path(id): Path<i64>,
    Json(body): Json<ImportApplyRequest>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .import_transactions(
                user.id,
                id,
                body.header_row,
                &body.column_mapping,
                &body.grid,
            )
            .await,
    )
}

async fn fifo_lots(
    State(s): State<PortfolioState>,
    Path((id, symbol)): Path<(i64, String)>,
) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .fifo_lots(user.id, id, &symbol)
            .await
            .map(|l| serde_json::json!({ "lots": l })),
    )
}

async fn realized_matches(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .realized_matches(user.id, id)
            .await
            .map(|m| serde_json::json!({ "matches": m })),
    )
}

fn portfolio_response<T: serde::Serialize>(result: Result<T, PortfolioError>) -> Response {
    match result {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

fn portfolio_response_status<T: serde::Serialize>(
    result: Result<T, PortfolioError>,
    status: StatusCode,
) -> Response {
    match result {
        Ok(v) => (status, Json(v)).into_response(),
        Err(e) => portfolio_error_response(e),
    }
}

fn portfolio_error_response(e: PortfolioError) -> Response {
    let status = match &e {
        PortfolioError::NotFound => StatusCode::NOT_FOUND,
        PortfolioError::Unauthorized => StatusCode::UNAUTHORIZED,
        PortfolioError::Forbidden => StatusCode::FORBIDDEN,
        PortfolioError::InvalidInput(_) | PortfolioError::Ledger(_) => StatusCode::BAD_REQUEST,
        PortfolioError::Conflict(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
}
