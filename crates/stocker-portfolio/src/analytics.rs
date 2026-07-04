//! Portfolio analytics — single read pipeline with snapshot caching.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::SqlitePool;

use crate::engine::snapshot::{
    self, ensure_ledger, load_valuation, save_valuation, SymbolPrice, STOCK_PRICE_TTL_SECS,
};
use crate::engine::{RebuildResult, SymbolStats};
use crate::error::Result;
use crate::labels;
use crate::models::{
    holding_entity_id, AllocationRow, Holding, LabelEntityType, PortfolioSummary, Transaction,
};
use crate::returns::{self, ReturnMethod};
use stocker_core::fetcher::fetch_price;
use stocker_mf::{is_mutual_fund_symbol, parse_mf_symbol};
use stocker_screener::snapshot_is_fresh;
use crate::portfolios;
use crate::transactions::{self, TransactionFilter};
use crate::models::Label as LabelModel;

use super::PortfolioService;

/// Options controlling ledger rebuild and price refresh for portfolio views.
#[derive(Debug, Clone, Default)]
pub struct PortfolioViewOptions {
    pub force_rebuild: bool,
    pub force_refresh_prices: bool,
}

/// Fully computed portfolio view — holdings, summary, and ledger stats.
#[derive(Debug, Clone)]
pub struct PortfolioView {
    pub ledger: RebuildResult,
    pub holdings: Vec<Holding>,
    pub summary: PortfolioSummary,
}

impl PortfolioService {
    pub async fn get_portfolio_view(
        &self,
        user_id: i64,
        portfolio_id: i64,
        opts: PortfolioViewOptions,
    ) -> Result<PortfolioView> {
        let _ = portfolios::get(&self.pool, user_id, portfolio_id).await?;
        let ledger = if self.is_read_only() {
            if opts.force_rebuild {
                return Err(crate::error::Error::Other(
                    "cannot rebuild a read-only portfolio database".into(),
                ));
            }
            snapshot::load_ledger_stats(&self.pool, portfolio_id).await?
        } else {
            ensure_ledger(&self.pool, portfolio_id, opts.force_rebuild).await?
        };

        let active_symbols: Vec<String> = ledger
            .symbols
            .iter()
            .filter(|(_, s)| s.quantity > 1e-9)
            .map(|(sym, _)| sym.clone())
            .collect();

        if let Some(cached) = load_valuation(&self.pool, portfolio_id).await? {
            if snapshot::valuation_cache_valid(
                &cached,
                ledger.rebuilt_at,
                &active_symbols,
                opts.force_refresh_prices,
            ) {
                let holdings = reload_holding_labels(
                    &self.pool,
                    user_id,
                    portfolio_id,
                    cached.holdings,
                )
                .await?;
                return Ok(PortfolioView {
                    ledger,
                    holdings,
                    summary: cached.summary,
                });
            }
        }

        let txns = transactions::list(
            &self.pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
        )
        .await?;

        let cached_prices = load_valuation(&self.pool, portfolio_id)
            .await?
            .map(|v| v.symbol_prices)
            .unwrap_or_default();

        let (mut holdings, symbol_prices) = build_holdings_from_ledger(
            self,
            user_id,
            portfolio_id,
            &ledger,
            &txns,
            &cached_prices,
            opts.force_refresh_prices,
        )
        .await?;

        apply_portfolio_weights(&mut holdings);
        sort_holdings_by_value(&mut holdings);

        let summary = build_summary(portfolio_id, &holdings, &ledger, &txns);

        if !self.is_read_only() {
            let priced_at = symbol_prices
                .values()
                .map(|p| p.priced_at)
                .max()
                .unwrap_or_else(|| Utc::now().timestamp());
            save_valuation(
                &self.pool,
                portfolio_id,
                ledger.rebuilt_at,
                &holdings,
                &summary,
                &symbol_prices,
                priced_at,
            )
            .await?;
        }

        Ok(PortfolioView {
            ledger,
            holdings,
            summary,
        })
    }

    pub(crate) async fn resolve_symbol(&self, raw: &str) -> Result<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::error::Error::InvalidInput("symbol is required".into()));
        }

        if is_mutual_fund_symbol(trimmed) {
            if let Some(mf) = &self.mf {
                if let Some(code) = parse_mf_symbol(trimmed) {
                    mf.ensure_scheme(code)
                        .await
                        .map_err(|e| crate::error::Error::InvalidInput(e.to_string()))?;
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
                .map_err(|e| crate::error::Error::InvalidInput(e.to_string()))?;
            return Ok(stocker_mf::mf_symbol(code));
        }

        Ok(trimmed.to_uppercase())
    }
}

pub fn total_market_value(holdings: &[Holding]) -> f64 {
    holdings.iter().filter_map(|h| h.current_value).sum()
}

pub fn apply_portfolio_weights(holdings: &mut [Holding]) {
    let total_mv = total_market_value(holdings);
    for h in holdings.iter_mut() {
        if let Some(mv) = h.current_value {
            h.portfolio_weight = Some(if total_mv > 0.0 {
                mv / total_mv * 100.0
            } else {
                0.0
            });
        }
    }
}

pub fn sort_holdings_by_value(holdings: &mut [Holding]) {
    holdings.sort_by(|a, b| {
        b.current_value
            .unwrap_or(0.0)
            .partial_cmp(&a.current_value.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

pub fn build_summary(
    portfolio_id: i64,
    holdings: &[Holding],
    ledger: &RebuildResult,
    txns: &[Transaction],
) -> PortfolioSummary {
    let invested = ledger.total_invested;
    let current_mv = total_market_value(holdings);
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
    let portfolio_returns = returns::portfolio_metrics(txns, &terminal_values);

    PortfolioSummary {
        portfolio_id,
        invested_amount: invested,
        current_market_value: current_mv,
        unrealized_gain: unrealized,
        unrealized_gain_pct: unrealized_pct,
        realized_gain: ledger.total_realized,
        dividend_received: ledger.total_dividend,
        total_return: portfolio_returns.total_return,
        total_return_pct: portfolio_returns.return_pct.unwrap_or(0.0),
        return_method: portfolio_returns
            .return_method
            .map(return_method_label),
        holdings_count: holdings.len(),
        rebuilt_at: ledger.rebuilt_at,
    }
}

fn holding_to_allocation_row(h: &Holding, total_mv: f64) -> AllocationRow {
    let mv = h.current_value.unwrap_or(0.0);
    AllocationRow {
        key: h.symbol.clone(),
        label: h.short_name.clone().unwrap_or_else(|| h.symbol.clone()),
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
        total_return: h.total_return.unwrap_or(0.0),
        holdings_count: 1,
    }
}

fn accumulate_allocation(row: &mut AllocationRow, h: &Holding) {
    let mv = h.current_value.unwrap_or(0.0);
    row.current_value += mv;
    row.invested_amount += h.total_cost;
    row.unrealized_gain += h.unrealized_gain.unwrap_or(0.0);
    row.realized_gain += h.realized_gain;
    row.dividend_received += h.dividend_received;
    row.total_return += h.total_return.unwrap_or(0.0);
    row.holdings_count += 1;
}

pub fn allocations_by_stock(holdings: &[Holding]) -> Vec<AllocationRow> {
    let total_mv = total_market_value(holdings);
    holdings.iter().map(|h| holding_to_allocation_row(h, total_mv)).collect()
}

pub fn allocations_by_label(holdings: &[Holding]) -> Vec<AllocationRow> {
    let total_mv = total_market_value(holdings);
    let mut by_label: HashMap<String, AllocationRow> = HashMap::new();

    for h in holdings {
        if h.labels.is_empty() {
            let entry = by_label.entry("Unlabeled".into()).or_insert_with(|| AllocationRow {
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
            accumulate_allocation(entry, h);
        } else {
            for label in &h.labels {
                let entry = by_label.entry(label.name.clone()).or_insert_with(|| AllocationRow {
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
                accumulate_allocation(entry, h);
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
    rows
}

async fn reload_holding_labels(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    mut holdings: Vec<Holding>,
) -> Result<Vec<Holding>> {
    for h in &mut holdings {
        let entity_id = holding_entity_id(portfolio_id, &h.symbol);
        h.labels = labels::labels_for_entity(
            pool,
            user_id,
            LabelEntityType::Holding,
            &entity_id,
        )
        .await?;
    }
    Ok(holdings)
}

async fn build_holdings_from_ledger(
    svc: &PortfolioService,
    user_id: i64,
    portfolio_id: i64,
    ledger: &RebuildResult,
    txns: &[Transaction],
    cached_prices: &HashMap<String, SymbolPrice>,
    force_refresh: bool,
) -> Result<(Vec<Holding>, HashMap<String, SymbolPrice>)> {
    let mut holdings = Vec::new();
    let mut symbol_prices = HashMap::new();
    for (symbol, stats) in &ledger.symbols {
        if stats.quantity <= 1e-9 {
            continue;
        }
        let entity_id = holding_entity_id(portfolio_id, symbol);
        let label_list = labels::labels_for_entity(
            &svc.pool,
            user_id,
            LabelEntityType::Holding,
            &entity_id,
        )
        .await?;
        let cached = cached_prices.get(symbol);
        let sp = resolve_symbol_price(svc, symbol, cached, force_refresh).await;
        symbol_prices.insert(symbol.clone(), sp.clone());
        let h = build_holding(symbol, stats, &label_list, txns, &sp);
        holdings.push(h);
    }
    Ok((holdings, symbol_prices))
}

fn build_holding(
    symbol: &str,
    stats: &SymbolStats,
    label_list: &[LabelModel],
    txns: &[Transaction],
    sp: &SymbolPrice,
) -> Holding {
    let avg_cost = if stats.quantity > 0.0 {
        stats.total_cost / stats.quantity
    } else {
        0.0
    };

    let current_price = Some(sp.price);
    let current_value = Some(sp.price * stats.quantity);
    let unrealized = current_value.map(|mv| mv - stats.total_cost);
    let unrealized_pct = unrealized.map(|u| {
        if stats.total_cost > 0.0 {
            u / stats.total_cost * 100.0
        } else {
            0.0
        }
    });

    let return_metrics = returns::symbol_metrics(txns, symbol, current_value);

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
        total_return: Some(return_metrics.total_return),
        total_return_pct: return_metrics.return_pct,
        return_method: return_metrics.return_method.map(return_method_label),
        portfolio_weight: None,
        last_transaction_date: stats.last_transaction_date.clone(),
        short_name: sp.short_name.clone(),
        sector: sp.sector.clone(),
        industry: sp.industry.clone(),
        exchange: sp.exchange.clone(),
        asset_class: Some(sp.asset_class.clone()),
        nav_date: sp.nav_date.clone(),
        labels: label_list.to_vec(),
    }
}

async fn resolve_symbol_price(
    svc: &PortfolioService,
    symbol: &str,
    cached: Option<&SymbolPrice>,
    force_refresh: bool,
) -> SymbolPrice {
    let now = Utc::now().timestamp();
    if let Some(cached) = cached {
        if !force_refresh && snapshot::is_price_fresh(cached, now) {
            return cached.clone();
        }
    }

    if is_mutual_fund_symbol(symbol) {
        if let (Some(mf), Some(code)) = (&svc.mf, parse_mf_symbol(symbol)) {
            if let Ok(nav) = mf.latest_nav(code).await {
                return SymbolPrice {
                    price: nav.nav,
                    priced_at: nav.fetched_at,
                    asset_class: "mutual_fund".into(),
                    short_name: Some(nav.scheme_name),
                    sector: nav.scheme_category,
                    industry: None,
                    exchange: Some("MF".into()),
                    nav_date: Some(nav.nav_date),
                };
            }
        }
        return SymbolPrice {
            price: cached.map(|c| c.price).unwrap_or(0.0),
            priced_at: now,
            asset_class: "mutual_fund".into(),
            short_name: cached.and_then(|c| c.short_name.clone()),
            sector: cached.and_then(|c| c.sector.clone()),
            industry: None,
            exchange: Some("MF".into()),
            nav_date: cached.and_then(|c| c.nav_date.clone()),
        };
    }

    if let Some(screener) = &svc.screener {
        let quote_symbol = screener
            .resolve_symbol(symbol)
            .await
            .unwrap_or_else(|_| symbol.to_string());
        if let Ok(Some(row)) = screener.snapshot_for(symbol).await {
            let screener_price = row
                .metrics
                .get("current_price")
                .and_then(|v| v.as_f64())
                .filter(|p| p.is_finite() && *p > 0.0);
            let use_screener = screener_price.is_some()
                && snapshot_is_fresh(row.updated_at, STOCK_PRICE_TTL_SECS);
            let (price, priced_at) = if use_screener {
                (screener_price.unwrap(), row.updated_at.unwrap_or(now))
            } else {
                let live = fetch_price(&quote_symbol).await;
                if live > 0.0 {
                    (live, now)
                } else if let Some(p) = screener_price {
                    (p, row.updated_at.unwrap_or(now))
                } else {
                    (cached.map(|c| c.price).unwrap_or(0.0), now)
                }
            };
            if price > 0.0 {
                return SymbolPrice {
                    price,
                    priced_at,
                    asset_class: "equity".into(),
                    short_name: row.short_name,
                    sector: row.sector,
                    industry: row.industry,
                    exchange: row.exchange,
                    nav_date: None,
                };
            }
        }
        let live = fetch_price(&quote_symbol).await;
        if live > 0.0 {
            return SymbolPrice {
                price: live,
                priced_at: now,
                asset_class: "equity".into(),
                short_name: cached.and_then(|c| c.short_name.clone()),
                sector: cached.and_then(|c| c.sector.clone()),
                industry: cached.and_then(|c| c.industry.clone()),
                exchange: cached.and_then(|c| c.exchange.clone()),
                nav_date: None,
            };
        }
    }

    SymbolPrice {
        price: cached.map(|c| c.price).unwrap_or(0.0),
        priced_at: now,
        asset_class: "equity".into(),
        short_name: cached.and_then(|c| c.short_name.clone()),
        sector: cached.and_then(|c| c.sector.clone()),
        industry: cached.and_then(|c| c.industry.clone()),
        exchange: cached.and_then(|c| c.exchange.clone()),
        nav_date: None,
    }
}

fn return_method_label(method: ReturnMethod) -> String {
    match method {
        ReturnMethod::Xirr => "xirr".to_string(),
        ReturnMethod::Cagr => "cagr".to_string(),
        ReturnMethod::Simple => "simple".to_string(),
    }
}
