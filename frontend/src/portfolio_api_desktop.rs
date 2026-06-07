//! In-process portfolio backend for the desktop standalone app.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use stocker_portfolio::{db::default_db_path, LabelEntityType, PortfolioService};
use tokio::sync::OnceCell;

use super::{
    AllocationRow, Dashboard, FifoLot, Holding, Label, NewLabel, NewPortfolio, NewTransaction,
    Portfolio, Transaction, TransactionFilter, UpdatePortfolio,
};

static SERVICE: OnceCell<Arc<PortfolioService>> = OnceCell::const_new();

async fn service() -> Result<Arc<PortfolioService>, String> {
    SERVICE
        .get_or_try_init(|| async {
            let screener = crate::screener_api::shared_screener().await.ok();
            let path = default_db_path();
            eprintln!("Opening portfolio DB at {}", path.display());
            let svc = PortfolioService::open(&path, screener)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Arc::new(svc))
        })
        .await
        .cloned()
}

async fn user_id(svc: &PortfolioService) -> Result<i64, String> {
    svc.local_user()
        .await
        .map(|u| u.id)
        .map_err(|e| e.to_string())
}

fn map_err(e: stocker_portfolio::Error) -> String {
    e.to_string()
}

fn convert<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U, String> {
    serde_json::from_value(serde_json::to_value(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub async fn list_portfolios(include_archived: bool) -> Result<Vec<Portfolio>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(
        svc.list_portfolios(uid, include_archived)
            .await
            .map_err(map_err)?,
    )
}

pub async fn create_portfolio(input: &NewPortfolio) -> Result<Portfolio, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let input: stocker_portfolio::NewPortfolio = convert(input)?;
    convert(
        svc.create_portfolio(uid, &input)
            .await
            .map_err(map_err)?,
    )
}

pub async fn update_portfolio(id: i64, input: &UpdatePortfolio) -> Result<Portfolio, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let input: stocker_portfolio::UpdatePortfolio = convert(input)?;
    convert(
        svc.update_portfolio(uid, id, &input)
            .await
            .map_err(map_err)?,
    )
}

pub async fn delete_portfolio(id: i64) -> Result<(), String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    svc.delete_portfolio(uid, id).await.map_err(map_err)
}

pub async fn list_labels() -> Result<Vec<Label>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(svc.list_labels(uid).await.map_err(map_err)?)
}

pub async fn create_label(input: &NewLabel) -> Result<Label, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let input: stocker_portfolio::NewLabel = convert(input)?;
    convert(svc.create_label(uid, &input).await.map_err(map_err)?)
}

pub async fn delete_label(id: i64) -> Result<(), String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    svc.delete_label(uid, id).await.map_err(map_err)
}

pub async fn attach_label(label_id: i64, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let entity_type = LabelEntityType::parse(entity_type)
        .ok_or_else(|| "invalid entity_type".to_string())?;
    svc.attach_label(uid, label_id, entity_type, entity_id)
        .await
        .map_err(map_err)
}

pub async fn list_transactions(filter: &TransactionFilter) -> Result<Vec<Transaction>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let filter: stocker_portfolio::TransactionFilter = convert(filter)?;
    convert(
        svc.list_transactions(uid, &filter)
            .await
            .map_err(map_err)?,
    )
}

pub async fn create_transaction(input: &NewTransaction) -> Result<Transaction, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    let input: stocker_portfolio::NewTransaction = convert(input)?;
    convert(
        svc.create_transaction(uid, &input)
            .await
            .map_err(map_err)?,
    )
}

pub async fn delete_transaction(id: i64) -> Result<(), String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    svc.delete_transaction(uid, id).await.map_err(map_err)
}

pub async fn dashboard(portfolio_id: i64) -> Result<Dashboard, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(
        svc.dashboard(uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn holdings(portfolio_id: i64) -> Result<Vec<Holding>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(svc.holdings(uid, portfolio_id).await.map_err(map_err)?)
}

pub async fn allocation_stock(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(
        svc.allocation_by_stock(uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn allocation_label(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(
        svc.allocation_by_label(uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn rebuild_portfolio(portfolio_id: i64) -> Result<(), String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    svc.rebuild_portfolio(uid, portfolio_id)
        .await
        .map_err(map_err)
        .map(|_| ())
}

pub async fn fifo_lots(portfolio_id: i64, symbol: &str) -> Result<Vec<FifoLot>, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    convert(
        svc.fifo_lots(uid, portfolio_id, symbol)
            .await
            .map_err(map_err)?,
    )
}

pub fn export_holdings_url(portfolio_id: i64) -> String {
    let _ = portfolio_id;
    "#".into()
}

pub fn export_transactions_url(portfolio_id: i64) -> String {
    let _ = portfolio_id;
    "#".into()
}
