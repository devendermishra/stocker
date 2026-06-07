//! Portfolio routes mounted under `/api/v1/portfolio/` (no login required).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use stocker_portfolio::{
    Error as PortfolioError, LabelEntityType, NewLabel, NewPortfolio, NewTransaction,
    PortfolioService, TransactionFilter, UpdatePortfolio,
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
            "/api/v1/portfolio/portfolios/{id}/export/holdings.csv",
            get(export_holdings),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/export/transactions.csv",
            get(export_transactions),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/stock/{symbol}/lots",
            get(fifo_lots),
        )
        .route(
            "/api/v1/portfolio/portfolios/{id}/realized",
            get(realized_matches),
        )
        .with_state(PortfolioState { service })
}

async fn local_user(s: &PortfolioState) -> Result<stocker_portfolio::User, Response> {
    s.service.local_user().await.map_err(portfolio_error_response)
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
    match s.service.delete_portfolio(user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => portfolio_error_response(e),
    }
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
    match s.service.delete_label(user.id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => portfolio_error_response(e),
    }
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

async fn dashboard(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.dashboard(user.id, id).await)
}

async fn holdings(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .holdings(user.id, id)
            .await
            .map(|h| serde_json::json!({ "holdings": h })),
    )
}

async fn summary(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(s.service.summary(user.id, id).await)
}

async fn allocation_stock(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .allocation_by_stock(user.id, id)
            .await
            .map(|a| serde_json::json!({ "allocation": a })),
    )
}

async fn allocation_label(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    portfolio_response(
        s.service
            .allocation_by_label(user.id, id)
            .await
            .map(|a| serde_json::json!({ "allocation": a })),
    )
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

async fn export_holdings(State(s): State<PortfolioState>, Path(id): Path<i64>) -> Response {
    let user = match local_user(&s).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    match s.service.export_holdings_csv(user.id, id).await {
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
