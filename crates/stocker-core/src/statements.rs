//! Helpers for sorting financial statement rows by period end timestamp.

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

/// Annual cash flow rows, oldest first.
pub fn cashflow_annual_asc(bundle: &StatementBundle) -> Vec<&CashflowRow> {
    annual_asc(&bundle.cashflow_annual, |r| r.end_ts)
}

/// Quarterly cash flow rows, oldest first.
pub fn cashflow_quarterly_asc(bundle: &StatementBundle) -> Vec<&CashflowRow> {
    annual_asc(&bundle.cashflow_quarterly, |r| r.end_ts)
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
