//! Public façade — portfolio service with screener enrichment.

use std::path::Path;
use std::sync::Arc;

use sqlx::Row;
use sqlx::SqlitePool;
use stocker_mf::{MfSearchHit, MfService};
use stocker_screener::ScreenerService;

use crate::analytics::{allocations_by_label, allocations_by_stock, PortfolioViewOptions};
use crate::auth::ensure_local_user;
use crate::engine::{ensure_ledger, RebuildResult};
use crate::error::{Error, Result};
use crate::labels;
use crate::models::{
    AllocationRow, Dashboard, DeleteLabelResult, Holding, LabelEntityType, NewLabel,
    NewPortfolio, NewTransaction, Portfolio, PortfolioSummary, Transaction, UpdatePortfolio,
};
use crate::portfolios;
use crate::transactions::{self, TransactionFilter};
use crate::{db, models::Label};

#[derive(Clone)]
pub struct PortfolioService {
    pub(crate) pool: sqlx::SqlitePool,
    pub(crate) screener: Option<Arc<ScreenerService>>,
    pub(crate) mf: Option<Arc<MfService>>,
    read_only: bool,
}

impl PortfolioService {
    pub async fn open(
        path: &Path,
        screener: Option<Arc<ScreenerService>>,
        mf: Option<Arc<MfService>>,
    ) -> Result<Self> {
        let pool = db::open(path).await?;
        let svc = Self {
            pool,
            screener,
            mf,
            read_only: false,
        };
        svc.ensure_local_user().await?;
        Ok(svc)
    }

    /// Open an existing database read-only (no migrations, no local user bootstrap).
    pub async fn open_readonly(path: &Path) -> Result<Self> {
        let pool = db::open_existing_readonly(path).await?;
        Ok(Self {
            pool,
            screener: None,
            mf: None,
            read_only: true,
        })
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Returns the implicit local user (no login required).
    pub async fn local_user(&self) -> Result<crate::models::User> {
        ensure_local_user(&self.pool).await
    }

    async fn ensure_local_user(&self) -> Result<()> {
        ensure_local_user(&self.pool).await.map(|_| ())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Search mutual funds by name (mfapi.in).
    pub async fn search_mutual_funds(&self, query: &str) -> Result<Vec<MfSearchHit>> {
        let mf = self
            .mf
            .as_ref()
            .ok_or_else(|| Error::Other("mutual fund service unavailable".into()))?;
        mf.search(query)
            .await
            .map_err(|e| Error::Other(e.to_string()))
    }

    // --- Portfolios ---

    pub async fn list_portfolios(
        &self,
        user_id: i64,
        include_archived: bool,
    ) -> Result<Vec<Portfolio>> {
        portfolios::list(&self.pool, user_id, include_archived).await
    }

    pub async fn get_portfolio(&self, user_id: i64, id: i64) -> Result<Portfolio> {
        portfolios::get(&self.pool, user_id, id).await
    }

    pub async fn create_portfolio(&self, user_id: i64, input: &NewPortfolio) -> Result<Portfolio> {
        let p = portfolios::create(&self.pool, user_id, input).await?;
        labels::ensure_label_for_portfolio(&self.pool, user_id, p.id, &p.name).await?;
        Ok(p)
    }

    pub async fn update_portfolio(
        &self,
        user_id: i64,
        id: i64,
        input: &UpdatePortfolio,
    ) -> Result<Portfolio> {
        portfolios::update(&self.pool, user_id, id, input).await
    }

    pub async fn delete_portfolio(&self, user_id: i64, id: i64) -> Result<DeleteLabelResult> {
        let p = portfolios::get(&self.pool, user_id, id).await?;
        labels::delete_by_name(&self.pool, user_id, &p.name).await
    }

    // --- Labels ---

    pub async fn list_labels(&self, user_id: i64) -> Result<Vec<Label>> {
        labels::list(&self.pool, user_id).await
    }

    pub async fn create_label(&self, user_id: i64, input: &NewLabel) -> Result<Label> {
        labels::create(&self.pool, user_id, input).await
    }

    pub async fn update_label(&self, user_id: i64, id: i64, input: &NewLabel) -> Result<Label> {
        labels::update(&self.pool, user_id, id, input).await
    }

    pub async fn delete_label(&self, user_id: i64, id: i64) -> Result<DeleteLabelResult> {
        labels::delete(&self.pool, user_id, id).await
    }

    pub async fn attach_label(
        &self,
        user_id: i64,
        label_id: i64,
        entity_type: LabelEntityType,
        entity_id: &str,
    ) -> Result<()> {
        labels::attach(&self.pool, user_id, label_id, entity_type, entity_id).await
    }

    pub async fn detach_label(
        &self,
        user_id: i64,
        label_id: i64,
        entity_type: LabelEntityType,
        entity_id: &str,
    ) -> Result<()> {
        labels::detach(&self.pool, user_id, label_id, entity_type, entity_id).await
    }

    // --- Transactions ---

    pub async fn list_transactions(
        &self,
        user_id: i64,
        filter: &TransactionFilter,
    ) -> Result<Vec<Transaction>> {
        let mut txns = transactions::list(&self.pool, user_id, filter).await?;
        for t in &mut txns {
            t.labels = labels::labels_for_entity(
                &self.pool,
                user_id,
                LabelEntityType::Transaction,
                &t.id.to_string(),
            )
            .await?;
        }
        Ok(txns)
    }

    pub async fn get_transaction(&self, user_id: i64, id: i64) -> Result<Transaction> {
        let mut t = transactions::get(&self.pool, user_id, id).await?;
        t.labels = labels::labels_for_entity(
            &self.pool,
            user_id,
            LabelEntityType::Transaction,
            &t.id.to_string(),
        )
        .await?;
        Ok(t)
    }

    pub async fn create_transaction(
        &self,
        user_id: i64,
        input: &NewTransaction,
    ) -> Result<Transaction> {
        if let Some(sym) = &input.symbol {
            let resolved = self.resolve_symbol(sym).await?;
            let mut input = input.clone();
            input.symbol = Some(resolved);
            return transactions::create(&self.pool, user_id, &input).await;
        }
        transactions::create(&self.pool, user_id, input).await
    }

    pub async fn update_transaction(
        &self,
        user_id: i64,
        id: i64,
        input: &NewTransaction,
    ) -> Result<Transaction> {
        let mut input = input.clone();
        if let Some(sym) = &input.symbol {
            input.symbol = Some(self.resolve_symbol(sym).await?);
        }
        transactions::update(&self.pool, user_id, id, &input).await
    }

    pub async fn delete_transaction(&self, user_id: i64, id: i64) -> Result<()> {
        transactions::delete(&self.pool, user_id, id).await
    }

    pub async fn clear_portfolio_transactions(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<crate::models::ClearTransactionsResult> {
        let deleted =
            transactions::delete_all_for_portfolio(&self.pool, user_id, portfolio_id).await?;
        Ok(crate::models::ClearTransactionsResult {
            transactions_deleted: deleted,
        })
    }

    pub async fn duplicate_transaction(&self, user_id: i64, id: i64) -> Result<Transaction> {
        transactions::duplicate(&self.pool, user_id, id).await
    }

    pub fn parse_import_file(&self, bytes: &[u8], filename: &str) -> Result<crate::import::ParsePreview> {
        let grid = crate::import::parse_file(bytes, filename)?;
        Ok(crate::import::build_preview(&grid))
    }

    pub async fn preview_import(
        &self,
        portfolio_id: i64,
        header_row: usize,
        column_mapping: &[crate::import::ImportField],
        grid: &crate::import::RawGrid,
    ) -> Vec<crate::import::ImportRowPreview> {
        let mf_index = if let Some(mf) = &self.mf {
            mf.ensure_scheme_index_cache().await.unwrap_or_default()
        } else {
            stocker_mf::load_scheme_index_from_file(&stocker_mf::default_scheme_list_cache_path())
                .unwrap_or_default()
        };
        crate::import::preview_rows_with_index(
            portfolio_id,
            header_row,
            column_mapping,
            grid,
            &mf_index,
        )
    }

    pub async fn import_transactions(
        &self,
        user_id: i64,
        portfolio_id: i64,
        header_row: usize,
        column_mapping: &[crate::import::ImportField],
        grid: &crate::import::RawGrid,
    ) -> Result<crate::import::ImportResult> {
        let mf_index = if let Some(mf) = &self.mf {
            mf.ensure_scheme_index_cache().await.unwrap_or_default()
        } else {
            stocker_mf::load_scheme_index_from_file(&stocker_mf::default_scheme_list_cache_path())
                .unwrap_or_default()
        };
        let resolver_rows = crate::import::preview_rows_with_index(
            portfolio_id,
            header_row,
            column_mapping,
            grid,
            &mf_index,
        );

        if let Some(mf) = &self.mf {
            for row in &resolver_rows {
                if let Some(txn) = &row.transaction {
                    if let Some(sym) = txn.symbol.as_deref() {
                        if let Some(code) = stocker_mf::parse_mf_symbol(sym) {
                            let _ = mf.ensure_scheme(code).await;
                        }
                    }
                }
            }
        }

        let txns: Vec<NewTransaction> = resolver_rows
            .into_iter()
            .filter_map(|r| r.transaction)
            .collect();
        crate::import::bulk_import_with_mf(
            &self.pool,
            user_id,
            portfolio_id,
            txns,
            self.mf.clone(),
        )
        .await
    }

    pub async fn refresh_sip_transactions(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<crate::sip_refresh::SipRefreshResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let schedule = crate::mf_schedule::refresh_active_schedules(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
            Some(crate::models::ScheduleType::Sip),
        )
        .await?;
        let mut result = crate::sip_refresh::refresh_sip_transactions(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
        )
        .await?;
        merge_schedule_refresh_into_sip_result(&mut result, schedule);
        Ok(result)
    }

    pub async fn refresh_swp_transactions(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<crate::swp_refresh::SwpRefreshResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let schedule = crate::mf_schedule::refresh_active_schedules(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
            Some(crate::models::ScheduleType::Swp),
        )
        .await?;
        let mut result = crate::swp_refresh::refresh_swp_transactions(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
        )
        .await?;
        merge_schedule_refresh_into_swp_result(&mut result, schedule);
        Ok(result)
    }

    pub async fn register_mf_schedule(
        &self,
        user_id: i64,
        portfolio_id: i64,
        input: &crate::models::RegisterMfSchedule,
    ) -> Result<crate::models::RegisterMfScheduleResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let symbol = self.resolve_symbol(&input.symbol).await?;
        crate::mf_schedule::register_mf_schedule(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
            &symbol,
            input,
        )
        .await
    }

    pub async fn list_mf_schedules(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<Vec<crate::models::MfSchedule>> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        crate::mf_schedule::list_mf_schedules(
            &self.pool,
            user_id,
            portfolio_id,
            self.mf.as_deref(),
        )
        .await
    }

    pub async fn inactivate_mf_schedule(
        &self,
        user_id: i64,
        schedule_id: i64,
    ) -> Result<crate::models::MfSchedule> {
        crate::mf_schedule::inactivate_schedule(&self.pool, user_id, schedule_id).await
    }

    pub async fn get_mf_scheme(&self, scheme_code: i64) -> Result<stocker_mf::MfSearchHit> {
        let mf = self
            .mf
            .as_ref()
            .ok_or_else(|| Error::Other("mutual fund service unavailable".into()))?;
        let meta = mf.load_scheme_meta(scheme_code).await.map_err(|e| Error::Other(e.to_string()))?;
        Ok(stocker_mf::MfSearchHit {
            scheme_code: meta.scheme_code,
            scheme_name: meta.scheme_name,
        })
    }

    pub async fn scan_portfolio_refresh(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<crate::portfolio_refresh::PortfolioRefreshScan> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        crate::portfolio_refresh::scan_portfolio_refresh(&self.pool, user_id, portfolio_id).await
    }

    pub async fn apply_portfolio_refresh(
        &self,
        user_id: i64,
        portfolio_id: i64,
        selections: &[String],
    ) -> Result<crate::portfolio_refresh::PortfolioRefreshApplyResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        crate::portfolio_refresh::apply_portfolio_refresh(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
            selections,
        )
        .await
    }

    pub async fn rebuild_portfolio(&self, user_id: i64, portfolio_id: i64) -> Result<RebuildResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        ensure_ledger(&self.pool, portfolio_id, true).await
    }

    pub async fn refresh_prices(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<PortfolioSummary> {
        Ok(self
            .get_portfolio_view(
                user_id,
                portfolio_id,
                PortfolioViewOptions {
                    force_refresh_prices: true,
                    ..Default::default()
                },
            )
            .await?
            .summary)
    }

    // --- Analytics ---

    pub async fn holdings(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<Vec<Holding>> {
        Ok(self
            .get_portfolio_view(user_id, portfolio_id, opts)
            .await?
            .holdings)
    }

    pub async fn summary(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<PortfolioSummary> {
        Ok(self
            .get_portfolio_view(user_id, portfolio_id, opts)
            .await?
            .summary)
    }

    pub async fn dashboard(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<Dashboard> {
        let portfolio = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let view = self.get_portfolio_view(user_id, portfolio_id, opts).await?;
        let mut top_holdings = view.holdings;
        top_holdings.truncate(5);

        let recent = self.list_transactions(
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                limit: Some(10),
                ..Default::default()
            },
        )
        .await?;

        Ok(Dashboard {
            portfolio,
            summary: view.summary,
            top_holdings,
            recent_transactions: recent,
        })
    }

    pub async fn allocation_by_stock(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<Vec<AllocationRow>> {
        let holdings = self.holdings(user_id, portfolio_id, opts).await?;
        Ok(allocations_by_stock(&holdings))
    }

    pub async fn allocation_by_label(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<Vec<AllocationRow>> {
        let holdings = self.holdings(user_id, portfolio_id, opts).await?;
        Ok(allocations_by_label(&holdings))
    }

    pub async fn export_holdings_csv(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<String> {
        let holdings = self.holdings(user_id, portfolio_id, opts).await?;
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record([
            "symbol",
            "quantity",
            "average_cost",
            "total_cost",
            "current_price",
            "current_value",
            "unrealized_gain",
            "realized_gain",
            "dividend_received",
            "total_return",
        ])
        .map_err(|e| Error::Other(format!("csv: {e}")))?;
        for h in holdings {
            wtr.write_record([
                h.symbol,
                h.quantity.to_string(),
                h.average_cost.to_string(),
                h.total_cost.to_string(),
                h.current_price.map(|p| p.to_string()).unwrap_or_default(),
                h.current_value.map(|v| v.to_string()).unwrap_or_default(),
                h.unrealized_gain
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                h.realized_gain.to_string(),
                h.dividend_received.to_string(),
                h.total_return.map(|v| v.to_string()).unwrap_or_default(),
            ])
            .map_err(|e| Error::Other(format!("csv: {e}")))?;
        }
        let bytes = wtr.into_inner().map_err(|e| Error::Other(format!("csv: {e}")))?;
        String::from_utf8(bytes).map_err(|e| Error::Other(format!("csv utf8: {e}")))
    }

    pub async fn export_transactions_csv(
        &self,
        user_id: i64,
        filter: &TransactionFilter,
    ) -> Result<String> {
        let txns = self.list_transactions(user_id, filter).await?;
        transactions::export_csv(&txns)
    }

    pub async fn fifo_lots(
        &self,
        user_id: i64,
        portfolio_id: i64,
        symbol: &str,
    ) -> Result<Vec<crate::models::FifoLot>> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let symbol = self.resolve_symbol(symbol).await?;
        if !self.read_only {
            ensure_ledger(&self.pool, portfolio_id, false).await?;
        }

        let rows = sqlx::query(
            "SELECT id, portfolio_id, symbol, source_transaction_id, acquired_date,
             original_quantity, remaining_quantity, total_cost, cost_per_share
             FROM fifo_lots WHERE portfolio_id = ? AND symbol = ?",
        )
        .bind(portfolio_id)
        .bind(&symbol)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                Ok(crate::models::FifoLot {
                    id: row.try_get("id")?,
                    portfolio_id: row.try_get("portfolio_id")?,
                    symbol: row.try_get("symbol")?,
                    source_transaction_id: row.try_get("source_transaction_id")?,
                    acquired_date: row.try_get("acquired_date")?,
                    original_quantity: row.try_get("original_quantity")?,
                    remaining_quantity: row.try_get("remaining_quantity")?,
                    total_cost: row.try_get("total_cost")?,
                    cost_per_share: row.try_get("cost_per_share")?,
                })
            })
            .collect()
    }

    pub async fn realized_matches(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<Vec<crate::models::RealizedMatch>> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        if !self.read_only {
            ensure_ledger(&self.pool, portfolio_id, false).await?;
        }

        let rows = sqlx::query(
            "SELECT id, portfolio_id, sell_transaction_id, buy_transaction_id, symbol, quantity,
             buy_date, sell_date, buy_cost_per_share, sell_price, cost_basis, sell_value, realized_gain
             FROM realized_matches WHERE portfolio_id = ? ORDER BY sell_date DESC",
        )
        .bind(portfolio_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                Ok(crate::models::RealizedMatch {
                    id: row.try_get("id")?,
                    portfolio_id: row.try_get("portfolio_id")?,
                    sell_transaction_id: row.try_get("sell_transaction_id")?,
                    buy_transaction_id: row.try_get("buy_transaction_id")?,
                    symbol: row.try_get("symbol")?,
                    quantity: row.try_get("quantity")?,
                    buy_date: row.try_get("buy_date")?,
                    sell_date: row.try_get("sell_date")?,
                    buy_cost_per_share: row.try_get("buy_cost_per_share")?,
                    sell_price: row.try_get("sell_price")?,
                    cost_basis: row.try_get("cost_basis")?,
                    sell_value: row.try_get("sell_value")?,
                    realized_gain: row.try_get("realized_gain")?,
                })
            })
            .collect()
    }
}

fn merge_schedule_refresh_into_sip_result(
    result: &mut crate::sip_refresh::SipRefreshResult,
    schedule: crate::models::RegisterMfScheduleResult,
) {
    result.registered = schedule.registered;
    for buy_id in schedule.materialized {
        if !result.created.contains(&buy_id) {
            result.created.push(buy_id);
        }
    }
    for failure in schedule.failed {
        result.failed.push(crate::sip_refresh::SipRefreshFailure {
            sip_id: 0,
            symbol: None,
            trade_date: failure.trade_date,
            reason: failure.reason,
        });
    }
}

fn merge_schedule_refresh_into_swp_result(
    result: &mut crate::swp_refresh::SwpRefreshResult,
    schedule: crate::models::RegisterMfScheduleResult,
) {
    result.registered = schedule.registered;
    for sell_id in schedule.materialized {
        if !result.created.contains(&sell_id) {
            result.created.push(sell_id);
        }
    }
    for failure in schedule.failed {
        result.failed.push(crate::swp_refresh::SwpRefreshFailure {
            swp_id: 0,
            symbol: None,
            trade_date: failure.trade_date,
            reason: failure.reason,
        });
    }
}
