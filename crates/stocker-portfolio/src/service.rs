//! Public façade — portfolio service with screener enrichment.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use sqlx::{Row, SqlitePool};
use stocker_screener::ScreenerService;

use crate::auth::ensure_local_user;
use crate::engine::{rebuild, RebuildResult};
use crate::error::{Error, Result};
use crate::labels;
use crate::models::{
    holding_entity_id, AllocationRow, Dashboard, Holding, LabelEntityType, NewLabel,
    NewPortfolio, NewTransaction, Portfolio, PortfolioSummary, Transaction, UpdatePortfolio,
};
use crate::portfolios;
use crate::transactions::{self, TransactionFilter};
use crate::{db, models::Label};

#[derive(Clone)]
pub struct PortfolioService {
    pool: SqlitePool,
    screener: Option<Arc<ScreenerService>>,
}

impl PortfolioService {
    pub async fn open(path: &Path, screener: Option<Arc<ScreenerService>>) -> Result<Self> {
        let pool = db::open(path).await?;
        let svc = Self { pool, screener };
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
        portfolios::create(&self.pool, user_id, input).await
    }

    pub async fn update_portfolio(
        &self,
        user_id: i64,
        id: i64,
        input: &UpdatePortfolio,
    ) -> Result<Portfolio> {
        portfolios::update(&self.pool, user_id, id, input).await
    }

    pub async fn delete_portfolio(&self, user_id: i64, id: i64) -> Result<()> {
        portfolios::delete(&self.pool, user_id, id).await
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

    pub async fn delete_label(&self, user_id: i64, id: i64) -> Result<()> {
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

    pub async fn duplicate_transaction(&self, user_id: i64, id: i64) -> Result<Transaction> {
        transactions::duplicate(&self.pool, user_id, id).await
    }

    pub async fn rebuild_portfolio(&self, user_id: i64, portfolio_id: i64) -> Result<RebuildResult> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        rebuild::rebuild(&self.pool, portfolio_id).await
    }

    // --- Analytics ---

    pub async fn holdings(&self, user_id: i64, portfolio_id: i64) -> Result<Vec<Holding>> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let rebuild_result = rebuild::rebuild(&self.pool, portfolio_id).await?;
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
            let mut h = self.build_holding(symbol, stats, &label_list).await;
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

        let invested = rebuild_result.total_invested;
        let current_mv: f64 = holdings.iter().filter_map(|h| h.current_value).sum();
        let unrealized: f64 = holdings.iter().filter_map(|h| h.unrealized_gain).sum();
        let unrealized_pct = if invested > 0.0 {
            unrealized / invested * 100.0
        } else {
            0.0
        };
        let total_return =
            unrealized + rebuild_result.total_realized + rebuild_result.total_dividend;
        let total_return_pct = if invested > 0.0 {
            total_return / invested * 100.0
        } else {
            0.0
        };

        Ok(PortfolioSummary {
            portfolio_id,
            invested_amount: invested,
            current_market_value: current_mv,
            unrealized_gain: unrealized,
            unrealized_gain_pct: unrealized_pct,
            realized_gain: rebuild_result.total_realized,
            dividend_received: rebuild_result.total_dividend,
            total_return,
            total_return_pct,
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
        if let Some(screener) = &self.screener {
            return screener
                .resolve_symbol(raw)
                .await
                .map_err(|e| Error::InvalidInput(e.to_string()));
        }
        Ok(raw.trim().to_uppercase())
    }

    async fn build_holding(
        &self,
        symbol: &str,
        stats: &rebuild::SymbolStats,
        label_list: &[Label],
    ) -> Holding {
        let avg_cost = if stats.quantity > 0.0 {
            stats.total_cost / stats.quantity
        } else {
            0.0
        };

        let (current_price, short_name, sector, industry, exchange) =
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
        let total_return = unrealized.map(|u| {
            u + stats.realized_gain + stats.dividend_received
        });
        let total_return_pct = total_return.map(|tr| {
            if stats.total_cost > 0.0 {
                tr / stats.total_cost * 100.0
            } else {
                0.0
            }
        });

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
            portfolio_weight: None,
            last_transaction_date: stats.last_transaction_date.clone(),
            short_name,
            sector,
            industry,
            exchange,
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
    ) {
        if let Some(screener) = &self.screener {
            if let Ok(Some(row)) = screener.snapshot_for(symbol).await {
                let price = row
                    .metrics
                    .get("current_price")
                    .and_then(|v| v.as_f64());
                return (
                    price,
                    row.short_name,
                    row.sector,
                    row.industry,
                    row.exchange,
                );
            }
        }
        (None, None, None, None, None)
    }
}
