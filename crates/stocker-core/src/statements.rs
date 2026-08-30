//! Helpers for sorting financial statement rows by period end timestamp.

use chrono::Datelike;

use crate::models::{BalanceSheetRow, CashflowRow, IncomeStatementRow, StatementBundle};

/// Statement rows sorted oldest-first by `end_ts`.
pub fn annual_asc<T>(rows: &[T], end_ts: impl Fn(&T) -> Option<i64>) -> Vec<&T> {
    let mut v: Vec<&T> = rows.iter().collect();
    v.sort_by_key(|r| end_ts(r).unwrap_or(0));
    v
}

/// Statement rows sorted newest-first by `end_ts`.
pub fn annual_desc<T>(rows: &[T], end_ts: impl Fn(&T) -> Option<i64>) -> Vec<&T> {
    let mut v: Vec<&T> = rows.iter().collect();
    v.sort_by(|a, b| {
        end_ts(b)
            .unwrap_or(0)
            .cmp(&end_ts(a).unwrap_or(0))
    });
    v
}

/// Annual income statements, oldest first.
pub fn income_annual_asc(bundle: &StatementBundle) -> Vec<&IncomeStatementRow> {
    annual_asc(&bundle.income_annual, |r| r.end_ts)
}

/// Annual income statements, newest first.
pub fn income_annual_desc(bundle: &StatementBundle) -> Vec<&IncomeStatementRow> {
    annual_desc(&bundle.income_annual, |r| r.end_ts)
}

/// Quarterly income statements, oldest first.
pub fn income_quarterly_asc(bundle: &StatementBundle) -> Vec<&IncomeStatementRow> {
    annual_asc(&bundle.income_quarterly, |r| r.end_ts)
}

/// Annual balance sheet rows, oldest first.
pub fn balance_annual_asc(bundle: &StatementBundle) -> Vec<&BalanceSheetRow> {
    annual_asc(&bundle.balance_annual, |r| r.end_ts)
}

/// Annual balance sheet rows, newest first.
pub fn balance_annual_desc(bundle: &StatementBundle) -> Vec<&BalanceSheetRow> {
    annual_desc(&bundle.balance_annual, |r| r.end_ts)
}

/// Annual cash flow rows, newest first.
pub fn cashflow_annual_desc(bundle: &StatementBundle) -> Vec<&CashflowRow> {
    annual_desc(&bundle.cashflow_annual, |r| r.end_ts)
}

/// Annual cash flow rows, oldest first.
pub fn cashflow_annual_asc(bundle: &StatementBundle) -> Vec<&CashflowRow> {
    annual_asc(&bundle.cashflow_annual, |r| r.end_ts)
}

/// Quarterly cash flow rows, oldest first.
pub fn cashflow_quarterly_asc(bundle: &StatementBundle) -> Vec<&CashflowRow> {
    annual_asc(&bundle.cashflow_quarterly, |r| r.end_ts)
}

pub fn parse_period_end_date(fmt: &str) -> Option<chrono::NaiveDate> {
    let s = fmt.trim();
    if s.is_empty() {
        return None;
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .or_else(|| chrono::NaiveDate::parse_from_str(s, "%m/%d/%Y").ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(s, "%d-%m-%Y").ok())
}

/// Newest fiscal period-end among the given `end_date_fmt` values (calendar date, not Yahoo metadata).
pub fn latest_period_end_fmt<'a, I>(dates: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    dates
        .into_iter()
        .filter_map(|s| parse_period_end_date(s).map(|d| (d, s.to_string())))
        .max_by_key(|(d, _)| *d)
        .map(|(_, s)| s)
}

/// Newest Yahoo quarterly income period-end (not company-reported latest quarter unless they match).
pub fn latest_yahoo_quarter_end(bundle: &StatementBundle) -> Option<String> {
    latest_quarterly_income_row(bundle).map(|r| r.end_date_fmt.clone())
}

/// Backward-compatible alias of `latest_yahoo_quarter_end`.
pub fn latest_quarter_end(bundle: &StatementBundle) -> Option<String> {
    latest_yahoo_quarter_end(bundle)
}

/// Last Mar/Jun/Sep/Dec quarter-end that should already have results by `as_of` (25-day reporting lag).
pub fn expected_reported_quarter_end(as_of: chrono::NaiveDate) -> chrono::NaiveDate {
    let lag = as_of
        .checked_sub_signed(chrono::Duration::days(25))
        .unwrap_or(as_of);
    let y = lag.year();
    let candidates = [
        chrono::NaiveDate::from_ymd_opt(y, 3, 31),
        chrono::NaiveDate::from_ymd_opt(y, 6, 30),
        chrono::NaiveDate::from_ymd_opt(y, 9, 30),
        chrono::NaiveDate::from_ymd_opt(y, 12, 31),
        chrono::NaiveDate::from_ymd_opt(y - 1, 12, 31),
        chrono::NaiveDate::from_ymd_opt(y - 1, 9, 30),
        chrono::NaiveDate::from_ymd_opt(y - 1, 6, 30),
        chrono::NaiveDate::from_ymd_opt(y - 1, 3, 31),
    ];
    candidates
        .into_iter()
        .flatten()
        .filter(|d| *d <= lag)
        .max()
        .unwrap_or(lag)
}

pub struct YahooQuarterFreshness {
    pub stale: bool,
    pub age_days: Option<i64>,
    pub expected_reported_quarter_end: String,
    pub note: String,
}

pub fn yahoo_quarter_freshness(yahoo_end: Option<&str>, as_of: chrono::NaiveDate) -> YahooQuarterFreshness {
    let expected = expected_reported_quarter_end(as_of);
    let expected_s = expected.format("%Y-%m-%d").to_string();
    let Some(yend) = yahoo_end.and_then(parse_period_end_date) else {
        return YahooQuarterFreshness {
            stale: true,
            age_days: None,
            expected_reported_quarter_end: expected_s,
            note: "Yahoo quarterly income series has no usable quarter-end.".to_string(),
        };
    };
    let age_days = (as_of - yend).num_days();
    let behind_expected = yend < expected;
    let stale = behind_expected || age_days > 100;
    let note = if stale {
        format!(
            "Yahoo quarterly statement data appears stale relative to the company's latest published quarter. Yahoo latest quarter-end is {}; a quarter ending {} should already be in filings (statement age {} days).",
            yend.format("%Y-%m-%d"),
            expected_s,
            age_days
        )
    } else {
        String::new()
    };
    YahooQuarterFreshness {
        stale,
        age_days: Some(age_days),
        expected_reported_quarter_end: expected_s,
        note,
    }
}

/// True quarterly income rows only (excludes 12M/TTM and FY-sized PAT restated on an annual date).
pub fn true_quarterly_income_rows(bundle: &StatementBundle) -> Vec<&IncomeStatementRow> {
    let mut rows: Vec<&IncomeStatementRow> = bundle
        .income_quarterly
        .iter()
        .filter(|r| is_true_quarter(r, &bundle.income_annual))
        .collect();
    if rows.is_empty() {
        rows = bundle.income_quarterly.iter().collect();
    }
    rows.sort_by_key(|r| parse_period_end_date(&r.end_date_fmt));
    rows
}

/// Newest true quarterly income statement by calendar period-end.
pub fn latest_quarterly_income_row(bundle: &StatementBundle) -> Option<&IncomeStatementRow> {
    true_quarterly_income_rows(bundle)
        .into_iter()
        .max_by_key(|r| parse_period_end_date(&r.end_date_fmt))
}

fn is_true_quarter(row: &IncomeStatementRow, annual: &[IncomeStatementRow]) -> bool {
    let pt = row.period_type.to_ascii_uppercase();
    if pt == "12M" || pt == "TTM" || pt == "9M" || pt == "6M" {
        return false;
    }
    let Some(qd) = parse_period_end_date(&row.end_date_fmt) else {
        return true;
    };
    for a in annual {
        let Some(ad) = parse_period_end_date(&a.end_date_fmt) else {
            continue;
        };
        if ad == qd && a.net_income.abs() > 1.0 && row.net_income.abs() > 1.0 {
            let ratio = row.net_income.abs() / a.net_income.abs();
            if ratio > 0.70 {
                return false;
            }
        }
    }
    true
}

pub fn latest_fiscal_year_end(bundle: &StatementBundle) -> Option<String> {
    latest_period_end_fmt(
        bundle
            .income_annual
            .iter()
            .map(|r| r.end_date_fmt.as_str())
            .chain(bundle.balance_annual.iter().map(|r| r.end_date_fmt.as_str()))
            .chain(bundle.cashflow_annual.iter().map(|r| r.end_date_fmt.as_str())),
    )
}

/// Owned statement rows sorted newest-first by `end_ts`.
pub fn sort_owned_desc<T>(mut rows: Vec<T>, end_ts: impl Fn(&T) -> Option<i64>) -> Vec<T> {
    rows.sort_by(|a, b| {
        end_ts(b)
            .unwrap_or(0)
            .cmp(&end_ts(a).unwrap_or(0))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::IncomeStatementRow;

    #[test]
    fn latest_quarter_end_uses_newest_calendar_date_not_stale_timestamp_order() {
        let bundle = StatementBundle {
            income_quarterly: vec![
                IncomeStatementRow {
                    end_date_fmt: "2025-06-30".into(),
                    end_ts: Some(1_800_000_000),
                    net_income: 1.0,
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2026-06-30".into(),
                    end_ts: Some(1_000_000_000),
                    net_income: 162e9,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(latest_quarter_end(&bundle).as_deref(), Some("2026-06-30"));
    }

    #[test]
    fn latest_quarter_end_ignores_fy_balance_and_annual_sized_income() {
        use crate::models::{BalanceSheetRow, CashflowRow};
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                end_date_fmt: "2026-03-31".into(),
                net_income: 700e9,
                ..Default::default()
            }],
            income_quarterly: vec![
                IncomeStatementRow {
                    end_date_fmt: "2026-03-31".into(),
                    period_type: "12M".into(),
                    net_income: 700e9,
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2026-06-30".into(),
                    period_type: "3M".into(),
                    net_income: 181e9,
                    net_income_yahoo_row: Some("Net Income Common Stockholders".into()),
                    ..Default::default()
                },
            ],
            balance_quarterly: vec![BalanceSheetRow {
                end_date_fmt: "2026-03-31".into(),
                total_assets: 1.0,
                ..Default::default()
            }],
            cashflow_quarterly: vec![CashflowRow {
                end_date_fmt: "2026-03-31".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(latest_quarter_end(&bundle).as_deref(), Some("2026-06-30"));
        let row = latest_quarterly_income_row(&bundle).unwrap();
        assert!((row.net_income - 181e9).abs() < 1.0);
    }

    #[test]
    fn yahoo_quarter_from_june_2025_is_stale_on_aug_2026() {
        let as_of = chrono::NaiveDate::from_ymd_opt(2026, 8, 30).unwrap();
        let expected = expected_reported_quarter_end(as_of);
        assert_eq!(expected, chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
        let f = yahoo_quarter_freshness(Some("2025-06-30"), as_of);
        assert!(f.stale);
        assert!(f.age_days.unwrap() > 350);
        assert!(f.note.contains("stale"));
        let fresh = yahoo_quarter_freshness(Some("2026-06-30"), as_of);
        assert!(!fresh.stale);
    }
}
