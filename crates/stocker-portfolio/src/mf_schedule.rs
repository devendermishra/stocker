//! MF SIP/SWP schedule registration, installment backfill, and lifecycle.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};
use stocker_mf::MfService;

use crate::engine::rebuild;
use crate::error::{Error, Result};
use crate::models::{
    MfSchedule, RegisterMfSchedule, RegisterMfScheduleResult, ScheduleFailure, ScheduleStatus,
    ScheduleType, TransactionType,
};
use crate::sip_refresh::materialize_sip;
use crate::swp_refresh::materialize_swp;
use crate::transactions::{self};

#[derive(Debug, Clone)]
pub struct InstallmentDate {
    pub trade_date: String,
}

pub fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
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
    let day = date.day();
    let max_day = days_in_month(year, month as u32);
    NaiveDate::from_ymd_opt(year, month as u32, day.min(max_day)).unwrap_or(date)
}

pub fn clamp_day(date: NaiveDate, day: u32) -> NaiveDate {
    let max_day = days_in_month(date.year(), date.month());
    NaiveDate::from_ymd_opt(date.year(), date.month(), day.min(max_day)).unwrap_or(date)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(
        if month == 12 { year + 1 } else { year },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .and_then(|d| d.pred_opt())
    .map(|d| d.day())
    .unwrap_or(28)
}

pub fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::InvalidInput(format!("invalid date {s}: {e}")))
}

pub fn today_string() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

/// Compute monthly installment dates from start through effective end.
pub fn compute_installment_dates(
    start_date: &str,
    sip_day: u32,
    end_date: Option<&str>,
    installment_count: Option<u32>,
    upper_bound: &str,
) -> Result<Vec<InstallmentDate>> {
    let start = parse_date(start_date)?;
    let upper = parse_date(upper_bound)?;

    let last_by_count = installment_count.map(|n| {
        if n == 0 {
            start
        } else {
            add_months(start, (n as i32) - 1)
        }
    });

    let last_by_end = end_date.and_then(|e| parse_date(e).ok());

    let mut last = upper;
    if let Some(d) = last_by_count {
        if d < last {
            last = d;
        }
    }
    if let Some(d) = last_by_end {
        if d < last {
            last = d;
        }
    }

    let mut out = Vec::new();
    let mut cursor = clamp_day(start, sip_day);
    let max_installments = installment_count.unwrap_or(u32::MAX);

    while cursor <= last && out.len() < max_installments as usize {
        out.push(InstallmentDate {
            trade_date: cursor.format("%Y-%m-%d").to_string(),
        });
        cursor = add_months(cursor, 1);
        cursor = clamp_day(cursor, sip_day);
    }

    Ok(out)
}

pub async fn load_covered_months(
    pool: &SqlitePool,
    portfolio_id: i64,
    schedule_type: ScheduleType,
) -> Result<HashSet<String>> {
    let txn_type = schedule_type.txn_type().as_str();
    let materialized_source = match schedule_type {
        ScheduleType::Sip => "sip_refresh",
        ScheduleType::Swp => "swp_refresh",
    };

    let rows = sqlx::query(
        "SELECT symbol, trade_date FROM transactions
         WHERE portfolio_id = ? AND symbol LIKE 'MF:%'
         AND (txn_type = ? OR (source = ? AND txn_type IN ('buy', 'sell')))",
    )
    .bind(portfolio_id)
    .bind(txn_type)
    .bind(materialized_source)
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

pub async fn count_schedule_installments(
    pool: &SqlitePool,
    schedule_id: i64,
) -> Result<i32> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions WHERE schedule_id = ?",
    )
    .bind(schedule_id)
    .fetch_one(pool)
    .await?;
    Ok(count as i32)
}

pub fn should_inactivate(
    status: ScheduleStatus,
    end_date: Option<&str>,
    installment_count: Option<i32>,
    registered: i32,
    today: &str,
) -> bool {
    if status == ScheduleStatus::Inactive {
        return true;
    }
    if let Some(end) = end_date {
        if today > end {
            return true;
        }
    }
    if let Some(count) = installment_count {
        if registered >= count {
            return true;
        }
    }
    false
}

pub async fn register_mf_schedule(
    pool: &SqlitePool,
    mf: Option<Arc<MfService>>,
    user_id: i64,
    portfolio_id: i64,
    symbol_resolved: &str,
    input: &RegisterMfSchedule,
) -> Result<RegisterMfScheduleResult> {
    if input.amount <= 0.0 {
        return Err(Error::InvalidInput("amount must be positive".into()));
    }
    if input.end_date.is_some() && input.installment_count.is_some() {
        return Err(Error::InvalidInput(
            "provide either end_date or installment_count, not both".into(),
        ));
    }

    let start_date = input
        .start_date
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(today_string);
    parse_date(&start_date)?;

    if let Some(ref end) = input.end_date {
        parse_date(end)?;
        if end.as_str() < start_date.as_str() {
            return Err(Error::InvalidInput(
                "end_date must be on or after start_date".into(),
            ));
        }
    }

    let start = parse_date(&start_date)?;
    let sip_day = start.day() as i32;
    let today = today_string();
    let upper = today.as_str();

    let now = Utc::now().timestamp();
    let res = sqlx::query(
        "INSERT INTO mf_schedules (user_id, portfolio_id, schedule_type, symbol, amount,
         start_date, end_date, installment_count, sip_day, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .bind(input.schedule_type.as_str())
    .bind(symbol_resolved)
    .bind(input.amount)
    .bind(&start_date)
    .bind(input.end_date.as_deref())
    .bind(input.installment_count.map(|n| n as i32))
    .bind(sip_day)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    let schedule_id = res.last_insert_rowid();
    let dates = compute_installment_dates(
        &start_date,
        sip_day as u32,
        input.end_date.as_deref(),
        input.installment_count,
        upper,
    )?;

    let covered = load_covered_months(pool, portfolio_id, input.schedule_type).await?;
    let mut registered = Vec::new();
    let mut materialized = Vec::new();
    let mut skipped_months = Vec::new();
    let mut failed = Vec::new();

    let txn_type = input.schedule_type.txn_type();

    for inst in dates {
        let month_key = format!("{}:{}", symbol_resolved, &inst.trade_date[..7]);
        if covered.contains(&month_key) {
            skipped_months.push(month_key);
            continue;
        }

        let txn_id = match insert_installment(
            pool,
            user_id,
            portfolio_id,
            schedule_id,
            txn_type,
            symbol_resolved,
            &inst.trade_date,
            input.amount,
        )
        .await
        {
            Ok(id) => id,
            Err(e) => {
                failed.push(ScheduleFailure {
                    trade_date: inst.trade_date.clone(),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        registered.push(txn_id);

        let txn = transactions::get(pool, user_id, txn_id).await?;
        let mat_result = match input.schedule_type {
            ScheduleType::Sip => {
                materialize_sip(pool, mf.as_deref(), user_id, &txn).await
            }
            ScheduleType::Swp => {
                materialize_swp(pool, mf.as_deref(), user_id, &txn).await
            }
        };
        match mat_result {
            Ok(buy_sell_id) => materialized.push(buy_sell_id),
            Err(e) => failed.push(ScheduleFailure {
                trade_date: inst.trade_date,
                reason: e.to_string(),
            }),
        }
    }

    let registered_count = count_schedule_installments(pool, schedule_id).await?;
    let status = if should_inactivate(
        ScheduleStatus::Active,
        input.end_date.as_deref(),
        input.installment_count.map(|n| n as i32),
        registered_count,
        &today,
    ) {
        ScheduleStatus::Inactive
    } else {
        ScheduleStatus::Active
    };

    if status == ScheduleStatus::Inactive {
        sqlx::query(
            "UPDATE mf_schedules SET status = 'inactive', updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now().timestamp())
        .bind(schedule_id)
        .execute(pool)
        .await?;
    }

    if !registered.is_empty() || !materialized.is_empty() {
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(RegisterMfScheduleResult {
        schedule_id,
        registered,
        materialized,
        skipped_months,
        status,
        failed,
    })
}

async fn insert_installment(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    schedule_id: i64,
    txn_type: TransactionType,
    symbol: &str,
    trade_date: &str,
    amount: f64,
) -> Result<i64> {
    let now = Utc::now().timestamp();
    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, schedule_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, NULL, NULL, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
         'mf_schedule', NULL, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .bind(txn_type.as_str())
    .bind(trade_date)
    .bind(symbol)
    .bind(amount)
    .bind(amount)
    .bind(schedule_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(res.last_insert_rowid())
}

pub async fn list_mf_schedules(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    mf: Option<&MfService>,
) -> Result<Vec<MfSchedule>> {
    let rows = sqlx::query(
        "SELECT id, user_id, portfolio_id, schedule_type, symbol, amount, start_date, end_date,
         installment_count, sip_day, status, created_at, updated_at
         FROM mf_schedules WHERE user_id = ? AND portfolio_id = ?
         ORDER BY status ASC, start_date DESC, id DESC",
    )
    .bind(user_id)
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for row in rows {
        let id: i64 = row.try_get("id")?;
        let schedule_type_str: String = row.try_get("schedule_type")?;
        let schedule_type = ScheduleType::parse(&schedule_type_str)
            .ok_or_else(|| Error::Other(format!("unknown schedule_type {schedule_type_str}")))?;
        let status_str: String = row.try_get("status")?;
        let status = ScheduleStatus::parse(&status_str)
            .ok_or_else(|| Error::Other(format!("unknown status {status_str}")))?;
        let symbol: String = row.try_get("symbol")?;
        let scheme_name = scheme_name_for_symbol(mf, &symbol).await;

        let registered_installments = count_schedule_installments(pool, id).await?;

        out.push(MfSchedule {
            id,
            user_id: row.try_get("user_id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            schedule_type,
            symbol: symbol.clone(),
            scheme_name,
            amount: row.try_get("amount")?,
            start_date: row.try_get("start_date")?,
            end_date: row.try_get("end_date")?,
            installment_count: row.try_get("installment_count")?,
            sip_day: row.try_get("sip_day")?,
            status,
            registered_installments,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

async fn scheme_name_for_symbol(mf: Option<&MfService>, symbol: &str) -> Option<String> {
    let code = stocker_mf::parse_mf_symbol(symbol)?;
    if let Some(svc) = mf {
        if let Ok(nav) = svc.load_scheme_meta(code).await {
            return Some(nav.scheme_name);
        }
    }
    None
}

pub async fn inactivate_schedule(
    pool: &SqlitePool,
    user_id: i64,
    schedule_id: i64,
) -> Result<MfSchedule> {
    let now = Utc::now().timestamp();
    let res = sqlx::query(
        "UPDATE mf_schedules SET status = 'inactive', updated_at = ?
         WHERE id = ? AND user_id = ?",
    )
    .bind(now)
    .bind(schedule_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }

    let row = sqlx::query(
        "SELECT id, user_id, portfolio_id, schedule_type, symbol, amount, start_date, end_date,
         installment_count, sip_day, status, created_at, updated_at
         FROM mf_schedules WHERE id = ?",
    )
    .bind(schedule_id)
    .fetch_one(pool)
    .await?;

    let schedule_type_str: String = row.try_get("schedule_type")?;
    let schedule_type = ScheduleType::parse(&schedule_type_str)
        .ok_or_else(|| Error::Other(format!("unknown schedule_type {schedule_type_str}")))?;
    let status_str: String = row.try_get("status")?;
    let status = ScheduleStatus::parse(&status_str)
        .ok_or_else(|| Error::Other(format!("unknown status {status_str}")))?;
    let registered_installments = count_schedule_installments(pool, schedule_id).await?;

    Ok(MfSchedule {
        id: schedule_id,
        user_id: row.try_get("user_id")?,
        portfolio_id: row.try_get("portfolio_id")?,
        schedule_type,
        symbol: row.try_get("symbol")?,
        scheme_name: None,
        amount: row.try_get("amount")?,
        start_date: row.try_get("start_date")?,
        end_date: row.try_get("end_date")?,
        installment_count: row.try_get("installment_count")?,
        sip_day: row.try_get("sip_day")?,
        status,
        registered_installments,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub async fn get_active_schedules(
    pool: &SqlitePool,
    portfolio_id: i64,
) -> Result<Vec<MfSchedule>> {
    let rows = sqlx::query(
        "SELECT id, user_id, portfolio_id, schedule_type, symbol, amount, start_date, end_date,
         installment_count, sip_day, status, created_at, updated_at
         FROM mf_schedules WHERE portfolio_id = ? AND status = 'active'
         ORDER BY id ASC",
    )
    .bind(portfolio_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for row in rows {
        let id: i64 = row.try_get("id")?;
        let schedule_type_str: String = row.try_get("schedule_type")?;
        let schedule_type = ScheduleType::parse(&schedule_type_str)
            .ok_or_else(|| Error::Other(format!("unknown schedule_type {schedule_type_str}")))?;
        let status_str: String = row.try_get("status")?;
        let status = ScheduleStatus::parse(&status_str)
            .ok_or_else(|| Error::Other(format!("unknown status {status_str}")))?;
        let registered_installments = count_schedule_installments(pool, id).await?;

        out.push(MfSchedule {
            id,
            user_id: row.try_get("user_id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            schedule_type,
            symbol: row.try_get("symbol")?,
            scheme_name: None,
            amount: row.try_get("amount")?,
            start_date: row.try_get("start_date")?,
            end_date: row.try_get("end_date")?,
            installment_count: row.try_get("installment_count")?,
            sip_day: row.try_get("sip_day")?,
            status,
            registered_installments,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

pub async fn suggest_missing_for_schedule(
    pool: &SqlitePool,
    schedule: &MfSchedule,
) -> Result<Vec<InstallmentDate>> {
    if schedule.status != ScheduleStatus::Active {
        return Ok(Vec::new());
    }

    let today = today_string();
    let last_registered = sqlx::query_scalar::<_, Option<String>>(
        "SELECT MAX(trade_date) FROM transactions WHERE schedule_id = ?",
    )
    .bind(schedule.id)
    .fetch_one(pool)
    .await?
    .unwrap_or_else(|| schedule.start_date.clone());

    let sip_day = schedule.sip_day as u32;
    let mut cursor = parse_date(&last_registered)?;
    cursor = add_months(cursor, 1);
    cursor = clamp_day(cursor, sip_day);

    let upper = parse_date(&today)?;
    let last_by_count = schedule.installment_count.map(|n| {
        let start = parse_date(&schedule.start_date).unwrap_or(cursor);
        add_months(start, (n as i32) - 1)
    });
    let last_by_end = schedule
        .end_date
        .as_deref()
        .and_then(|e| parse_date(e).ok());

    let mut last = upper;
    if let Some(d) = last_by_count {
        if d < last {
            last = d;
        }
    }
    if let Some(d) = last_by_end {
        if d < last {
            last = d;
        }
    }

    let covered = load_covered_months(pool, schedule.portfolio_id, schedule.schedule_type).await?;
    let max_count = schedule
        .installment_count
        .map(|c| c - schedule.registered_installments)
        .unwrap_or(i32::MAX);
    let mut out = Vec::new();
    let mut added = 0i32;

    while cursor <= last && added < max_count {
        let date = cursor.format("%Y-%m-%d").to_string();
        let month_key = format!("{}:{}", schedule.symbol, &date[..7]);
        if !covered.contains(&month_key) {
            out.push(InstallmentDate { trade_date: date });
            added += 1;
        }
        cursor = add_months(cursor, 1);
        cursor = clamp_day(cursor, sip_day);
    }

    Ok(out)
}

pub async fn refresh_active_schedules(
    pool: &SqlitePool,
    mf: Option<Arc<MfService>>,
    user_id: i64,
    portfolio_id: i64,
    schedule_type: Option<ScheduleType>,
) -> Result<RegisterMfScheduleResult> {
    let schedules = get_active_schedules(pool, portfolio_id).await?;
    let mut all_registered = Vec::new();
    let mut all_materialized = Vec::new();
    let mut all_skipped = Vec::new();
    let mut all_failed = Vec::new();
    let mut last_schedule_id = 0i64;
    let mut last_status = ScheduleStatus::Active;

    for schedule in schedules {
        if schedule_type.is_some_and(|t| schedule.schedule_type != t) {
            continue;
        }
        last_schedule_id = schedule.id;
        let suggestions = suggest_missing_for_schedule(pool, &schedule).await?;
        if suggestions.is_empty() {
            let today = today_string();
            if should_inactivate(
                schedule.status,
                schedule.end_date.as_deref(),
                schedule.installment_count,
                schedule.registered_installments,
                &today,
            ) {
                inactivate_schedule(pool, user_id, schedule.id).await?;
                last_status = ScheduleStatus::Inactive;
            }
            continue;
        }

        for inst in suggestions {
            let txn_type = schedule.schedule_type.txn_type();
            let txn_id = insert_installment(
                pool,
                user_id,
                portfolio_id,
                schedule.id,
                txn_type,
                &schedule.symbol,
                &inst.trade_date,
                schedule.amount,
            )
            .await?;
            all_registered.push(txn_id);

            let txn = transactions::get(pool, user_id, txn_id).await?;
            let mat = match schedule.schedule_type {
                ScheduleType::Sip => materialize_sip(pool, mf.as_deref(), user_id, &txn).await,
                ScheduleType::Swp => materialize_swp(pool, mf.as_deref(), user_id, &txn).await,
            };
            match mat {
                Ok(id) => all_materialized.push(id),
                Err(e) => all_failed.push(ScheduleFailure {
                    trade_date: inst.trade_date,
                    reason: e.to_string(),
                }),
            }
        }

        let registered_count = count_schedule_installments(pool, schedule.id).await?;
        let today = today_string();
        if should_inactivate(
            ScheduleStatus::Active,
            schedule.end_date.as_deref(),
            schedule.installment_count,
            registered_count,
            &today,
        ) {
            inactivate_schedule(pool, user_id, schedule.id).await?;
            last_status = ScheduleStatus::Inactive;
        }
    }

    if !all_registered.is_empty() || !all_materialized.is_empty() {
        rebuild::rebuild(pool, portfolio_id).await?;
    }

    Ok(RegisterMfScheduleResult {
        schedule_id: last_schedule_id,
        registered: all_registered,
        materialized: all_materialized,
        skipped_months: all_skipped,
        status: last_status,
        failed: all_failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ensure_local_user;
    use crate::models::NewPortfolio;
    use crate::portfolios;

    #[test]
    fn compute_installment_dates_respects_count() {
        let dates = compute_installment_dates(
            "2024-01-05",
            5,
            None,
            Some(3),
            "2024-12-31",
        )
        .unwrap();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0].trade_date, "2024-01-05");
        assert_eq!(dates[1].trade_date, "2024-02-05");
        assert_eq!(dates[2].trade_date, "2024-03-05");
    }

    #[test]
    fn compute_installment_dates_respects_end_date() {
        let dates = compute_installment_dates(
            "2024-01-05",
            5,
            Some("2024-02-05"),
            None,
            "2024-12-31",
        )
        .unwrap();
        assert_eq!(dates.len(), 2);
        assert_eq!(dates.last().unwrap().trade_date, "2024-02-05");
    }

    #[test]
    fn compute_installment_dates_caps_at_today() {
        let dates = compute_installment_dates(
            "2020-01-05",
            5,
            None,
            None,
            "2020-03-05",
        )
        .unwrap();
        assert_eq!(dates.len(), 3);
    }

    #[test]
    fn should_inactivate_on_count() {
        assert!(should_inactivate(
            ScheduleStatus::Active,
            None,
            Some(12),
            12,
            "2024-06-01"
        ));
    }

    #[test]
    fn should_inactivate_on_end_date() {
        assert!(should_inactivate(
            ScheduleStatus::Active,
            Some("2024-05-01"),
            None,
            5,
            "2024-06-01"
        ));
    }

    #[tokio::test]
    async fn register_rejects_both_end_constraints() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "T".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let err = register_mf_schedule(
            &pool,
            None,
            user.id,
            portfolio.id,
            "MF:122639",
            &RegisterMfSchedule {
                schedule_type: ScheduleType::Sip,
                symbol: "MF:122639".into(),
                amount: 1000.0,
                start_date: Some("2024-01-05".into()),
                end_date: Some("2024-12-05".into()),
                installment_count: Some(12),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("not both"));
    }
}
