//! Materialize SWP transactions into sell rows using NAV on or after withdrawal date.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use sqlx::SqlitePool;
use stocker_mf::{parse_mf_symbol, MfService};

use crate::engine::{ensure_ledger, rebuild};
use crate::error::{Error, Result};
use crate::models::{NewTransaction, Transaction, TransactionType};
use crate::transactions::{self, TransactionFilter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwpRefreshFailure {
    pub swp_id: i64,
    pub symbol: Option<String>,
    pub trade_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwpRefreshResult {
    pub created: Vec<i64>,
    pub skipped: Vec<i64>,
    pub failed: Vec<SwpRefreshFailure>,
}

pub fn swp_sell_key(swp_id: i64) -> String {
    format!("swp_sell:{swp_id}")
}

pub async fn refresh_swp_transactions(
    pool: &SqlitePool,
    mf: Option<Arc<MfService>>,
    user_id: i64,
    portfolio_id: i64,
) -> Result<SwpRefreshResult> {
    let swps = transactions::list(
        pool,
        user_id,
        &TransactionFilter {
            portfolio_id: Some(portfolio_id),
            txn_type: Some(TransactionType::Swp),
            ..Default::default()
        },
    )
    .await?;

    let materialized = load_materialized_swp_ids(pool, user_id, portfolio_id).await?;

    let mut result = SwpRefreshResult {
        created: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };

    for swp in swps {
        if materialized.contains(&swp.id) {
            result.skipped.push(swp.id);
            continue;
        }

        match materialize_swp(pool, mf.as_deref(), user_id, &swp).await {
            Ok(sell_id) => result.created.push(sell_id),
            Err(e) => result.failed.push(SwpRefreshFailure {
                swp_id: swp.id,
                symbol: swp.symbol.clone(),
                trade_date: swp.trade_date.clone(),
                reason: e.to_string(),
            }),
        }
    }

    if !result.created.is_empty() {
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(result)
}

pub async fn load_materialized_swp_ids(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
) -> Result<HashSet<i64>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT corporate_action_key FROM transactions
         WHERE user_id = ? AND portfolio_id = ? AND corporate_action_key LIKE 'swp_sell:%'",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|key| key.strip_prefix("swp_sell:")?.parse().ok())
        .collect())
}

pub async fn materialize_swp(
    pool: &SqlitePool,
    mf: Option<&MfService>,
    user_id: i64,
    swp: &Transaction,
) -> Result<i64> {
    let symbol = swp
        .symbol
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::InvalidInput("swp missing symbol".into()))?;

    let amount = swp
        .net_amount
        .or(swp.gross_amount)
        .filter(|a| *a > 0.0)
        .ok_or_else(|| Error::InvalidInput("swp missing amount".into()))?;

    let scheme_code = parse_mf_symbol(symbol)
        .ok_or_else(|| Error::InvalidInput("swp requires MF symbol".into()))?;
    let mf = mf.ok_or_else(|| Error::Other("mutual fund service unavailable".into()))?;
    let nav = mf
        .nav_on_or_after(scheme_code, &swp.trade_date)
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    let quantity = amount / nav.nav;
    let trade_date = nav.nav_date;
    let price = nav.nav;

    if price <= 0.0 || quantity <= 0.0 {
        return Err(Error::InvalidInput(format!(
            "invalid NAV/qty for {symbol}"
        )));
    }

    let ledger = ensure_ledger(pool, swp.portfolio_id, false).await?;
    let available = ledger
        .symbols
        .get(symbol)
        .map(|s| s.quantity)
        .unwrap_or(0.0);
    if quantity > available + 1e-6 {
        return Err(Error::InvalidInput(format!(
            "swp quantity {quantity:.4} exceeds available {available:.4} for {symbol}"
        )));
    }

    let corp_key = swp_sell_key(swp.id);
    let notes = format!("Materialized from SWP #{}", swp.id);
    let now = Utc::now().timestamp();

    let input = NewTransaction {
        portfolio_id: swp.portfolio_id,
        txn_type: TransactionType::Sell,
        trade_date,
        symbol: Some(symbol.to_string()),
        quantity: Some(quantity),
        price: Some(price),
        gross_amount: Some(amount),
        brokerage: None,
        taxes: None,
        net_amount: Some(amount),
        split_ratio_num: None,
        split_ratio_den: None,
        bonus_ratio_num: None,
        bonus_ratio_den: None,
        dividend_per_share: None,
        tds: None,
        eligible_quantity: None,
        notes: Some(notes),
        schedule_id: swp.schedule_id,
    };
    transactions::validate_new(&input)?;

    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, schedule_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'swp_refresh', ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(input.portfolio_id)
    .bind(input.txn_type.as_str())
    .bind(&input.trade_date)
    .bind(input.symbol.as_deref())
    .bind(input.quantity)
    .bind(input.price)
    .bind(input.gross_amount)
    .bind(input.brokerage)
    .bind(input.taxes)
    .bind(input.net_amount)
    .bind(input.split_ratio_num)
    .bind(input.split_ratio_den)
    .bind(input.bonus_ratio_num)
    .bind(input.bonus_ratio_den)
    .bind(input.dividend_per_share)
    .bind(input.tds)
    .bind(input.eligible_quantity)
    .bind(input.notes.as_deref())
    .bind(corp_key.as_str())
    .bind(input.schedule_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return Error::Conflict("swp already materialized".into());
            }
        }
        Error::from(e)
    })?;

    Ok(res.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swp_sell_key_format() {
        assert_eq!(swp_sell_key(42), "swp_sell:42");
    }
}
