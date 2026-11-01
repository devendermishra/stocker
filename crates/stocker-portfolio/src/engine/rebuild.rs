//! Replay transactions in order and materialize fifo_lots + realized_matches.

/// Bump when open-lot rows must be rewritten for portfolios that are otherwise clean.
pub(crate) const FIFO_LOTS_VERSION: i64 = 2;

use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use stocker_domain::portfolio::models::{Transaction, TransactionType};
use stocker_domain::portfolio::{replay_transactions, Lot, PendingMatch};

use stocker_domain::portfolio::SymbolStats;

#[derive(Debug, Clone)]
pub struct RebuildResult {
    pub portfolio_id: i64,
    pub symbols: HashMap<String, SymbolStats>,
    pub total_invested: f64,
    pub total_realized: f64,
    pub total_dividend: f64,
    pub rebuilt_at: i64,
}

pub async fn rebuild(pool: &SqlitePool, portfolio_id: i64) -> Result<RebuildResult> {
    let txns = load_transactions(pool, portfolio_id).await?;
    let replay = replay_transactions(&txns).map_err(|e| Error::Ledger(e.to_string()))?;
    persist_rebuild(
        pool,
        portfolio_id,
        &replay.open_lots,
        &replay.matches,
        &replay.symbols,
    )
    .await
}

async fn persist_rebuild(
    pool: &SqlitePool,
    portfolio_id: i64,
    pending_lots: &[(String, Lot)],
    pending_matches: &[PendingMatch],
    states: &HashMap<String, SymbolStats>,
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
        .bind(lot.opened_qty)
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

    for (sym, stats) in states {
        total_invested += stats.total_cost;
        total_realized += stats.realized_gain;
        total_dividend += stats.dividend_received;
        symbols.insert(sym.clone(), stats.clone());
    }

    let summary = serde_json::json!({
        "fifo_version": FIFO_LOTS_VERSION,
        "invested_amount": total_invested,
        "realized_gain": total_realized,
        "dividend_received": total_dividend,
        "symbols": symbols.iter().map(|(k, v)| (k, v)).collect::<HashMap<_, _>>(),
    });

    sqlx::query(
        "INSERT INTO portfolio_snapshots (portfolio_id, summary_json, rebuilt_at,
         holdings_json, valuation_summary_json, symbol_prices_json, priced_at)
         VALUES (?, ?, ?, NULL, NULL, NULL, 0)
         ON CONFLICT(portfolio_id) DO UPDATE SET summary_json = excluded.summary_json,
         rebuilt_at = excluded.rebuilt_at,
         holdings_json = NULL, valuation_summary_json = NULL,
         symbol_prices_json = NULL, priced_at = 0",
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
         notes, source, corporate_action_key, schedule_id, created_at, updated_at
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
        schedule_id: row.try_get("schedule_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        labels: vec![],
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ensure_local_user;
    use crate::db;
    use stocker_domain::portfolio::models::NewTransaction;
    use crate::portfolios;
    use crate::transactions;

    async fn setup_portfolio(pool: &SqlitePool) -> (i64, i64) {
        let user = ensure_local_user(pool).await.unwrap();
        let p = portfolios::create(
            pool,
            user.id,
            &stocker_domain::portfolio::models::NewPortfolio {
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 8.0).abs() < 1e-6);
        assert!((stats.realized_gain - 1100.0).abs() < 1e-6);

        let lots: Vec<(f64, f64, f64)> = sqlx::query_as(
            "SELECT original_quantity, remaining_quantity, total_cost FROM fifo_lots WHERE portfolio_id = ? ORDER BY source_transaction_id",
        )
        .bind(portfolio_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(lots.len(), 1);
        assert!((lots[0].0 - 10.0).abs() < 1e-6);
        assert!((lots[0].1 - 8.0).abs() < 1e-6);
        assert!((lots[0].2 - 1200.0).abs() < 1e-6);

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
                schedule_id: None,
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
            schedule_id: None,
        };
        transactions::create(&pool, user_id, &split_txn).await.unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 50.0).abs() < 1e-6);
        assert!((stats.total_cost - 10000.0).abs() < 1e-6);
        let avg = stats.total_cost / stats.quantity;
        assert!((avg - 200.0).abs() < 1e-6);

        let lots: Vec<(f64, f64, f64)> = sqlx::query_as(
            "SELECT original_quantity, remaining_quantity, cost_per_share FROM fifo_lots WHERE portfolio_id = ?",
        )
        .bind(portfolio_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(lots.len(), 1);
        assert!((lots[0].0 - 50.0).abs() < 1e-6);
        assert!((lots[0].1 - 50.0).abs() < 1e-6);
        assert!((lots[0].2 - 200.0).abs() < 1e-6);
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
    async fn bonus_one_for_five_scales_quantity() {
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
                schedule_id: None,
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
                bonus_ratio_den: Some(5.0),
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
                schedule_id: None,
            },
        )
        .await
        .unwrap();

        let result = rebuild(&pool, portfolio_id).await.unwrap();
        let stats = result.symbols.get("ITC.NS").unwrap();
        assert!((stats.quantity - 120.0).abs() < 1e-6);
        assert!((stats.total_cost - 50000.0).abs() < 1e-6);
        let avg = stats.total_cost / stats.quantity;
        assert!((avg - 50000.0 / 120.0).abs() < 1e-6);
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                schedule_id: None,
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
                    schedule_id: None,
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
                schedule_id: None,
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
