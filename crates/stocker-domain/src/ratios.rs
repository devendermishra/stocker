//! Statement and quote ratios. Yahoo JSON parsing stays in the market-data adapter.

use crate::models::{CashflowRow, StatementBundle};
use crate::statements::{cashflow_annual_asc, cashflow_quarterly_asc};

/// Yahoo reports `debtToEquity` as (total debt / equity) × 100 (e.g. 36.65 → 0.3665×).
pub fn normalize_debt_to_equity(raw: f64) -> f64 {
    if !raw.is_finite() {
        return 0.0;
    }
    raw / 100.0
}

pub fn normalize_debt_to_equity_opt(raw: Option<f64>) -> Option<f64> {
    raw.filter(|x| x.is_finite()).map(normalize_debt_to_equity)
}

/// Prefer a provided enterprise value; otherwise market cap + debt − cash.
pub fn resolve_enterprise_value(
    yahoo_ev: Option<f64>,
    market_cap: f64,
    total_debt: f64,
    total_cash: f64,
) -> Option<f64> {
    if let Some(ev) = yahoo_ev.filter(|x| x.is_finite() && *x > 0.0) {
        return Some(ev);
    }
    if market_cap > 0.0 {
        Some((market_cap + total_debt - total_cash).max(0.0))
    } else {
        None
    }
}

pub fn ev_to_ebitda(ev: Option<f64>, ebitda: f64) -> Option<f64> {
    match (ev, ebitda) {
        (Some(e), b) if e > 0.0 && b > 0.0 => Some(e / b),
        _ => None,
    }
}

pub fn ev_to_sales(ev: Option<f64>, revenue: f64) -> Option<f64> {
    match (ev, revenue) {
        (Some(e), r) if e > 0.0 && r > 0.0 => Some(e / r),
        _ => None,
    }
}

/// Statement FCF: CFO − capex only (not a vendor's levered free-cash-flow field).
pub fn canonical_statement_fcf(bundle: &StatementBundle) -> Option<f64> {
    let q = cashflow_quarterly_asc(bundle);
    if q.len() >= 4 {
        let sum: f64 = q[q.len() - 4..]
            .iter()
            .map(|r| row_statement_fcf(r).unwrap_or(0.0))
            .sum();
        if sum.abs() > 1e-3 {
            return Some(sum);
        }
    }
    let a = cashflow_annual_asc(bundle);
    a.last().and_then(|r| row_statement_fcf(r)).filter(|x| x.abs() > 1e-3)
}

pub fn row_statement_fcf(row: &CashflowRow) -> Option<f64> {
    row.calculated_fcf.filter(|x| x.is_finite()).or_else(|| {
        if row.operating_cashflow.abs() > 1e-9 || row.capital_expenditure.abs() > 1e-9 {
            Some(row.operating_cashflow - row.capital_expenditure)
        } else {
            None
        }
    })
}

/// Yield uses statement CFO−capex FCF only.
pub fn report_fcf(_vendor_fcf: Option<f64>, bundle: &StatementBundle) -> Option<f64> {
    canonical_statement_fcf(bundle)
}

pub fn fcf_yield_pct(fcf: Option<f64>, market_cap: f64) -> Option<f64> {
    match (fcf, market_cap) {
        (Some(f), m) if m > 0.0 && f.is_finite() => Some((f / m) * 100.0),
        _ => None,
    }
}

/// EPS / price, dropped when it disagrees with 100/PE by more than 10%.
pub fn earnings_yield_eps_pct(price: f64, trailing_eps: f64, pe: f64) -> Option<f64> {
    let from_eps = if price > 0.0 && trailing_eps > 0.0 {
        Some((trailing_eps / price) * 100.0)
    } else {
        None
    };
    let from_pe = if pe > 0.0 { Some(100.0 / pe) } else { None };
    match (from_eps, from_pe) {
        (Some(a), Some(b)) if b.abs() > 1e-9 => {
            if ((a - b) / b).abs() > 0.10 {
                None
            } else {
                Some(a)
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        _ => None,
    }
}

/// Local ROCE = EBIT / (equity + debt − cash). Decimal (0.12 = 12%).
pub fn local_roce(ebit: f64, equity: f64, debt: f64, cash: f64) -> Option<f64> {
    let capital = equity + debt - cash;
    if capital.abs() > 1.0 && ebit.is_finite() {
        Some(ebit / capital)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CashflowRow;

    #[test]
    fn fcf_yield_none_when_fcf_missing() {
        assert!(fcf_yield_pct(None, 17.81e12).is_none());
        let y = fcf_yield_pct(Some(691.97e9), 17.81e12).unwrap();
        assert!((y - 3.885).abs() < 0.02);
    }

    #[test]
    fn earnings_yield_dropped_when_eps_and_pe_disagree() {
        assert!(earnings_yield_eps_pct(100.0, 8.0, 23.81).is_none());
        let ok = earnings_yield_eps_pct(100.0, 4.20, 23.81).unwrap();
        assert!((ok - 4.20).abs() < 0.05);
    }

    #[test]
    fn debt_to_equity_scale() {
        assert!((normalize_debt_to_equity(36.65) - 0.3665).abs() < 1e-9);
        assert!(normalize_debt_to_equity_opt(None).is_none());
    }

    #[test]
    fn statement_fcf_prefers_calculated() {
        let bundle = StatementBundle {
            cashflow_annual: vec![CashflowRow {
                end_date_fmt: "2026-03-31".into(),
                end_ts: Some(1),
                operating_cashflow: 1000.0,
                capital_expenditure: 300.0,
                free_cashflow: 0.0,
                yahoo_free_cashflow: None,
                calculated_fcf: Some(700.0),
            }],
            ..Default::default()
        };
        assert_eq!(canonical_statement_fcf(&bundle), Some(700.0));
    }

    #[test]
    fn ev_falls_back_to_mcap_plus_debt_minus_cash() {
        let ev = resolve_enterprise_value(None, 100.0, 40.0, 10.0).unwrap();
        assert!((ev - 130.0).abs() < 1e-9);
        assert_eq!(ev_to_ebitda(Some(130.0), 10.0), Some(13.0));
    }
}
