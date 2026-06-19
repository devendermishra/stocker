//! Portfolio snapshot cache — ledger stats and valuation (holdings + prices).

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::{Holding, PortfolioSummary};

use super::rebuild::{self, RebuildResult, SymbolStats};

pub const STOCK_PRICE_TTL_SECS: i64 = 3600;
pub const MF_PRICE_TTL_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPrice {
    pub price: f64,
    pub priced_at: i64,
    pub asset_class: String,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub nav_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValuationSnapshot {
    pub ledger_rebuilt_at: i64,
    pub holdings: Vec<Holding>,
    pub summary: PortfolioSummary,
    pub symbol_prices: HashMap<String, SymbolPrice>,
    pub priced_at: i64,
}

#[derive(Debug, Deserialize)]
struct LedgerSnapshotJson {
    invested_amount: f64,
    realized_gain: f64,
    dividend_received: f64,
    symbols: HashMap<String, SymbolStats>,
}

/// Whether transactions changed since the last ledger rebuild.
pub async fn ledger_dirty(pool: &SqlitePool, portfolio_id: i64) -> Result<bool> {
    let row = sqlx::query(
        "SELECT rebuilt_at FROM portfolio_snapshots WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(true);
    };
    let rebuilt_at: i64 = row.try_get("rebuilt_at")?;

    let max_updated: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(updated_at) FROM transactions WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .fetch_one(pool)
    .await?;

    match max_updated {
        None => Ok(false),
        Some(ts) => Ok(ts > rebuilt_at),
    }
}

/// Rebuild FIFO ledger only when dirty or forced; otherwise load cached stats.
pub async fn ensure_ledger(
    pool: &SqlitePool,
    portfolio_id: i64,
    force_rebuild: bool,
) -> Result<RebuildResult> {
    if force_rebuild || ledger_dirty(pool, portfolio_id).await? {
        return rebuild::rebuild(pool, portfolio_id).await;
    }
    load_ledger_stats(pool, portfolio_id).await
}

pub async fn load_ledger_stats(pool: &SqlitePool, portfolio_id: i64) -> Result<RebuildResult> {
    let row = sqlx::query(
        "SELECT summary_json, rebuilt_at FROM portfolio_snapshots WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| Error::Other("portfolio snapshot missing".into()))?;

    let summary_json: String = row.try_get("summary_json")?;
    let rebuilt_at: i64 = row.try_get("rebuilt_at")?;
    let parsed: LedgerSnapshotJson = serde_json::from_str(&summary_json)
        .map_err(|e| Error::Other(format!("invalid ledger snapshot: {e}")))?;

    Ok(RebuildResult {
        portfolio_id,
        symbols: parsed.symbols,
        total_invested: parsed.invested_amount,
        total_realized: parsed.realized_gain,
        total_dividend: parsed.dividend_received,
        rebuilt_at,
    })
}

pub async fn load_valuation(
    pool: &SqlitePool,
    portfolio_id: i64,
) -> Result<Option<ValuationSnapshot>> {
    let row = sqlx::query(
        "SELECT rebuilt_at, holdings_json, valuation_summary_json, priced_at, symbol_prices_json
         FROM portfolio_snapshots WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let holdings_json: Option<String> = row.try_get("holdings_json")?;
    let summary_json: Option<String> = row.try_get("valuation_summary_json")?;
    let symbol_prices_json: Option<String> = row.try_get("symbol_prices_json")?;

    let (Some(holdings_json), Some(summary_json)) = (holdings_json, summary_json) else {
        return Ok(None);
    };

    let holdings: Vec<Holding> = serde_json::from_str(&holdings_json)
        .map_err(|e| Error::Other(format!("invalid holdings snapshot: {e}")))?;
    let summary: PortfolioSummary = serde_json::from_str(&summary_json)
        .map_err(|e| Error::Other(format!("invalid summary snapshot: {e}")))?;
    let symbol_prices: HashMap<String, SymbolPrice> = symbol_prices_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| Error::Other(format!("invalid symbol prices snapshot: {e}")))?
        .unwrap_or_default();

    Ok(Some(ValuationSnapshot {
        ledger_rebuilt_at: row.try_get("rebuilt_at")?,
        holdings,
        summary,
        symbol_prices,
        priced_at: row.try_get("priced_at")?,
    }))
}

pub async fn save_valuation(
    pool: &SqlitePool,
    portfolio_id: i64,
    ledger_rebuilt_at: i64,
    holdings: &[Holding],
    summary: &PortfolioSummary,
    symbol_prices: &HashMap<String, SymbolPrice>,
    priced_at: i64,
) -> Result<()> {
    let holdings_json = serde_json::to_string(holdings)
        .map_err(|e| Error::Other(format!("serialize holdings: {e}")))?;
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| Error::Other(format!("serialize summary: {e}")))?;
    let symbol_prices_json = serde_json::to_string(symbol_prices)
        .map_err(|e| Error::Other(format!("serialize symbol prices: {e}")))?;

    sqlx::query(
        "UPDATE portfolio_snapshots SET holdings_json = ?, valuation_summary_json = ?,
         priced_at = ?, symbol_prices_json = ?, rebuilt_at = ?
         WHERE portfolio_id = ?",
    )
    .bind(&holdings_json)
    .bind(&summary_json)
    .bind(priced_at)
    .bind(&symbol_prices_json)
    .bind(ledger_rebuilt_at)
    .bind(portfolio_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Clear enriched valuation columns after ledger changes.
pub async fn clear_valuation(pool: &SqlitePool, portfolio_id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE portfolio_snapshots SET holdings_json = NULL, valuation_summary_json = NULL,
         symbol_prices_json = NULL, priced_at = 0 WHERE portfolio_id = ?",
    )
    .bind(portfolio_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub fn price_ttl_for(asset_class: &str) -> i64 {
    if asset_class == "mutual_fund" {
        MF_PRICE_TTL_SECS
    } else {
        STOCK_PRICE_TTL_SECS
    }
}

pub fn is_price_fresh(entry: &SymbolPrice, now: i64) -> bool {
    now.saturating_sub(entry.priced_at) <= price_ttl_for(&entry.asset_class)
}

pub fn prices_fresh(
    symbol_prices: &HashMap<String, SymbolPrice>,
    symbols: &[String],
    now: i64,
    force_refresh: bool,
) -> bool {
    if force_refresh {
        return false;
    }
    for sym in symbols {
        let Some(entry) = symbol_prices.get(sym) else {
            return false;
        };
        if !is_price_fresh(entry, now) {
            return false;
        }
    }
    true
}

pub fn valuation_cache_valid(
    cached: &ValuationSnapshot,
    ledger_rebuilt_at: i64,
    symbols: &[String],
    force_refresh_prices: bool,
) -> bool {
    if cached.ledger_rebuilt_at != ledger_rebuilt_at {
        return false;
    }
    let now = Utc::now().timestamp();
    prices_fresh(&cached.symbol_prices, symbols, now, force_refresh_prices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_ttl_by_asset_class() {
        assert_eq!(price_ttl_for("equity"), STOCK_PRICE_TTL_SECS);
        assert_eq!(price_ttl_for("mutual_fund"), MF_PRICE_TTL_SECS);
    }

    #[test]
    fn is_price_fresh_respects_ttl() {
        let now = 1_700_000_000i64;
        let mut entry = SymbolPrice {
            price: 100.0,
            priced_at: now - 1800,
            asset_class: "equity".into(),
            short_name: None,
            sector: None,
            industry: None,
            exchange: None,
            nav_date: None,
        };
        assert!(is_price_fresh(&entry, now));
        entry.priced_at = now - STOCK_PRICE_TTL_SECS - 1;
        assert!(!is_price_fresh(&entry, now));
    }

    #[test]
    fn prices_fresh_requires_all_symbols() {
        let now = 1_700_000_000i64;
        let mut prices = HashMap::new();
        prices.insert(
            "RELIANCE".into(),
            SymbolPrice {
                price: 100.0,
                priced_at: now,
                asset_class: "equity".into(),
                short_name: None,
                sector: None,
                industry: None,
                exchange: None,
                nav_date: None,
            },
        );
        assert!(!prices_fresh(
            &prices,
            &["RELIANCE".into(), "TCS".into()],
            now,
            false
        ));
        assert!(prices_fresh(&prices, &["RELIANCE".into()], now, false));
        assert!(!prices_fresh(&prices, &["RELIANCE".into()], now, true));
    }

    #[tokio::test]
    async fn ensure_ledger_skips_rebuild_when_clean() {
        use crate::db;
        use crate::models::{NewPortfolio, NewTransaction, TransactionType};
        use crate::portfolios;

        let pool = db::open_memory().await.unwrap();
        let user = crate::auth::ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "Test".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let txn = NewTransaction {
            portfolio_id: portfolio.id,
            txn_type: TransactionType::Buy,
            trade_date: "2024-01-01".into(),
            symbol: Some("RELIANCE".into()),
            quantity: Some(10.0),
            price: Some(100.0),
            gross_amount: Some(1000.0),
            brokerage: None,
            taxes: None,
            net_amount: Some(1000.0),
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
            schedule_id: None,
        };
        crate::transactions::create(&pool, user.id, &txn).await.unwrap();

        assert!(!super::ledger_dirty(&pool, portfolio.id).await.unwrap());

        let first = ensure_ledger(&pool, portfolio.id, false).await.unwrap();
        let second = ensure_ledger(&pool, portfolio.id, false).await.unwrap();
        assert_eq!(first.rebuilt_at, second.rebuilt_at);
        assert!(!super::ledger_dirty(&pool, portfolio.id).await.unwrap());
    }
}
