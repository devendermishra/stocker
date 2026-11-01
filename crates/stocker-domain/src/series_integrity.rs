//! Validate Yahoo/yfinance historical statement series before CAGR and growth scoring.
//!
//! Yahoo can stitch restated, standalone, and consolidated columns into one array.
//! A single incompatible year (e.g. FY26 consolidated vs prior standalone) corrupts CAGR.

use crate::math::cagr;
use crate::models::{AnnualReport, IncomeStatementRow};

/// Consecutive YoY above this, without corroboration, is treated as a possible scope break.
const LATEST_YOY_BREAK: f64 = 0.50;
/// Older steps: more than a doubling is treated as a possible restatement/scope mix.
const OLDER_YOY_BREAK: f64 = 1.0;
/// Latest FY YoY vs Yahoo `revenueGrowth` gap that confirms a stitching anomaly.
const YAHOO_GROWTH_DISAGREE: f64 = 0.35;
/// Latest annual vs Yahoo TTM must sit in this band or the series is the wrong scope.
const QUOTE_ALIGN_LO: f64 = 0.60;
const QUOTE_ALIGN_HI: f64 = 1.50;

#[derive(Debug, Clone, Default)]
pub struct SeriesCheck {
    /// Count of newest points that look like one statement scope (oldest-first values).
    pub suffix_len: usize,
    pub flags: Vec<String>,
}

pub fn aligns_with_quote(latest_annual: f64, quote_ttm: Option<f64>) -> bool {
    let Some(q) = quote_ttm.filter(|x| x.is_finite() && *x > 1.0) else {
        return true;
    };
    if !latest_annual.is_finite() || latest_annual <= 0.0 {
        return true;
    }
    let ratio = latest_annual / q;
    ratio >= QUOTE_ALIGN_LO && ratio <= QUOTE_ALIGN_HI
}

/// `values_asc` is oldest → newest. `yahoo_latest_yoy` is Yahoo's current growth field (decimal).
pub fn check_level_series(values_asc: &[f64], yahoo_latest_yoy: Option<f64>) -> SeriesCheck {
    let n = values_asc.len();
    if n == 0 {
        return SeriesCheck::default();
    }
    let mut flags = Vec::new();
    let mut suffix = 1usize;
    for i in (0..n.saturating_sub(1)).rev() {
        let older = values_asc[i];
        let newer = values_asc[i + 1];
        if older <= 0.0 || newer <= 0.0 || !older.is_finite() || !newer.is_finite() {
            flags.push("Non-positive level in historical series; stopped extending CAGR window.".into());
            break;
        }
        let yoy = newer / older - 1.0;
        let latest_step = i + 1 == n - 1;
        let break_detected = if latest_step {
            if yoy.abs() <= LATEST_YOY_BREAK {
                false
            } else {
                match yahoo_latest_yoy.filter(|g| g.is_finite()) {
                    Some(g) => (yoy - g).abs() > YAHOO_GROWTH_DISAGREE,
                    None => yoy.abs() > OLDER_YOY_BREAK,
                }
            }
        } else {
            yoy.abs() > OLDER_YOY_BREAK
        };
        if break_detected {
            flags.push(format!(
                "POSSIBLE_SCOPE_OR_RESTATEMENT_CHANGE: {:.1}% YoY between consecutive annual points.",
                yoy * 100.0
            ));
            break;
        }
        suffix += 1;
    }
    SeriesCheck {
        suffix_len: suffix,
        flags,
    }
}

pub struct TrailingCagr {
    pub value: Option<f64>,
    pub flags: Vec<String>,
}

/// CAGR over `years` using only a consistent newest suffix. Returns `None` if the window is tainted.
pub fn trailing_cagr_pct(
    values_asc: &[f64],
    years: usize,
    yahoo_latest_yoy: Option<f64>,
) -> TrailingCagr {
    let check = check_level_series(values_asc, yahoo_latest_yoy);
    if check.suffix_len < years + 1 {
        let mut flags = check.flags;
        flags.push("Historical series inconsistent — exclude CAGR.".into());
        return TrailingCagr {
            value: None,
            flags,
        };
    }
    let n = values_asc.len();
    let start = values_asc[n - 1 - years];
    let end = values_asc[n - 1];
    TrailingCagr {
        value: cagr(start, end, years as f64),
        flags: check.flags,
    }
}

pub fn fy_yoy_pct(values_asc: &[f64]) -> Option<f64> {
    let n = values_asc.len();
    if n < 2 {
        return None;
    }
    let older = values_asc[n - 2];
    let newer = values_asc[n - 1];
    if older.abs() < 1e-9 {
        return None;
    }
    let yoy = (newer / older - 1.0) * 100.0;
    yoy.is_finite().then_some(yoy)
}

/// True when the latest two annual points are not a scope-stitching jump.
pub fn latest_step_usable(values_asc: &[f64], yahoo_latest_yoy: Option<f64>) -> bool {
    let n = values_asc.len();
    if n < 2 {
        return false;
    }
    let older = values_asc[n - 2];
    let newer = values_asc[n - 1];
    if older <= 0.0 || newer <= 0.0 {
        return false;
    }
    let yoy = newer / older - 1.0;
    if yoy.abs() <= LATEST_YOY_BREAK {
        return true;
    }
    match yahoo_latest_yoy.filter(|g| g.is_finite()) {
        Some(g) => (yoy - g).abs() <= YAHOO_GROWTH_DISAGREE,
        None => false,
    }
}

pub fn income_levels_asc(rows_asc: &[&IncomeStatementRow], f: impl Fn(&IncomeStatementRow) -> f64) -> Vec<f64> {
    rows_asc.iter().map(|r| f(r)).collect()
}

pub fn annual_reports_from_income(
    rows_desc: &[&IncomeStatementRow],
    rev_check: &SeriesCheck,
    revenue_is_sales: bool,
) -> Vec<AnnualReport> {
    let n = rows_desc.len();
    rows_desc
        .iter()
        .enumerate()
        .map(|(i, r)| {
            // rows_desc is newest first; index 0 is latest. A break leaves only suffix_len newest points clean.
            let from_end = n - i; // 1 = newest
            let warning = if from_end > rev_check.suffix_len {
                Some("Outside consistent statement-scope suffix; not used for CAGR.".to_string())
            } else {
                None
            };
            AnnualReport {
                date: r.end_date_fmt.clone(),
                revenue: if revenue_is_sales { Some(r.revenue) } else { None },
                yahoo_total_revenue_raw: r.revenue,
                revenue_represents_sales: revenue_is_sales,
                net_income: r.net_income,
                net_income_yahoo_row: r.net_income_yahoo_row.clone(),
                pat_scope: "unknown — Yahoo does not label standalone vs consolidated".to_string(),
                series_warning: warning,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_reliance_like_revenue_excludes_cagr() {
        // Standalone-scale history then one consolidated year — the bug that produced ~26% CAGR.
        let mixed = vec![5.29773e12, 5.34534e12, 5.17349e12, 10.57219e12];
        let out = trailing_cagr_pct(&mixed, 3, Some(0.297));
        assert!(out.value.is_none(), "mixed scope must not produce CAGR");
        assert!(out.flags.iter().any(|f| f.contains("SCOPE") || f.contains("inconsistent")));
    }

    #[test]
    fn consolidated_reliance_like_revenue_is_about_6_point_4() {
        let cons = vec![8.77835e12, 9.01064e12, 9.64693e12, 10.57219e12];
        let out = trailing_cagr_pct(&cons, 3, Some(0.297));
        let v = out.value.expect("consistent series should yield CAGR");
        assert!((v - 6.4).abs() < 0.3, "got {v}");
        let yoy = fy_yoy_pct(&cons).unwrap();
        assert!((yoy - 9.6).abs() < 0.4, "FY YoY got {yoy}");
    }

    #[test]
    fn quote_alignment_rejects_standalone_latest_vs_ttm() {
        assert!(!aligns_with_quote(5.17e12, Some(11.30e12)));
        assert!(aligns_with_quote(10.57e12, Some(11.30e12)));
    }

    #[test]
    fn hypergrowth_matching_yahoo_is_kept() {
        let series = vec![100.0, 180.0, 320.0, 550.0];
        let out = trailing_cagr_pct(&series, 3, Some(0.72));
        assert!(out.value.is_some());
    }
}
