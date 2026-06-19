//! Scan and apply missing corporate actions and MF SIP installments.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};
use stocker_core::fetcher::fetch_chart_events;
use stocker_mf::{parse_mf_symbol, MfService};

use crate::engine::{ensure_ledger, rebuild};
use crate::error::{Error, Result};
use crate::models::{NewTransaction, Transaction, TransactionType};
use crate::mf_schedule;
use crate::models::ScheduleType;
use crate::sip_refresh::{load_materialized_sip_ids, materialize_sip};
use crate::swp_refresh::{load_materialized_swp_ids, materialize_swp};
use crate::transactions::{self, corporate_action_key, TransactionFilter};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanError {
    pub symbol: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuggestedCorporateAction {
    pub suggestion_id: String,
    pub symbol: String,
    pub txn_type: String,
    pub trade_date: String,
    pub dividend_per_share: Option<f64>,
    pub eligible_quantity: Option<f64>,
    pub gross_amount: Option<f64>,
    pub split_ratio_num: Option<f64>,
    pub split_ratio_den: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingSipMaterialization {
    pub suggestion_id: String,
    pub sip_id: i64,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuggestedSipInstallment {
    pub suggestion_id: String,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingSwpMaterialization {
    pub suggestion_id: String,
    pub swp_id: i64,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuggestedSwpInstallment {
    pub suggestion_id: String,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PortfolioRefreshScan {
    pub corporate_actions: Vec<SuggestedCorporateAction>,
    pub sip_pending: Vec<PendingSipMaterialization>,
    pub sip_suggested: Vec<SuggestedSipInstallment>,
    pub swp_pending: Vec<PendingSwpMaterialization>,
    pub swp_suggested: Vec<SuggestedSwpInstallment>,
    pub scan_errors: Vec<ScanError>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PortfolioRefreshApplyResult {
    pub corporate_actions_created: usize,
    pub sip_registered: usize,
    pub sip_materialized: usize,
    pub swp_registered: usize,
    pub swp_materialized: usize,
    pub failed: Vec<String>,
}

pub async fn scan_portfolio_refresh(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
) -> Result<PortfolioRefreshScan> {
    let ledger = ensure_ledger(pool, portfolio_id, false).await?;
    let existing_keys = load_existing_corp_keys(pool, portfolio_id).await?;
    let symbol_txns = load_equity_transactions(pool, portfolio_id).await?;

    let mut corporate_actions = Vec::new();

    for (symbol, stats) in &ledger.symbols {
        if symbol.starts_with("MF:") {
            continue;
        }
        if stats.quantity <= 1e-9 {
            continue;
        }
        let Some(since) = stats.last_transaction_date.as_deref() else {
            continue;
        };

        let events = fetch_chart_events(symbol, since).await;
        if events.dividends.is_empty() && events.splits.is_empty() {
            continue;
        }

        for div in &events.dividends {
            if div.date.as_str() <= since {
                continue;
            }
            let input = NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Dividend,
                trade_date: div.date.clone(),
                symbol: Some(symbol.clone()),
                quantity: None,
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: Some(div.amount),
                tds: None,
                eligible_quantity: None,
                notes: None,
                schedule_id: None,
            };
            let Some(key) = corporate_action_key(&input) else {
                continue;
            };
            if existing_keys.contains(&key) {
                continue;
            }
            let eligible_qty =
                holdings_qty_at_date(symbol_txns.get(symbol), &div.date);
            let gross = div.amount * eligible_qty;
            corporate_actions.push(SuggestedCorporateAction {
                suggestion_id: key.clone(),
                symbol: symbol.clone(),
                txn_type: "dividend".into(),
                trade_date: div.date.clone(),
                dividend_per_share: Some(div.amount),
                eligible_quantity: Some(eligible_qty),
                gross_amount: Some(gross),
                split_ratio_num: None,
                split_ratio_den: None,
            });
        }

        for split in &events.splits {
            if split.date.as_str() <= since {
                continue;
            }
            let input = NewTransaction {
                portfolio_id,
                txn_type: TransactionType::Split,
                trade_date: split.date.clone(),
                symbol: Some(symbol.clone()),
                quantity: None,
                price: None,
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
                split_ratio_num: Some(split.numerator),
                split_ratio_den: Some(split.denominator),
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
                schedule_id: None,
            };
            let Some(key) = corporate_action_key(&input) else {
                continue;
            };
            if existing_keys.contains(&key) {
                continue;
            }
            corporate_actions.push(SuggestedCorporateAction {
                suggestion_id: key,
                symbol: symbol.clone(),
                txn_type: "split".into(),
                trade_date: split.date.clone(),
                dividend_per_share: None,
                eligible_quantity: None,
                gross_amount: None,
                split_ratio_num: Some(split.numerator),
                split_ratio_den: Some(split.denominator),
            });
        }
    }

    let materialized = load_materialized_sip_ids(pool, user_id, portfolio_id).await?;
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

    let mut sip_pending = Vec::new();
    for sip in &sips {
        if materialized.contains(&sip.id) {
            continue;
        }
        let amount = sip
            .net_amount
            .or(sip.gross_amount)
            .unwrap_or(0.0);
        sip_pending.push(PendingSipMaterialization {
            suggestion_id: format!("sip_mat:{}", sip.id),
            sip_id: sip.id,
            symbol: sip.symbol.clone().unwrap_or_default(),
            trade_date: sip.trade_date.clone(),
            amount,
        });
    }

    let mut sip_suggested = detect_missing_sip_installments(pool, portfolio_id, &sips).await?;

    let materialized_swp = load_materialized_swp_ids(pool, user_id, portfolio_id).await?;
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

    let mut swp_pending = Vec::new();
    for swp in &swps {
        if materialized_swp.contains(&swp.id) {
            continue;
        }
        let amount = swp.net_amount.or(swp.gross_amount).unwrap_or(0.0);
        swp_pending.push(PendingSwpMaterialization {
            suggestion_id: format!("swp_mat:{}", swp.id),
            swp_id: swp.id,
            symbol: swp.symbol.clone().unwrap_or_default(),
            trade_date: swp.trade_date.clone(),
            amount,
        });
    }

    let mut swp_suggested = Vec::new();
    let active_schedules = mf_schedule::get_active_schedules(pool, portfolio_id).await?;
    for schedule in &active_schedules {
        let missing = mf_schedule::suggest_missing_for_schedule(pool, schedule).await?;
        for inst in missing {
            let suggestion_id = format!(
                "schedule_new:{}:{}:{}",
                schedule.id, inst.trade_date, (schedule.amount * 100.0).round() as i64
            );
            match schedule.schedule_type {
                ScheduleType::Sip => {
                    if !sip_suggested.iter().any(|s| s.suggestion_id == suggestion_id) {
                        sip_suggested.push(SuggestedSipInstallment {
                            suggestion_id,
                            symbol: schedule.symbol.clone(),
                            trade_date: inst.trade_date,
                            amount: schedule.amount,
                        });
                    }
                }
                ScheduleType::Swp => {
                    swp_suggested.push(SuggestedSwpInstallment {
                        suggestion_id,
                        symbol: schedule.symbol.clone(),
                        trade_date: inst.trade_date,
                        amount: schedule.amount,
                    });
                }
            }
        }
    }

    Ok(PortfolioRefreshScan {
        corporate_actions,
        sip_pending,
        sip_suggested,
        swp_pending,
        swp_suggested,
        scan_errors: Vec::new(),
    })
}

pub async fn apply_portfolio_refresh(
    pool: &SqlitePool,
    mf: Option<Arc<MfService>>,
    user_id: i64,
    portfolio_id: i64,
    selections: &[String],
) -> Result<PortfolioRefreshApplyResult> {
    if selections.is_empty() {
        return Ok(PortfolioRefreshApplyResult {
            corporate_actions_created: 0,
            sip_registered: 0,
            sip_materialized: 0,
            swp_registered: 0,
            swp_materialized: 0,
            failed: Vec::new(),
        });
    }

    let scan = scan_portfolio_refresh(pool, user_id, portfolio_id).await?;
    let corp_by_id: HashMap<_, _> = scan
        .corporate_actions
        .iter()
        .map(|c| (c.suggestion_id.clone(), c.clone()))
        .collect();
    let sip_pending_by_id: HashMap<_, _> = scan
        .sip_pending
        .iter()
        .map(|s| (s.suggestion_id.clone(), s.clone()))
        .collect();
    let sip_new_by_id: HashMap<_, _> = scan
        .sip_suggested
        .iter()
        .map(|s| (s.suggestion_id.clone(), s.clone()))
        .collect();
    let swp_pending_by_id: HashMap<_, _> = scan
        .swp_pending
        .iter()
        .map(|s| (s.suggestion_id.clone(), s.clone()))
        .collect();
    let swp_new_by_id: HashMap<_, _> = scan
        .swp_suggested
        .iter()
        .map(|s| (s.suggestion_id.clone(), s.clone()))
        .collect();

    let mut corp_created = 0usize;
    let mut sip_registered = 0usize;
    let mut sip_materialized = 0usize;
    let mut swp_registered = 0usize;
    let mut swp_materialized = 0usize;
    let mut failed = Vec::new();
    let mut needs_rebuild = false;
    let mut new_sip_ids: Vec<i64> = Vec::new();
    let mut new_swp_ids: Vec<i64> = Vec::new();

    for sel in selections {
        if let Some(corp) = corp_by_id.get(sel) {
            match insert_corporate_action(pool, user_id, portfolio_id, corp).await {
                Ok(()) => {
                    corp_created += 1;
                    needs_rebuild = true;
                }
                Err(e) => failed.push(format!("{}: {e}", sel)),
            }
            continue;
        }

        if let Some(pending) = sip_pending_by_id.get(sel) {
            let sip = transactions::get(pool, user_id, pending.sip_id).await?;
            match materialize_sip(pool, mf.as_deref(), user_id, &sip).await {
                Ok(_) => {
                    sip_materialized += 1;
                    needs_rebuild = true;
                }
                Err(e) => failed.push(format!("{}: {e}", sel)),
            }
            continue;
        }

        if let Some(new_sip) = sip_new_by_id.get(sel) {
            match insert_sip(pool, user_id, portfolio_id, new_sip).await {
                Ok(sip_id) => {
                    sip_registered += 1;
                    new_sip_ids.push(sip_id);
                }
                Err(e) => failed.push(format!("{}: {e}", sel)),
            }
            continue;
        }

        if let Some(pending) = swp_pending_by_id.get(sel) {
            let swp = transactions::get(pool, user_id, pending.swp_id).await?;
            match materialize_swp(pool, mf.as_deref(), user_id, &swp).await {
                Ok(_) => {
                    swp_materialized += 1;
                    needs_rebuild = true;
                }
                Err(e) => failed.push(format!("{}: {e}", sel)),
            }
            continue;
        }

        if let Some(new_swp) = swp_new_by_id.get(sel) {
            match insert_swp(pool, user_id, portfolio_id, new_swp).await {
                Ok(swp_id) => {
                    swp_registered += 1;
                    new_swp_ids.push(swp_id);
                }
                Err(e) => failed.push(format!("{}: {e}", sel)),
            }
            continue;
        }

        failed.push(format!("unknown selection: {sel}"));
    }

    for sip_id in new_sip_ids {
        if let Ok(sip) = transactions::get(pool, user_id, sip_id).await {
            if let Err(e) = materialize_sip(pool, mf.as_deref(), user_id, &sip).await {
                failed.push(format!("sip_mat_new:{sip_id}: {e}"));
            } else {
                sip_materialized += 1;
                needs_rebuild = true;
            }
        }
    }

    for swp_id in new_swp_ids {
        if let Ok(swp) = transactions::get(pool, user_id, swp_id).await {
            if let Err(e) = materialize_swp(pool, mf.as_deref(), user_id, &swp).await {
                failed.push(format!("swp_mat_new:{swp_id}: {e}"));
            } else {
                swp_materialized += 1;
                needs_rebuild = true;
            }
        }
    }

    if needs_rebuild {
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(PortfolioRefreshApplyResult {
        corporate_actions_created: corp_created,
        sip_registered,
        sip_materialized,
        swp_registered,
        swp_materialized,
        failed,
    })
}

async fn load_existing_corp_keys(
    pool: &SqlitePool,
    portfolio_id: i64,
) -> Result<HashSet<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT corporate_action_key FROM transactions
         WHERE portfolio_id = ? AND corporate_action_key IS NOT NULL",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

async fn load_equity_transactions(
    pool: &SqlitePool,
    portfolio_id: i64,
) -> Result<HashMap<String, Vec<Transaction>>> {
    let rows = sqlx::query(
        "SELECT id, user_id, portfolio_id, txn_type, trade_date, symbol, quantity, price,
         gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity,
         notes, source, corporate_action_key, schedule_id, created_at, updated_at
         FROM transactions WHERE portfolio_id = ? AND symbol NOT LIKE 'MF:%'
         ORDER BY trade_date ASC, id ASC",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let mut out: HashMap<String, Vec<Transaction>> = HashMap::new();
    for row in rows {
        let txn = row_to_transaction(&row)?;
        if let Some(sym) = txn.symbol.clone() {
            out.entry(sym).or_default().push(txn);
        }
    }
    Ok(out)
}

fn row_to_transaction(row: &sqlx::sqlite::SqliteRow) -> Result<Transaction> {
    let txn_type_str: String = row.try_get("txn_type")?;
    let txn_type = TransactionType::parse(&txn_type_str)
        .ok_or_else(|| Error::Other(format!("unknown txn_type {txn_type_str}")))?;
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

#[derive(Debug, Default)]
struct Lot {
    remaining_qty: f64,
}

fn holdings_qty_at_date(txns: Option<&Vec<Transaction>>, as_of_date: &str) -> f64 {
    let Some(txns) = txns else {
        return 0.0;
    };
    let mut lots: Vec<Lot> = Vec::new();
    for txn in txns {
        if txn.trade_date.as_str() > as_of_date {
            break;
        }
        match txn.txn_type {
            TransactionType::OpeningBalance
            | TransactionType::Buy
            | TransactionType::MergerInvestment
            | TransactionType::DemergerInvestment
            | TransactionType::Rights => {
                if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
                    lots.push(Lot {
                        remaining_qty: qty,
                    });
                }
            }
            TransactionType::Sell
            | TransactionType::MergerRedemption
            | TransactionType::DemergerRedemption => {
                let mut sell_qty = txn.quantity.unwrap_or(0.0).abs();
                for lot in lots.iter_mut() {
                    if sell_qty <= 1e-9 {
                        break;
                    }
                    let take = lot.remaining_qty.min(sell_qty);
                    lot.remaining_qty -= take;
                    sell_qty -= take;
                }
                lots.retain(|l| l.remaining_qty > 1e-9);
            }
            TransactionType::Split => {
                let num = txn.split_ratio_num.filter(|n| *n > 0.0).unwrap_or(1.0);
                let den = txn.split_ratio_den.filter(|d| *d > 0.0).unwrap_or(1.0);
                let factor = num / den;
                for lot in lots.iter_mut() {
                    lot.remaining_qty *= factor;
                }
            }
            TransactionType::Bonus => {
                if let (Some(bonus_num), Some(bonus_den)) =
                    (txn.bonus_ratio_num, txn.bonus_ratio_den)
                {
                    let factor = 1.0 + bonus_num / bonus_den;
                    for lot in lots.iter_mut() {
                        lot.remaining_qty *= factor;
                    }
                } else if let Some(qty) = txn.quantity.filter(|q| *q > 0.0) {
                    lots.push(Lot {
                        remaining_qty: qty,
                    });
                }
            }
            TransactionType::Dividend | TransactionType::Sip | TransactionType::Swp => {}
        }
    }
    lots.iter().map(|l| l.remaining_qty).sum()
}

async fn detect_missing_sip_installments(
    pool: &SqlitePool,
    portfolio_id: i64,
    sips: &[Transaction],
) -> Result<Vec<SuggestedSipInstallment>> {
    let mf_sips: Vec<&Transaction> = sips
        .iter()
        .filter(|s| {
            s.symbol
                .as_deref()
                .is_some_and(|sym| parse_mf_symbol(sym).is_some())
        })
        .collect();
    if mf_sips.is_empty() {
        return Ok(Vec::new());
    }

    let covered_months = load_sip_covered_months(pool, portfolio_id).await?;
    let today = Utc::now().format("%Y-%m-%d").to_string();

    let mut groups: HashMap<(String, i64), Vec<&Transaction>> = HashMap::new();
    for sip in mf_sips {
        let sym = sip.symbol.clone().unwrap_or_default();
        let amount = sip
            .net_amount
            .or(sip.gross_amount)
            .unwrap_or(0.0);
        let key = (sym, (amount * 100.0).round() as i64);
        groups.entry(key).or_default().push(sip);
    }

    let mut out = Vec::new();
    for ((symbol, amount_cents), group) in groups {
        let amount = amount_cents as f64 / 100.0;
        if amount <= 0.0 {
            continue;
        }
        let sip_day = infer_sip_day(&group);
        let last_date = group
            .iter()
            .map(|s| s.trade_date.as_str())
            .max()
            .unwrap_or("")
            .to_string();
        if last_date.is_empty() {
            continue;
        }

        let mut cursor = NaiveDate::parse_from_str(&last_date, "%Y-%m-%d")
            .map_err(|e| Error::Other(e.to_string()))?;
        cursor = add_months(cursor, 1);
        cursor = clamp_day(cursor, sip_day);

        while cursor.format("%Y-%m-%d").to_string() <= today {
            let date = cursor.format("%Y-%m-%d").to_string();
            let month_key = format!("{}:{}", symbol, &date[..7]);
            if !covered_months.contains(&month_key) {
                let suggestion_id = format!("sip_new:{symbol}:{date}:{amount_cents}");
                out.push(SuggestedSipInstallment {
                    suggestion_id,
                    symbol: symbol.clone(),
                    trade_date: date,
                    amount,
                });
            }
            cursor = add_months(cursor, 1);
            cursor = clamp_day(cursor, sip_day);
        }
    }

    out.sort_by(|a, b| {
        a.symbol
            .cmp(&b.symbol)
            .then(a.trade_date.cmp(&b.trade_date))
    });
    Ok(out)
}

async fn load_sip_covered_months(
    pool: &SqlitePool,
    portfolio_id: i64,
) -> Result<HashSet<String>> {
    let rows = sqlx::query(
        "SELECT symbol, trade_date FROM transactions
         WHERE portfolio_id = ? AND symbol LIKE 'MF:%'
         AND (txn_type = 'sip' OR (txn_type = 'buy' AND source = 'sip_refresh'))",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let mut out = HashSet::new();
    for row in rows {
        let symbol: String = row.try_get("symbol")?;
        let trade_date: String = row.try_get("trade_date")?;
        if trade_date.len() >= 7 {
            out.insert(format!("{}:{}", symbol, &trade_date[..7]));
        }
    }
    Ok(out)
}

fn infer_sip_day(group: &[&Transaction]) -> u32 {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for sip in group {
        if let Ok(d) = NaiveDate::parse_from_str(&sip.trade_date, "%Y-%m-%d") {
            *counts.entry(d.day()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(day, _)| day)
        .unwrap_or(1)
}

fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let mut year = date.year();
    let mut month = date.month() as i32 + months;
    while month > 12 {
        month -= 12;
        year += 1;
    }
    while month < 1 {
        month += 12;
        year -= 1;
    }
    let day = date.day().min(days_in_month(year, month as u32));
    NaiveDate::from_ymd_opt(year, month as u32, day).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(
        if month == 12 {
            year + 1
        } else {
            year
        },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .map(|d| d.pred_opt().map(|p| p.day()).unwrap_or(28))
    .unwrap_or(28)
}

fn clamp_day(date: NaiveDate, day: u32) -> NaiveDate {
    let max_day = days_in_month(date.year(), date.month());
    let d = day.min(max_day);
    NaiveDate::from_ymd_opt(date.year(), date.month(), d).unwrap_or(date)
}

async fn insert_corporate_action(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    corp: &SuggestedCorporateAction,
) -> Result<()> {
    let txn_type = TransactionType::parse(&corp.txn_type)
        .ok_or_else(|| Error::InvalidInput(format!("unknown txn type {}", corp.txn_type)))?;
    let input = NewTransaction {
        portfolio_id,
        txn_type,
        trade_date: corp.trade_date.clone(),
        symbol: Some(corp.symbol.clone()),
        quantity: None,
        price: None,
        gross_amount: corp.gross_amount,
        brokerage: None,
        taxes: None,
        net_amount: corp.gross_amount,
        split_ratio_num: corp.split_ratio_num,
        split_ratio_den: corp.split_ratio_den,
        bonus_ratio_num: None,
        bonus_ratio_den: None,
        dividend_per_share: corp.dividend_per_share,
        tds: None,
        eligible_quantity: corp.eligible_quantity,
        notes: Some("Applied via portfolio refresh".into()),
        schedule_id: None,
    };
    transactions::validate_new(&input)?;
    let corp_key = corporate_action_key(&input);
    let now = Utc::now().timestamp();

    sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'portfolio_refresh', ?, ?, ?)",
    )
    .bind(user_id)
    .bind(portfolio_id)
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
    .bind(corp_key.as_deref())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return Error::Conflict("corporate action already applied".into());
            }
        }
        Error::from(e)
    })?;
    Ok(())
}

async fn insert_sip(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    sip: &SuggestedSipInstallment,
) -> Result<i64> {
    let input = NewTransaction {
        portfolio_id,
        txn_type: TransactionType::Sip,
        trade_date: sip.trade_date.clone(),
        symbol: Some(sip.symbol.clone()),
        quantity: None,
        price: None,
        gross_amount: Some(sip.amount),
        brokerage: None,
        taxes: None,
        net_amount: Some(sip.amount),
        split_ratio_num: None,
        split_ratio_den: None,
        bonus_ratio_num: None,
        bonus_ratio_den: None,
        dividend_per_share: None,
        tds: None,
        eligible_quantity: None,
        notes: Some("Registered via portfolio refresh".into()),
        schedule_id: None,
    };
    transactions::validate_new(&input)?;
    let now = Utc::now().timestamp();

    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, created_at, updated_at)
         VALUES (?, ?, 'sip', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'portfolio_refresh', NULL, ?, ?)",
    )
    .bind(user_id)
    .bind(portfolio_id)
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
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

async fn insert_swp(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    swp: &SuggestedSwpInstallment,
) -> Result<i64> {
    let input = NewTransaction {
        portfolio_id,
        txn_type: TransactionType::Swp,
        trade_date: swp.trade_date.clone(),
        symbol: Some(swp.symbol.clone()),
        quantity: None,
        price: None,
        gross_amount: Some(swp.amount),
        brokerage: None,
        taxes: None,
        net_amount: Some(swp.amount),
        split_ratio_num: None,
        split_ratio_den: None,
        bonus_ratio_num: None,
        bonus_ratio_den: None,
        dividend_per_share: None,
        tds: None,
        eligible_quantity: None,
        notes: Some("Registered via portfolio refresh".into()),
        schedule_id: parse_schedule_id_from_suggestion(&swp.suggestion_id),
    };
    transactions::validate_new(&input)?;
    let now = Utc::now().timestamp();

    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, schedule_id, created_at, updated_at)
         VALUES (?, ?, 'swp', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'portfolio_refresh', NULL, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(portfolio_id)
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
    .bind(input.schedule_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

fn parse_schedule_id_from_suggestion(suggestion_id: &str) -> Option<i64> {
    suggestion_id
        .strip_prefix("schedule_new:")
        .and_then(|rest| rest.split(':').next())
        .and_then(|id| id.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::portfolios;

    fn tx(
        portfolio_id: i64,
        txn_type: TransactionType,
        date: &str,
        qty: Option<f64>,
        split_num: Option<f64>,
        split_den: Option<f64>,
    ) -> Transaction {
        Transaction {
            id: 0,
            user_id: 1,
            portfolio_id,
            txn_type,
            trade_date: date.into(),
            symbol: Some("ITC.NS".into()),
            quantity: qty,
            price: None,
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: split_num,
            split_ratio_den: split_den,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
            source: "manual".into(),
            corporate_action_key: None,
            schedule_id: None,
            created_at: 0,
            updated_at: 0,
            labels: vec![],
        }
    }

    #[test]
    fn holdings_qty_at_date_accounts_for_sell_and_split() {
        let txns = vec![
            tx(1, TransactionType::Buy, "2024-01-01", Some(10.0), None, None),
            tx(1, TransactionType::Split, "2024-06-01", None, Some(2.0), Some(1.0)),
            tx(1, TransactionType::Sell, "2024-07-01", Some(5.0), None, None),
        ];
        assert!((holdings_qty_at_date(Some(&txns), "2024-05-01") - 10.0).abs() < 1e-6);
        assert!((holdings_qty_at_date(Some(&txns), "2024-06-15") - 20.0).abs() < 1e-6);
        assert!((holdings_qty_at_date(Some(&txns), "2024-08-01") - 15.0).abs() < 1e-6);
    }

    #[test]
    fn infer_sip_day_uses_mode() {
        let t1 = Transaction {
            trade_date: "2024-01-05".into(),
            ..tx(1, TransactionType::Sip, "2024-01-05", None, None, None)
        };
        let t2 = Transaction {
            trade_date: "2024-02-05".into(),
            ..tx(1, TransactionType::Sip, "2024-02-05", None, None, None)
        };
        let t3 = Transaction {
            trade_date: "2024-03-10".into(),
            ..tx(1, TransactionType::Sip, "2024-03-10", None, None, None)
        };
        assert_eq!(infer_sip_day(&[&t1, &t2, &t3]), 5);
    }

    #[tokio::test]
    async fn detect_missing_sip_installment_after_last_sip() {
        let pool = db::open_memory().await.unwrap();
        let user = crate::auth::ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &crate::models::NewPortfolio {
                name: "Refresh".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let past = (Utc::now() - chrono::Duration::days(60)).format("%Y-%m-%d").to_string();
        transactions::create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Sip,
                trade_date: past.clone(),
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

        let sips = transactions::list(
            &pool,
            user.id,
            &TransactionFilter {
                portfolio_id: Some(portfolio.id),
                txn_type: Some(TransactionType::Sip),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let suggested =
            detect_missing_sip_installments(&pool, portfolio.id, &sips).await.unwrap();
        assert!(!suggested.is_empty());
        assert!(suggested.iter().all(|s| s.amount == 5000.0));
    }

    #[tokio::test]
    async fn apply_corporate_action_is_idempotent() {
        let pool = db::open_memory().await.unwrap();
        let user = crate::auth::ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &crate::models::NewPortfolio {
                name: "CA".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let corp = SuggestedCorporateAction {
            suggestion_id: "dividend:ITC.NS:2024-06-01".into(),
            symbol: "ITC.NS".into(),
            txn_type: "dividend".into(),
            trade_date: "2024-06-01".into(),
            dividend_per_share: Some(2.0),
            eligible_quantity: Some(10.0),
            gross_amount: Some(20.0),
            split_ratio_num: None,
            split_ratio_den: None,
        };
        insert_corporate_action(&pool, user.id, portfolio.id, &corp)
            .await
            .unwrap();
        let err = insert_corporate_action(&pool, user.id, portfolio.id, &corp)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Conflict(_)));
    }
}
