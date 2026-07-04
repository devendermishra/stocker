//! In-process portfolio backend for the desktop standalone app.

use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;
use stocker_mf::{db::default_db_path as mf_db_path, MfService};
use stocker_portfolio::{db::default_db_path, LabelEntityType, PortfolioService, PortfolioViewOptions};

use super::{
    AllocationRow, Dashboard, FifoLot, Holding, ImportApplyRequest, ImportField, ImportResult,
    ImportRowPreview, Label, MfSearchHit, NewLabel, NewPortfolio, NewTransaction, ParsePreview,
    Portfolio, PortfolioRefreshApplyResult, PortfolioRefreshScan, RawGrid, DeleteLabelResult,
    ClearTransactionsResult, MfSchedule, RegisterMfSchedule, RegisterMfScheduleResult,
    SipRefreshResult, SwpRefreshResult, Transaction, TransactionFilter, UpdatePortfolio,
};

static SERVICE: Mutex<Option<Arc<PortfolioService>>> = Mutex::new(None);

async fn open_service() -> Result<PortfolioService, String> {
    let screener = crate::screener_api::shared_screener().await.ok();
    let mf = {
        let path = mf_db_path();
        eprintln!("Opening mutual fund DB at {}", path.display());
        MfService::open(&path).await.ok().map(Arc::new)
    };
    let path = default_db_path();
    eprintln!("Opening portfolio DB at {}", path.display());
    PortfolioService::open(&path, screener, mf)
        .await
        .map_err(|e| e.to_string())
}

async fn service() -> Result<Arc<PortfolioService>, String> {
    if let Some(svc) = SERVICE.lock().unwrap().clone() {
        return Ok(svc);
    }
    let svc = Arc::new(open_service().await?);
    *SERVICE.lock().unwrap() = Some(svc.clone());
    Ok(svc)
}

/// Drop cached DB pools so the next call reopens files from disk (e.g. after sync pull).
pub fn invalidate_portfolio_service() {
    *SERVICE.lock().unwrap() = None;
}

pub async fn local_portfolio_refs_for_sync(
) -> Result<Vec<stocker_sync::LocalPortfolioRef>, String> {
    let portfolios: Vec<Portfolio> = list_portfolios(false).await?;
    convert(portfolios)
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

struct Ctx {
    svc: Arc<PortfolioService>,
    uid: i64,
}

async fn ctx() -> Result<Ctx, String> {
    let svc = service().await?;
    let uid = user_id(&svc).await?;
    Ok(Ctx { svc, uid })
}

fn convert<T: Serialize, U: DeserializeOwned>(value: T) -> Result<U, String> {
    serde_json::from_value(serde_json::to_value(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub async fn list_portfolios(include_archived: bool) -> Result<Vec<Portfolio>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .list_portfolios(c.uid, include_archived)
            .await
            .map_err(map_err)?,
    )
}

pub async fn create_portfolio(input: &NewPortfolio) -> Result<Portfolio, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::NewPortfolio = convert(input)?;
    convert(
        c.svc
            .create_portfolio(c.uid, &input)
            .await
            .map_err(map_err)?,
    )
}

pub async fn update_portfolio(id: i64, input: &UpdatePortfolio) -> Result<Portfolio, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::UpdatePortfolio = convert(input)?;
    convert(
        c.svc
            .update_portfolio(c.uid, id, &input)
            .await
            .map_err(map_err)?,
    )
}

pub async fn delete_portfolio(id: i64) -> Result<DeleteLabelResult, String> {
    let c = ctx().await?;
    convert(c.svc.delete_portfolio(c.uid, id).await.map_err(map_err)?)
}

pub async fn list_labels() -> Result<Vec<Label>, String> {
    let c = ctx().await?;
    convert(c.svc.list_labels(c.uid).await.map_err(map_err)?)
}

pub async fn create_label(input: &NewLabel) -> Result<Label, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::NewLabel = convert(input)?;
    convert(c.svc.create_label(c.uid, &input).await.map_err(map_err)?)
}

pub async fn delete_label(id: i64) -> Result<DeleteLabelResult, String> {
    let c = ctx().await?;
    convert(c.svc.delete_label(c.uid, id).await.map_err(map_err)?)
}

pub async fn attach_label(label_id: i64, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let c = ctx().await?;
    let entity_type = LabelEntityType::parse(entity_type)
        .ok_or_else(|| "invalid entity_type".to_string())?;
    c.svc
        .attach_label(c.uid, label_id, entity_type, entity_id)
        .await
        .map_err(map_err)
}

pub async fn detach_label(label_id: i64, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let c = ctx().await?;
    let entity_type = LabelEntityType::parse(entity_type)
        .ok_or_else(|| "invalid entity_type".to_string())?;
    c.svc
        .detach_label(c.uid, label_id, entity_type, entity_id)
        .await
        .map_err(map_err)
}

pub async fn list_transactions(filter: &TransactionFilter) -> Result<Vec<Transaction>, String> {
    let c = ctx().await?;
    let filter: stocker_portfolio::TransactionFilter = convert(filter)?;
    convert(
        c.svc
            .list_transactions(c.uid, &filter)
            .await
            .map_err(map_err)?,
    )
}

pub async fn create_transaction(input: &NewTransaction) -> Result<Transaction, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::NewTransaction = convert(input)?;
    let txn = convert(
        c.svc
            .create_transaction(c.uid, &input)
            .await
            .map_err(map_err)?,
    )?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(txn)
}

pub async fn update_transaction(id: i64, input: &NewTransaction) -> Result<Transaction, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::NewTransaction = convert(input)?;
    let txn = convert(
        c.svc
            .update_transaction(c.uid, id, &input)
            .await
            .map_err(map_err)?,
    )?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(txn)
}

pub async fn delete_transaction(id: i64) -> Result<(), String> {
    let c = ctx().await?;
    c.svc
        .delete_transaction(c.uid, id)
        .await
        .map_err(map_err)?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(())
}

pub async fn clear_portfolio_transactions(portfolio_id: i64) -> Result<ClearTransactionsResult, String> {
    let c = ctx().await?;
    let result = convert(
        c.svc
            .clear_portfolio_transactions(c.uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(result)
}

pub async fn dashboard(portfolio_id: i64) -> Result<Dashboard, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .dashboard(c.uid, portfolio_id, PortfolioViewOptions::default())
            .await
            .map_err(map_err)?,
    )
}

pub async fn holdings(portfolio_id: i64) -> Result<Vec<Holding>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .holdings(c.uid, portfolio_id, PortfolioViewOptions::default())
            .await
            .map_err(map_err)?,
    )
}

pub async fn allocation_stock(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .allocation_by_stock(c.uid, portfolio_id, PortfolioViewOptions::default())
            .await
            .map_err(map_err)?,
    )
}

pub async fn allocation_label(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .allocation_by_label(c.uid, portfolio_id, PortfolioViewOptions::default())
            .await
            .map_err(map_err)?,
    )
}

pub async fn rebuild_portfolio(portfolio_id: i64) -> Result<(), String> {
    let c = ctx().await?;
    c.svc
        .rebuild_portfolio(c.uid, portfolio_id)
        .await
        .map_err(map_err)?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(())
}

pub async fn refresh_prices(portfolio_id: i64) -> Result<(), String> {
    let c = ctx().await?;
    c.svc
        .refresh_prices(c.uid, portfolio_id)
        .await
        .map_err(map_err)?;
    Ok(())
}

pub async fn refresh_sip_transactions(portfolio_id: i64) -> Result<SipRefreshResult, String> {
    let c = ctx().await?;
    let result: SipRefreshResult = convert(
        c.svc
            .refresh_sip_transactions(c.uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )?;
    if !result.created.is_empty() || !result.registered.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub async fn scan_portfolio_refresh(portfolio_id: i64) -> Result<PortfolioRefreshScan, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .scan_portfolio_refresh(c.uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn apply_portfolio_refresh(
    portfolio_id: i64,
    selections: &[String],
) -> Result<PortfolioRefreshApplyResult, String> {
    let c = ctx().await?;
    let result: PortfolioRefreshApplyResult = convert(
        c.svc
            .apply_portfolio_refresh(c.uid, portfolio_id, selections)
            .await
            .map_err(map_err)?,
    )?;
    if result.corporate_actions_created > 0
        || result.sip_registered > 0
        || result.sip_materialized > 0
        || result.swp_registered > 0
        || result.swp_materialized > 0
    {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub async fn refresh_swp_transactions(portfolio_id: i64) -> Result<SwpRefreshResult, String> {
    let c = ctx().await?;
    let result: SwpRefreshResult = convert(
        c.svc
            .refresh_swp_transactions(c.uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )?;
    if !result.created.is_empty() || !result.registered.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub async fn register_mf_schedule(
    portfolio_id: i64,
    input: &RegisterMfSchedule,
) -> Result<RegisterMfScheduleResult, String> {
    let c = ctx().await?;
    let input: stocker_portfolio::RegisterMfSchedule = convert(input)?;
    let result: RegisterMfScheduleResult = convert(
        c.svc
            .register_mf_schedule(c.uid, portfolio_id, &input)
            .await
            .map_err(map_err)?,
    )?;
    if !result.registered.is_empty() || !result.materialized.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub async fn list_mf_schedules(portfolio_id: i64) -> Result<Vec<MfSchedule>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .list_mf_schedules(c.uid, portfolio_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn inactivate_mf_schedule(schedule_id: i64) -> Result<MfSchedule, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .inactivate_mf_schedule(c.uid, schedule_id)
            .await
            .map_err(map_err)?,
    )
}

pub async fn get_mf_scheme(scheme_code: i64) -> Result<MfSearchHit, String> {
    let c = ctx().await?;
    convert(c.svc.get_mf_scheme(scheme_code).await.map_err(map_err)?)
}

pub async fn fifo_lots(portfolio_id: i64, symbol: &str) -> Result<Vec<FifoLot>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .fifo_lots(c.uid, portfolio_id, symbol)
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

pub async fn parse_import_file(
    _portfolio_id: i64,
    filename: &str,
    bytes: &[u8],
) -> Result<ParsePreview, String> {
    let c = ctx().await?;
    c.svc
        .parse_import_file(bytes, filename)
        .map_err(map_err)
        .and_then(convert)
}

pub async fn preview_import(
    portfolio_id: i64,
    request: &ImportApplyRequest,
) -> Result<Vec<ImportRowPreview>, String> {
    let c = ctx().await?;
    let column_mapping: Vec<stocker_portfolio::ImportField> = convert(request.column_mapping.clone())?;
    let grid: stocker_portfolio::RawGrid = convert(request.grid.clone())?;
    Ok(convert(
        c.svc
            .preview_import(portfolio_id, request.header_row, &column_mapping, &grid)
            .await,
    )?)
}

pub async fn apply_import(
    portfolio_id: i64,
    request: &ImportApplyRequest,
) -> Result<ImportResult, String> {
    let c = ctx().await?;
    let column_mapping: Vec<stocker_portfolio::ImportField> = convert(request.column_mapping.clone())?;
    let grid: stocker_portfolio::RawGrid = convert(request.grid.clone())?;
    let result: ImportResult = convert(
        c.svc
            .import_transactions(
                c.uid,
                portfolio_id,
                request.header_row,
                &column_mapping,
                &grid,
            )
            .await
            .map_err(map_err)?,
    )?;
    if result.imported > 0 {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub async fn search_mutual_funds(query: &str) -> Result<Vec<MfSearchHit>, String> {
    let c = ctx().await?;
    convert(
        c.svc
            .search_mutual_funds(query)
            .await
            .map_err(map_err)?,
    )
}
