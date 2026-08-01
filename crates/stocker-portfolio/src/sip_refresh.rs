//! Materialize SIP transactions into buy rows using NAV/price on or after SIP date.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use sqlx::SqlitePool;
use stocker_core::fetcher::fetch_chart_history;
use stocker_mf::{allot_mf_purchase, mf_symbol, parse_mf_symbol, MfService};

use crate::engine::rebuild;
use crate::error::{Error, Result};
use crate::models::{NewTransaction, Transaction, TransactionType};
use crate::transactions::{self, TransactionFilter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SipRefreshFailure {
    pub sip_id: i64,
    pub symbol: Option<String>,
    pub trade_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SipRefreshResult {
    /// SIP transaction rows registered from active schedules before materialization.
    #[serde(default)]
    pub registered: Vec<i64>,
    pub created: Vec<i64>,
    pub skipped: Vec<i64>,
    pub failed: Vec<SipRefreshFailure>,
}

pub fn sip_buy_key(sip_id: i64) -> String {
    format!("sip_buy:{sip_id}")
}

pub async fn refresh_sip_transactions(
    pool: &SqlitePool,
    mf: Option<Arc<MfService>>,
    user_id: i64,
    portfolio_id: i64,
) -> Result<SipRefreshResult> {
    let sips = transactions::list(
        pool,
        user_id,
        &TransactionFilter {
            portfolio_id: Some(portfolio_id),
            txn_type: Some(TransactionType::Sip),
            ..Default::default()
        },
    )
    .await?;

    let materialized = load_materialized_sip_ids(pool, user_id, portfolio_id).await?;

    let mut result = SipRefreshResult {
        registered: Vec::new(),
        created: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };

    for sip in sips {
        if materialized.contains(&sip.id) {
            result.skipped.push(sip.id);
            continue;
        }

        match materialize_sip(pool, mf.as_deref(), user_id, &sip).await {
            Ok(buy_id) => result.created.push(buy_id),
            Err(e) => result.failed.push(SipRefreshFailure {
                sip_id: sip.id,
                symbol: sip.symbol.clone(),
                trade_date: sip.trade_date.clone(),
                reason: e.to_string(),
            }),
        }
    }

    if !result.created.is_empty() {
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(result)
}

pub async fn load_materialized_sip_ids(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
) -> Result<HashSet<i64>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT corporate_action_key FROM transactions
         WHERE user_id = ? AND portfolio_id = ? AND corporate_action_key LIKE 'sip_buy:%'",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|key| key.strip_prefix("sip_buy:")?.parse().ok())
        .collect())
}

pub async fn materialize_sip(
    pool: &SqlitePool,
    mf: Option<&MfService>,
    user_id: i64,
    sip: &Transaction,
) -> Result<i64> {
    let symbol = sip
        .symbol
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::InvalidInput("sip missing symbol".into()))?;

    let amount = sip
        .net_amount
        .or(sip.gross_amount)
        .filter(|a| *a > 0.0)
        .ok_or_else(|| Error::InvalidInput("sip missing amount".into()))?;

    // Prefer MF NAV path: accept MF:{code}, fund name, or name wrongly suffixed with .BO/.NS.
    let mf_scheme = if let Some(code) = parse_mf_symbol(symbol) {
        Some(code)
    } else if looks_like_equity_sip_symbol(symbol) {
        None
    } else if let Some(svc) = mf {
        Some(svc.resolve_scheme_code(symbol).await.map_err(|e| {
            Error::Other(format!("could not resolve mutual fund '{symbol}': {e}"))
        })?)
    } else {
        None
    };

    let (trade_date, price, quantity, taxes, notes, buy_symbol) =
        if let (Some(qty), Some(px)) = (
            sip.quantity.filter(|q| *q > 0.0),
            sip.price.filter(|p| *p > 0.0),
        ) {
            (
                sip.trade_date.clone(),
                px,
                qty,
                sip.taxes.filter(|t| *t > 0.0),
                format!("Materialized from SIP #{}", sip.id),
                symbol.to_string(),
            )
        } else if let Some(scheme_code) = mf_scheme {
            let mf = mf.ok_or_else(|| Error::Other("mutual fund service unavailable".into()))?;
            let nav = mf
                .nav_on_or_after(scheme_code, &sip.trade_date)
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
            let allot = allot_mf_purchase(amount, nav.nav).ok_or_else(|| {
                Error::InvalidInput(format!(
                    "cannot allot MF purchase for amount {amount} at NAV {}",
                    nav.nav
                ))
            })?;
            let resolved = mf_symbol(scheme_code);
            canonicalize_mf_symbol(pool, sip, &resolved).await?;
            (
                nav.nav_date,
                allot.adjusted_nav,
                allot.quantity,
                Some(allot.stamp_duty),
                format!(
                    "Materialized from SIP #{} (NAV {:.4}, stamp duty {:.2})",
                    sip.id, allot.published_nav, allot.stamp_duty
                ),
                resolved,
            )
        } else {
            let (date, px) = stock_price_on_or_after(symbol, &sip.trade_date).await?;
            let qty = amount / px;
            (
                date,
                px,
                qty,
                None,
                format!("Materialized from SIP #{}", sip.id),
                symbol.to_string(),
            )
        };

    if price <= 0.0 {
        return Err(Error::InvalidInput(format!(
            "invalid price {price} for {buy_symbol}"
        )));
    }

    let corp_key = sip_buy_key(sip.id);
    let now = Utc::now().timestamp();

    let input = NewTransaction {
        portfolio_id: sip.portfolio_id,
        txn_type: TransactionType::Buy,
        trade_date,
        symbol: Some(buy_symbol),
        quantity: Some(quantity),
        price: Some(price),
        // Cash out is the full SIP amount; stamp duty is recorded under taxes.
        gross_amount: Some(amount),
        brokerage: None,
        taxes,
        net_amount: Some(amount),
        split_ratio_num: None,
        split_ratio_den: None,
        bonus_ratio_num: None,
        bonus_ratio_den: None,
        dividend_per_share: None,
        tds: None,
        eligible_quantity: None,
        notes: Some(notes),
        schedule_id: sip.schedule_id,
    };
    transactions::validate_new(&input)?;

    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, schedule_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'sip_refresh', ?, ?, ?, ?)",
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
                return Error::Conflict("sip already materialized".into());
            }
        }
        Error::from(e)
    })?;

    Ok(res.last_insert_rowid())
}

/// True when the symbol looks like an equity ticker (not a fund name).
fn looks_like_equity_sip_symbol(symbol: &str) -> bool {
    let trimmed = symbol.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return false;
    }
    let upper = trimmed.to_ascii_uppercase();
    upper.ends_with(".NS")
        || upper.ends_with(".BO")
        || (trimmed.len() <= 20
            && trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '&'))
}

/// Rewrite SIP / schedule rows that still store a fund name (or name.BO) to `MF:{code}`.
async fn canonicalize_mf_symbol(pool: &SqlitePool, sip: &Transaction, resolved: &str) -> Result<()> {
    if sip.symbol.as_deref() == Some(resolved) {
        return Ok(());
    }
    let now = Utc::now().timestamp();
    sqlx::query("UPDATE transactions SET symbol = ?, updated_at = ? WHERE id = ?")
        .bind(resolved)
        .bind(now)
        .bind(sip.id)
        .execute(pool)
        .await?;
    if let Some(schedule_id) = sip.schedule_id {
        sqlx::query("UPDATE mf_schedules SET symbol = ?, updated_at = ? WHERE id = ?")
            .bind(resolved)
            .bind(now)
            .bind(schedule_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn stock_price_on_or_after(symbol: &str, sip_date: &str) -> Result<(String, f64)> {
    let chart = fetch_chart_history(symbol, "10y").await;
    let mut best: Option<(String, f64)> = None;

    for bar in &chart.bars {
        let date = Utc
            .timestamp_opt(bar.ts, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
        if date.is_empty() || date.as_str() < sip_date || bar.close <= 0.0 {
            continue;
        }
        // Earliest trading day on or after the SIP date.
        if best.as_ref().is_none_or(|(d, _)| date.as_str() < d.as_str()) {
            best = Some((date, bar.close));
        }
    }

    best.ok_or_else(|| {
        Error::Other(format!(
            "no stock price on or after {sip_date} for {symbol}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::NewTransaction;
    use crate::portfolios;
    use crate::transactions;
    use stocker_mf::allot_mf_purchase;

    #[test]
    fn sip_buy_key_format() {
        assert_eq!(sip_buy_key(42), "sip_buy:42");
    }

    #[test]
    fn mf_sip_allotment_subtracts_stamp_and_adjusts_nav() {
        let allot = allot_mf_purchase(5_000.0, 100.0).unwrap();
        assert!((allot.stamp_duty - 0.25).abs() < 1e-9);
        assert!((allot.quantity - 49.9975).abs() < 1e-9);
        assert!((allot.adjusted_nav - (5_000.0 / 49.9975)).abs() < 1e-9);
        assert!(allot.adjusted_nav > 100.0);
    }

    #[test]
    fn fund_name_with_bo_suffix_is_not_equity() {
        assert!(!looks_like_equity_sip_symbol(
            "PARAG PARIKH FLEXI CAP FUND - DIRECT PLAN - GROWTH.BO"
        ));
        assert!(!looks_like_equity_sip_symbol(
            "SBI FLEXICAP FUND - DIRECT PLAN - GROWTH OPTION.BO"
        ));
        assert!(looks_like_equity_sip_symbol("RELIANCE.BO"));
        assert!(looks_like_equity_sip_symbol("CDSL.NS"));
    }

    #[tokio::test]
    async fn refresh_skips_already_materialized_sip() {
        let pool = db::open_memory().await.unwrap();
        let user = crate::auth::ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &crate::models::NewPortfolio {
                name: "SIP".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let sip = transactions::create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Sip,
                trade_date: "2024-01-01".into(),
                symbol: Some("MF:122639".into()),
                quantity: None,
                price: None,
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

        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
             price, gross_amount, net_amount, source, corporate_action_key, created_at, updated_at)
             VALUES (?, ?, 'buy', '2024-01-02', 'MF:122639', 10, 100, 1000, 1000, 'sip_refresh', ?, ?, ?)",
        )
        .bind(user.id)
        .bind(portfolio.id)
        .bind(sip_buy_key(sip.id))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let result = refresh_sip_transactions(&pool, None, user.id, portfolio.id)
            .await
            .unwrap();
        assert!(result.created.is_empty());
        assert_eq!(result.skipped, vec![sip.id]);
    }
}
