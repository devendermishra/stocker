//! Replay transactions in order and materialize fifo_lots + realized_matches.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::{Transaction, TransactionType};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SymbolStats {
    pub quantity: f64,
    pub total_cost: f64,
    pub realized_gain: f64,
    pub dividend_received: f64,
    pub last_transaction_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RebuildResult {
    pub portfolio_id: i64,
    pub symbols: HashMap<String, SymbolStats>,
    pub total_invested: f64,
    pub total_realized: f64,
    pub total_dividend: f64,
    pub rebuilt_at: i64,
}

#[derive(Debug, Clone)]
struct Lot {
    source_txn_id: i64,
    acquired_date: String,
    remaining_qty: f64,
    total_cost: f64,
}

impl Lot {
    fn cost_per_share(&self) -> f64 {
        if self.remaining_qty <= 0.0 {
            0.0
        } else {
            self.total_cost / self.remaining_qty
        }
    }
}

#[derive(Debug, Default)]
struct SymbolState {
    lots: Vec<Lot>,
    realized_gain: f64,
    dividend_received: f64,
    last_transaction_date: Option<String>,
    applied_corp_actions: HashMap<String, ()>,
}

impl SymbolState {
    fn total_qty(&self) -> f64 {
        self.lots.iter().map(|l| l.remaining_qty).sum()
    }

    fn total_cost(&self) -> f64 {
        self.lots.iter().map(|l| l.total_cost).sum()
    }

    fn to_stats(&self) -> SymbolStats {
        SymbolStats {
            quantity: self.total_qty(),
            total_cost: self.total_cost(),
            realized_gain: self.realized_gain,
            dividend_received: self.dividend_received,
            last_transaction_date: self.last_transaction_date.clone(),
        }
    }
}

pub async fn rebuild(pool: &SqlitePool, portfolio_id: i64) -> Result<RebuildResult> {
    let txns = load_transactions(pool, portfolio_id).await?;

    let mut states: HashMap<String, SymbolState> = HashMap::new();
    let mut pending_lots: Vec<(String, Lot)> = Vec::new();
    let mut pending_matches: Vec<PendingMatch> = Vec::new();

    for txn in &txns {
        match txn.txn_type {
            TransactionType::OpeningBalance
            | TransactionType::Buy
            | TransactionType::MergerInvestment
            | TransactionType::DemergerInvestment
            | TransactionType::Rights => {
                let symbol = require_symbol(txn)?;
                let qty = require_positive_qty(txn.quantity, "quantity")?;
                let total_cost = buy_total_cost(txn)?;
                let state = states.entry(symbol.clone()).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());
                let lot = Lot {
                    source_txn_id: txn.id,
                    acquired_date: txn.trade_date.clone(),
                    remaining_qty: qty,
                    total_cost,
                };
                state.lots.push(lot.clone());
                pending_lots.push((symbol, lot));
            }
            TransactionType::Sell
            | TransactionType::MergerRedemption
            | TransactionType::DemergerRedemption => {
                let symbol = require_symbol(txn)?;
                let sell_qty = require_positive_qty(txn.quantity, "sell quantity")?;
                let state = states.entry(symbol.clone()).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());

                let available = state.total_qty();
                if sell_qty > available + 1e-9 {
                    return Err(Error::Ledger(format!(
                        "sell quantity {sell_qty} exceeds available {available} for {symbol}"
                    )));
                }

                let sell_price = resolve_sell_price(txn, sell_qty, &state);

                let mut remaining = sell_qty;
                for lot in &mut state.lots {
                    if remaining <= 1e-9 {
                        break;
                    }
                    if lot.remaining_qty <= 1e-9 {
                        continue;
                    }
                    let matched = remaining.min(lot.remaining_qty);
                    let cost_per = lot.cost_per_share();
                    let cost_basis = matched * cost_per;
                    let sell_value = matched * sell_price;
                    let gain = sell_value - cost_basis;

                    pending_matches.push(PendingMatch {
                        sell_transaction_id: txn.id,
                        buy_transaction_id: lot.source_txn_id,
                        symbol: symbol.clone(),
                        quantity: matched,
                        buy_date: lot.acquired_date.clone(),
                        sell_date: txn.trade_date.clone(),
                        buy_cost_per_share: cost_per,
                        sell_price,
                        cost_basis,
                        sell_value,
                        realized_gain: gain,
                    });

                    lot.remaining_qty -= matched;
                    lot.total_cost -= cost_basis;
                    remaining -= matched;
                    state.realized_gain += gain;
                }
            }
            TransactionType::Split => {
                let symbol = require_symbol(txn)?;
                let (num, den) = split_ratio(txn)?;
                if num <= 0.0 || den <= 0.0 {
                    return Err(Error::Ledger("invalid split ratio".into()));
                }
                let factor = num / den;
                if let Some(key) = &txn.corporate_action_key {
                    let state = states.entry(symbol.clone()).or_default();
                    if state.applied_corp_actions.contains_key(key) {
                        continue;
                    }
                    state.applied_corp_actions.insert(key.clone(), ());
                }
                let state = states.entry(symbol).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());
                for lot in &mut state.lots {
                    lot.remaining_qty *= factor;
                    // total_cost unchanged; cost_per_share decreases
                }
            }
            TransactionType::Bonus => {
                let symbol = require_symbol(txn)?;
                if let Some(key) = &txn.corporate_action_key {
                    let state = states.entry(symbol.clone()).or_default();
                    if state.applied_corp_actions.contains_key(key) {
                        continue;
                    }
                    state.applied_corp_actions.insert(key.clone(), ());
                }
                let state = states.entry(symbol.clone()).or_default();
                state.last_transaction_date = Some(txn.trade_date.clone());

                if let (Some(bonus_num), Some(bonus_den)) =
                    (txn.bonus_ratio_num, txn.bonus_ratio_den)
                {
                    if bonus_num <= 0.0 || bonus_den <= 0.0 {
                        return Err(Error::Ledger("invalid bonus ratio".into()));
                    }
                    let factor = 1.0 + bonus_num / bonus_den;
                    for lot in &mut state.lots {
                        lot.remaining_qty *= factor;
                        // total_cost unchanged
                    }
                } else if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
                    // Broker exports list bonus shares received without a ratio.
                    let lot = Lot {
                        source_txn_id: txn.id,
                        acquired_date: txn.trade_date.clone(),
                        remaining_qty: qty,
                        total_cost: 0.0,
                    };
                    state.lots.push(lot.clone());
                    pending_lots.push((symbol, lot));
                } else {
                    return Err(Error::Ledger("bonus ratio num required".into()));
                }
            }
            TransactionType::Dividend => {
                let symbol = require_symbol(txn)?;
                let gross = txn.gross_amount.unwrap_or_else(|| {
                    let dps = txn.dividend_per_share.unwrap_or(0.0);
                    let qty = txn.eligible_quantity.unwrap_or_else(|| {
                        states.get(&symbol).map(|s| s.total_qty()).unwrap_or(0.0)
                    });
                    dps * qty
                });
                let state = states.entry(symbol).or_default();
                state.dividend_received += gross;
                state.last_transaction_date = Some(txn.trade_date.clone());
            }
            TransactionType::Sip => {}
        }
    }

    persist_rebuild(pool, portfolio_id, &pending_lots, &pending_matches, &states).await
}

struct PendingMatch {
    sell_transaction_id: i64,
    buy_transaction_id: i64,
    symbol: String,
    quantity: f64,
    buy_date: String,
    sell_date: String,
    buy_cost_per_share: f64,
    sell_price: f64,
    cost_basis: f64,
    sell_value: f64,
    realized_gain: f64,
}

async fn persist_rebuild(
    pool: &SqlitePool,
    portfolio_id: i64,
    pending_lots: &[(String, Lot)],
    pending_matches: &[PendingMatch],
    states: &HashMap<String, SymbolState>,
) -> Result<RebuildResult> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM fifo_lots WHERE portfolio_id = ?")
        .bind(portfolio_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM realized_matches WHERE portfolio_id = ?")
        .bind(portfolio_id)
        .execute(&mut *tx)
        .await?;

    for (symbol, lot) in pending_lots {
        if lot.remaining_qty <= 1e-9 {
            continue;
        }
        let cps = lot.cost_per_share();
        sqlx::query(
            "INSERT INTO fifo_lots (portfolio_id, symbol, source_transaction_id, acquired_date,
             original_quantity, remaining_quantity, total_cost, cost_per_share)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(portfolio_id)
        .bind(symbol)
        .bind(lot.source_txn_id)
        .bind(&lot.acquired_date)
        .bind(lot.remaining_qty) // original = remaining at rebuild time for open lots
        .bind(lot.remaining_qty)
        .bind(lot.total_cost)
        .bind(cps)
        .execute(&mut *tx)
        .await?;
    }

    for m in pending_matches {
        sqlx::query(
            "INSERT INTO realized_matches (portfolio_id, sell_transaction_id, buy_transaction_id,
             symbol, quantity, buy_date, sell_date, buy_cost_per_share, sell_price,
             cost_basis, sell_value, realized_gain)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(portfolio_id)
        .bind(m.sell_transaction_id)
        .bind(m.buy_transaction_id)
        .bind(&m.symbol)
        .bind(m.quantity)
        .bind(&m.buy_date)
        .bind(&m.sell_date)
        .bind(m.buy_cost_per_share)
        .bind(m.sell_price)
        .bind(m.cost_basis)
        .bind(m.sell_value)
        .bind(m.realized_gain)
        .execute(&mut *tx)
        .await?;
    }

    let rebuilt_at = Utc::now().timestamp();
    let mut symbols = HashMap::new();
    let mut total_invested = 0.0;
    let mut total_realized = 0.0;
    let mut total_dividend = 0.0;

    for (sym, state) in states {
        let stats = state.to_stats();
        total_invested += stats.total_cost;
        total_realized += stats.realized_gain;
        total_dividend += stats.dividend_received;
        symbols.insert(sym.clone(), stats);
    }

    let summary = serde_json::json!({
        "invested_amount": total_invested,
        "realized_gain": total_realized,
        "dividend_received": total_dividend,
        "symbols": symbols.iter().map(|(k, v)| (k, v)).collect::<HashMap<_, _>>(),
    });

    sqlx::query(
        "INSERT INTO portfolio_snapshots (portfolio_id, summary_json, rebuilt_at)
         VALUES (?, ?, ?)
         ON CONFLICT(portfolio_id) DO UPDATE SET summary_json = excluded.summary_json,
         rebuilt_at = excluded.rebuilt_at",
    )
    .bind(portfolio_id)
    .bind(summary.to_string())
    .bind(rebuilt_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(RebuildResult {
        portfolio_id,
        symbols,
        total_invested,
        total_realized,
        total_dividend,
        rebuilt_at,
    })
}

async fn load_transactions(pool: &SqlitePool, portfolio_id: i64) -> Result<Vec<Transaction>> {
    let rows = sqlx::query(
        "SELECT id, user_id, portfolio_id, txn_type, trade_date, symbol, quantity, price,
         gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity,
         notes, source, corporate_action_key, created_at, updated_at
         FROM transactions WHERE portfolio_id = ? ORDER BY trade_date ASC, id ASC",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    rows.iter().map(row_to_transaction).collect()
}

fn row_to_transaction(row: &sqlx::sqlite::SqliteRow) -> Result<Transaction> {
    let txn_type_str: String = row.try_get("txn_type")?;
    let txn_type = TransactionType::parse(&txn_type_str)
        .ok_or_else(|| Error::Ledger(format!("unknown txn_type {txn_type_str}")))?;
    Ok(Transaction {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        portfolio_id: row.try_get("portfolio_id")?,
        txn_type,
        trade_date: row.try_get("trade_date")?,
        symbol: row.try_get("symbol")?,
        quantity: row.try_get("quantity")?,
        price: row.try_get("price")?,
        gross_amount: row.try_get("gross_amount")?,
        brokerage: row.try_get("brokerage")?,
        taxes: row.try_get("taxes")?,
        net_amount: row.try_get("net_amount")?,
        split_ratio_num: row.try_get("split_ratio_num")?,
        split_ratio_den: row.try_get("split_ratio_den")?,
        bonus_ratio_num: row.try_get("bonus_ratio_num")?,
        bonus_ratio_den: row.try_get("bonus_ratio_den")?,
        dividend_per_share: row.try_get("dividend_per_share")?,
        tds: row.try_get("tds")?,
        eligible_quantity: row.try_get("eligible_quantity")?,
        notes: row.try_get("notes")?,
        source: row.try_get("source")?,
        corporate_action_key: row.try_get("corporate_action_key")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        labels: vec![],
    })
}

fn require_symbol(txn: &Transaction) -> Result<String> {
    txn.symbol
        .clone()
        .ok_or_else(|| Error::Ledger(format!("transaction {} requires symbol", txn.id)))
}

fn require_positive_qty(qty: Option<f64>, field: &str) -> Result<f64> {
    let q = qty.ok_or_else(|| Error::Ledger(format!("{field} is required")))?;
    if q <= 0.0 {
        return Err(Error::Ledger(format!("{field} must be positive")));
    }
    Ok(q)
}

fn buy_total_cost(txn: &Transaction) -> Result<f64> {
    if let Some(net) = txn.net_amount {
        return Ok(net);
    }
    let qty = require_positive_qty(txn.quantity, "quantity")?;
    let price = txn.price.unwrap_or(0.0);
    let gross = txn.gross_amount.unwrap_or(qty * price);
    let brokerage = txn.brokerage.unwrap_or(0.0);
    let taxes = txn.taxes.unwrap_or(0.0);
    Ok(gross + brokerage + taxes)
}

fn resolve_sell_price(txn: &Transaction, sell_qty: f64, state: &SymbolState) -> f64 {
    if let Some(price) = txn.price.map(f64::abs).filter(|p| *p > 0.0) {
        return price;
    }
    if let Some(gross) = txn.gross_amount.map(f64::abs).filter(|g| *g > 0.0) {
        return gross / sell_qty;
    }
    if let Some(net) = txn.net_amount.map(f64::abs).filter(|n| *n > 0.0) {
        return net / sell_qty;
    }
    if matches!(
        txn.txn_type,
        TransactionType::DemergerRedemption | TransactionType::MergerRedemption
    ) {
        let total_cost = state.total_cost();
        let total_qty = state.total_qty();
        if total_qty > 1e-9 {
            return total_cost / total_qty;
        }
    }
    0.0
}

fn split_ratio(txn: &Transaction) -> Result<(f64, f64)> {
    if let Some(num) = txn.split_ratio_num.filter(|n| *n > 0.0) {
        let den = txn.split_ratio_den.filter(|d| *d > 0.0).unwrap_or(1.0);
        return Ok((num, den));
    }
    if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
        return Ok((qty, 1.0));
    }
    Err(Error::Ledger("split ratio num required".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ensure_local_user;
    use crate::db;
    use crate::models::NewTransaction;
    use crate::portfolios;
    use crate::transactions;

    async fn setup_portfolio(pool: &SqlitePool) -> (i64, i64) {
        let user = ensure_local_user(pool).await.unwrap();
        let p = portfolios::create(
            pool,
            user.id,
            &crate::models::NewPortfolio {
                name: "Test".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();
        (user.id, p.id)
    }

    #[tokio::test]
    async fn fifo_sell_across_two_buys() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
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
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-02-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: Some(150.0),
                gross_amount: Some(1500.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(1500.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Sell,
                trade_date: "2024-03-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(12.0),
                price: Some(200.0),
                gross_amount: Some(2400.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(2400.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 8.0).abs() < 1e-6);
        assert!((stats.realized_gain - 1100.0).abs() < 1e-6);

        let matches: Vec<(f64, f64)> = sqlx::query_as(
            "SELECT quantity, realized_gain FROM realized_matches WHERE portfolio_id = ?",
        )
        .bind(portfolio_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(matches.len(), 2);
        let total_gain: f64 = matches.iter().map(|(_, g)| g).sum();
        assert!((total_gain - 1100.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn split_preserves_total_cost() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: Some(1000.0),
                gross_amount: Some(10000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(10000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let mut split_txn = NewTransaction {
            portfolio_id,
            txn_type: TransactionType::Split,
            trade_date: "2024-06-01".into(),
            symbol: Some("ITC.NS".into()),
            quantity: None,
            price: None,
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: Some(5.0),
            split_ratio_den: Some(1.0),
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
        };
        transactions::create(&pool, user_id, &split_txn).await.unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 50.0).abs() < 1e-6);
        assert!((stats.total_cost - 10000.0).abs() < 1e-6);
        let avg = stats.total_cost / stats.quantity;
        assert!((avg - 200.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn split_with_quantity_only_applies_multiplier() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: Some(1000.0),
                gross_amount: Some(10000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(10000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Split,
                trade_date: "2024-06-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(5.0),
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 50.0).abs() < 1e-6);
        assert!((stats.total_cost - 10000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn bonus_preserves_total_cost() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(100.0),
                price: Some(500.0),
                gross_amount: Some(50000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(50000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Bonus,
                trade_date: "2024-06-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: None,
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: Some(1.0),
                bonus_ratio_den: Some(1.0),
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 200.0).abs() < 1e-6);
        assert!((stats.total_cost - 50000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn bonus_with_quantity_adds_zero_cost_lot() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("BSE.NS".into()),
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
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Bonus,
                trade_date: "2025-05-23".into(),
                symbol: Some("BSE.NS".into()),
                quantity: Some(1.0),
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("BSE.NS").unwrap();
        assert!((stats.quantity - 11.0).abs() < 1e-6);
        assert!((stats.total_cost - 1000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn dividend_does_not_change_cost() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(100.0),
                price: Some(350.0),
                gross_amount: Some(35000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(35000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Dividend,
                trade_date: "2024-06-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: None,
                price: None,
                gross_amount: Some(700.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(700.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: Some(7.0),
                tds: None,
                eligible_quantity: Some(100.0),
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.total_cost - 35000.0).abs() < 1e-6);
        assert!((stats.dividend_received - 700.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn merger_investment_adds_lot() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::MergerInvestment,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
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
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 10.0).abs() < 1e-6);
        assert!((stats.total_cost - 1000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn demerger_redemption_zero_gain() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(100.0),
                price: Some(350.0),
                gross_amount: Some(35000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(35000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::DemergerRedemption,
                trade_date: "2024-06-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 90.0).abs() < 1e-6);
        assert!(stats.realized_gain.abs() < 1e-6);
    }

    #[tokio::test]
    async fn sell_exceeds_available_fails() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
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
            },
        )
        .await
        .unwrap();

        assert!(
            transactions::create(
                &pool,
                user_id,
                &NewTransaction {
                    portfolio_id,
                    txn_type: TransactionType::Sell,
                    trade_date: "2024-03-01".into(),
                    symbol: Some("ITC.NS".into()),
                    quantity: Some(15.0),
                    price: Some(200.0),
                    gross_amount: Some(3000.0),
                    brokerage: None,
                    taxes: None,
                    net_amount: Some(3000.0),
                    split_ratio_num: None,
                    split_ratio_den: None,
                    bonus_ratio_num: None,
                    bonus_ratio_den: None,
                    dividend_per_share: None,
                    tds: None,
                    eligible_quantity: None,
                    notes: None,
                },
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn sip_does_not_create_lots() {
        let pool = db::open_memory().await.unwrap();
        let (user_id, portfolio_id) = setup_portfolio(&pool).await;

        transactions::create(
            &pool,
            user_id,
            &NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Sip,
                trade_date: "2024-01-01".into(),
                symbol: Some("MF:122639".into()),
                quantity: None,
                price: None,
                gross_amount: Some(5000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(5000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        assert_eq!(result.total_invested, 0.0);

        let lots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fifo_lots WHERE portfolio_id = ?")
            .bind(portfolio_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(lots, 0);
    }
}
