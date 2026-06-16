//! Public façade — portfolio service with screener enrichment.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sqlx::{Row, SqlitePool};
use stocker_core::fetcher::fetch_price;
use stocker_mf::{is_mutual_fund_symbol, parse_mf_symbol, MfSearchHit, MfService};
use stocker_screener::ScreenerService;

use crate::auth::ensure_local_user;
use crate::engine::{rebuild, RebuildResult};
use crate::returns::{self, ReturnMethod};
use crate::error::{Error, Result};
use crate::labels;
use crate::models::{
    holding_entity_id, AllocationRow, Dashboard, DeleteLabelResult, Holding, LabelEntityType,
    NewLabel, NewPortfolio, NewTransaction, Portfolio, PortfolioSummary, Transaction,
    UpdatePortfolio,
};
use crate::portfolios;
use crate::transactions::{self, TransactionFilter};
use crate::{db, models::Label};

#[derive(Clone)]
pub struct PortfolioService {
    pool: SqlitePool,
    screener: Option<Arc<ScreenerService>>,
    mf: Option<Arc<MfService>>,
}

impl PortfolioService {
    pub async fn open(
        path: &Path,
        screener: Option<Arc<ScreenerService>>,
        mf: Option<Arc<MfService>>,
    ) -> Result<Self> {
        let pool = db::open(path).await?;
        let svc = Self { pool, screener, mf };
        svc.ensure_local_user().await?;
        Ok(svc)
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

    pub async fn rebuild_portfolio(&self, user_id: i64, portfolio_id: i64) -> Result<RebuildResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        rebuild::rebuild(&self.pool, portfolio_id).await
    }

    pub async fn refresh_sip_transactions(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<crate::sip_refresh::SipRefreshResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        crate::sip_refresh::refresh_sip_transactions(
            &self.pool,
            self.mf.clone(),
            user_id,
            portfolio_id,
        )
        .await
    }

    // --- Analytics ---

    pub async fn holdings(&self, user_id: i64, portfolio_id: i64) -> Result<Vec<Holding>> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let rebuild_result = rebuild::rebuild(&self.pool, portfolio_id).await?;
        let txns = transactions::list(
            &self.pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
        )
        .await?;
        let mut holdings = Vec::new();

        for (symbol, stats) in &rebuild_result.symbols {
            if stats.quantity <= 1e-9 {
                continue;
            }
            let entity_id = holding_entity_id(portfolio_id, symbol);
            let label_list = labels::labels_for_entity(
                &self.pool,
                user_id,
                LabelEntityType::Holding,
                &entity_id,
            )
            .await?;
            let h = self
                .build_holding(symbol, stats, &label_list, &txns)
                .await;
            holdings.push(h);
        }

        let total_mv: f64 = holdings
            .iter()
            .filter_map(|h| h.current_value)
            .sum();
        for h in &mut holdings {
            if let Some(mv) = h.current_value {
                h.portfolio_weight = if total_mv > 0.0 {
                    Some(mv / total_mv * 100.0)
                } else {
                    Some(0.0)
                };
            }
        }

        holdings.sort_by(|a, b| {
            b.current_value
                .unwrap_or(0.0)
                .partial_cmp(&a.current_value.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(holdings)
    }

    pub async fn summary(&self, user_id: i64, portfolio_id: i64) -> Result<PortfolioSummary> {
        let holdings = self.holdings(user_id, portfolio_id).await?;
        let rebuild_result = rebuild::rebuild(&self.pool, portfolio_id).await?;
        let txns = transactions::list(
            &self.pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
        )
        .await?;

        let invested = rebuild_result.total_invested;
        let current_mv: f64 = holdings.iter().filter_map(|h| h.current_value).sum();
        let unrealized: f64 = holdings.iter().filter_map(|h| h.unrealized_gain).sum();
        let unrealized_pct = if invested > 0.0 {
            unrealized / invested * 100.0
        } else {
            0.0
        };

        let terminal_values: HashMap<String, f64> = holdings
            .iter()
            .filter_map(|h| h.current_value.map(|mv| (h.symbol.clone(), mv)))
            .collect();
        let portfolio_returns = returns::portfolio_metrics(&txns, &terminal_values);

        Ok(PortfolioSummary {
            portfolio_id,
            invested_amount: invested,
            current_market_value: current_mv,
            unrealized_gain: unrealized,
            unrealized_gain_pct: unrealized_pct,
            realized_gain: rebuild_result.total_realized,
            dividend_received: rebuild_result.total_dividend,
            total_return: portfolio_returns.total_return,
            total_return_pct: portfolio_returns.return_pct.unwrap_or(0.0),
            return_method: portfolio_returns
                .return_method
                .map(return_method_label),
            holdings_count: holdings.len(),
            rebuilt_at: rebuild_result.rebuilt_at,
        })
    }

    pub async fn dashboard(&self, user_id: i64, portfolio_id: i64) -> Result<Dashboard> {
        let portfolio = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let summary = self.summary(user_id, portfolio_id).await?;
        let mut top_holdings = self.holdings(user_id, portfolio_id).await?;
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
            summary,
            top_holdings,
            recent_transactions: recent,
        })
    }

    pub async fn allocation_by_stock(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<Vec<AllocationRow>> {
        let holdings = self.holdings(user_id, portfolio_id).await?;
        let total_mv: f64 = holdings.iter().filter_map(|h| h.current_value).sum();

        Ok(holdings
            .into_iter()
            .map(|h| {
                let mv = h.current_value.unwrap_or(0.0);
                let tr = h.total_return.unwrap_or(0.0);
                AllocationRow {
                    key: h.symbol.clone(),
                    label: h.short_name.clone().unwrap_or(h.symbol.clone()),
                    current_value: mv,
                    invested_amount: h.total_cost,
                    weight_pct: if total_mv > 0.0 {
                        mv / total_mv * 100.0
                    } else {
                        0.0
                    },
                    unrealized_gain: h.unrealized_gain.unwrap_or(0.0),
                    realized_gain: h.realized_gain,
                    dividend_received: h.dividend_received,
                    total_return: tr,
                    holdings_count: 1,
                }
            })
            .collect())
    }

    pub async fn allocation_by_label(
        &self,
        user_id: i64,
        portfolio_id: i64,
    ) -> Result<Vec<AllocationRow>> {
        let holdings = self.holdings(user_id, portfolio_id).await?;
        let total_mv: f64 = holdings.iter().filter_map(|h| h.current_value).sum();

        let mut by_label: HashMap<String, AllocationRow> = HashMap::new();

        for h in holdings {
            let mv = h.current_value.unwrap_or(0.0);
            let tr = h.total_return.unwrap_or(0.0);
            if h.labels.is_empty() {
                let entry = by_label.entry("Unlabeled".into()).or_insert(AllocationRow {
                    key: "unlabeled".into(),
                    label: "Unlabeled".into(),
                    current_value: 0.0,
                    invested_amount: 0.0,
                    weight_pct: 0.0,
                    unrealized_gain: 0.0,
                    realized_gain: 0.0,
                    dividend_received: 0.0,
                    total_return: 0.0,
                    holdings_count: 0,
                });
                entry.current_value += mv;
                entry.invested_amount += h.total_cost;
                entry.unrealized_gain += h.unrealized_gain.unwrap_or(0.0);
                entry.realized_gain += h.realized_gain;
                entry.dividend_received += h.dividend_received;
                entry.total_return += tr;
                entry.holdings_count += 1;
            } else {
                for label in &h.labels {
                    let entry = by_label.entry(label.name.clone()).or_insert(AllocationRow {
                        key: label.id.to_string(),
                        label: label.name.clone(),
                        current_value: 0.0,
                        invested_amount: 0.0,
                        weight_pct: 0.0,
                        unrealized_gain: 0.0,
                        realized_gain: 0.0,
                        dividend_received: 0.0,
                        total_return: 0.0,
                        holdings_count: 0,
                    });
                    entry.current_value += mv;
                    entry.invested_amount += h.total_cost;
                    entry.unrealized_gain += h.unrealized_gain.unwrap_or(0.0);
                    entry.realized_gain += h.realized_gain;
                    entry.dividend_received += h.dividend_received;
                    entry.total_return += tr;
                    entry.holdings_count += 1;
                }
            }
        }

        let mut rows: Vec<AllocationRow> = by_label.into_values().collect();
        for row in &mut rows {
            row.weight_pct = if total_mv > 0.0 {
                row.current_value / total_mv * 100.0
            } else {
                0.0
            };
        }
        rows.sort_by(|a, b| {
            b.current_value
                .partial_cmp(&a.current_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(rows)
    }

    pub async fn export_holdings_csv(&self, user_id: i64, portfolio_id: i64) -> Result<String> {
        let holdings = self.holdings(user_id, portfolio_id).await?;
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
        rebuild::rebuild(&self.pool, portfolio_id).await?;

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
        rebuild::rebuild(&self.pool, portfolio_id).await?;

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

    async fn resolve_symbol(&self, raw: &str) -> Result<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidInput("symbol is required".into()));
        }

        if is_mutual_fund_symbol(trimmed) {
            if let Some(mf) = &self.mf {
                if let Some(code) = parse_mf_symbol(trimmed) {
                    mf.ensure_scheme(code)
                        .await
                        .map_err(|e| Error::InvalidInput(e.to_string()))?;
                    return Ok(trimmed.to_string());
                }
            }
        }

        if let Some(screener) = &self.screener {
            if let Ok(resolved) = screener.resolve_symbol(trimmed).await {
                return Ok(resolved);
            }
        }

        if let Some(mf) = &self.mf {
            let code = mf
                .resolve_by_name(trimmed)
                .await
                .map_err(|e| Error::InvalidInput(e.to_string()))?;
            return Ok(stocker_mf::mf_symbol(code));
        }

        Ok(trimmed.to_uppercase())
    }

    async fn build_holding(
        &self,
        symbol: &str,
        stats: &rebuild::SymbolStats,
        label_list: &[Label],
        txns: &[Transaction],
    ) -> Holding {
        let avg_cost = if stats.quantity > 0.0 {
            stats.total_cost / stats.quantity
        } else {
            0.0
        };

        let (current_price, short_name, sector, industry, exchange, asset_class, nav_date) =
            self.enrich_symbol(symbol).await;

        let current_value = current_price.map(|p| p * stats.quantity);
        let unrealized = current_value.map(|mv| mv - stats.total_cost);
        let unrealized_pct = unrealized.map(|u| {
            if stats.total_cost > 0.0 {
                u / stats.total_cost * 100.0
            } else {
                0.0
            }
        });

        let return_metrics =
            returns::symbol_metrics(txns, symbol, current_value);
        let total_return = Some(return_metrics.total_return);
        let total_return_pct = return_metrics.return_pct;
        let return_method = return_metrics.return_method.map(return_method_label);

        Holding {
            symbol: symbol.to_string(),
            quantity: stats.quantity,
            average_cost: avg_cost,
            total_cost: stats.total_cost,
            current_price,
            current_value,
            unrealized_gain: unrealized,
            unrealized_gain_pct: unrealized_pct,
            realized_gain: stats.realized_gain,
            dividend_received: stats.dividend_received,
            total_return,
            total_return_pct,
            return_method,
            portfolio_weight: None,
            last_transaction_date: stats.last_transaction_date.clone(),
            short_name,
            sector,
            industry,
            exchange,
            asset_class,
            nav_date,
            labels: label_list.to_vec(),
        }
    }

    async fn enrich_symbol(
        &self,
        symbol: &str,
    ) -> (
        Option<f64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        if is_mutual_fund_symbol(symbol) {
            if let (Some(mf), Some(code)) = (&self.mf, parse_mf_symbol(symbol)) {
                if let Ok(nav) = mf.latest_nav(code).await {
                    return (
                        Some(nav.nav),
                        Some(nav.scheme_name),
                        nav.scheme_category,
                        None,
                        Some("MF".into()),
                        Some("mutual_fund".into()),
                        Some(nav.nav_date),
                    );
                }
            }
            return (
                None,
                None,
                None,
                None,
                Some("MF".into()),
                Some("mutual_fund".into()),
                None,
            );
        }

        if let Some(screener) = &self.screener {
            let quote_symbol = screener
                .resolve_symbol(symbol)
                .await
                .unwrap_or_else(|_| symbol.to_string());
            if let Ok(Some(row)) = screener.snapshot_for(symbol).await {
                let mut price = row
                    .metrics
                    .get("current_price")
                    .and_then(|v| v.as_f64())
                    .filter(|p| p.is_finite() && *p > 0.0);
                if price.is_none() {
                    let live = fetch_price(&quote_symbol).await;
                    if live > 0.0 {
                        price = Some(live);
                    }
                }
                return (
                    price,
                    row.short_name,
                    row.sector,
                    row.industry,
                    row.exchange,
                    Some("equity".into()),
                    None,
                );
            }
            let live = fetch_price(&quote_symbol).await;
            if live > 0.0 {
                return (
                    Some(live),
                    None,
                    None,
                    None,
                    None,
                    Some("equity".into()),
                    None,
                );
            }
        }
        (
            None,
            None,
            None,
            None,
            None,
            Some("equity".into()),
            None,
        )
    }
}

fn return_method_label(method: ReturnMethod) -> String {
    match method {
        ReturnMethod::Xirr => "xirr".to_string(),
        ReturnMethod::Cagr => "cagr".to_string(),
        ReturnMethod::Simple => "simple".to_string(),
    }
}
