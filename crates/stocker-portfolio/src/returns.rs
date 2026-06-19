//! Portfolio return metrics — XIRR (preferred) and CAGR (fallback).

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use stocker_core::math::cagr;

use crate::models::{Transaction, TransactionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMethod {
    Xirr,
    Cagr,
    Simple,
}

#[derive(Debug, Clone)]
pub struct ReturnMetrics {
    /// Net profit: current value + dividends + sale proceeds − buy outflows.
    pub total_return: f64,
    /// Annualized return % (XIRR when available, otherwise CAGR).
    pub return_pct: Option<f64>,
    pub return_method: Option<ReturnMethod>,
    /// Cumulative cash invested (buy outflows).
    pub net_invested: f64,
}

#[derive(Debug, Clone)]
struct CashFlow {
    date: NaiveDate,
    amount: f64,
}

pub fn symbol_metrics(
    txns: &[Transaction],
    symbol: &str,
    terminal_value: Option<f64>,
) -> ReturnMetrics {
    let flows = symbol_cash_flows(txns, symbol, terminal_value);
    metrics_from_flows(&flows)
}

pub fn portfolio_metrics(
    txns: &[Transaction],
    terminal_values: &HashMap<String, f64>,
) -> ReturnMetrics {
    let mut flows = Vec::new();
    let mut symbols: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for txn in txns {
        if let Some(sym) = txn.symbol.as_deref() {
            symbols.insert(sym);
        }
    }
    for sym in symbols {
        let terminal = terminal_values.get(sym).copied();
        flows.extend(symbol_cash_flows_raw(txns, sym, terminal));
    }
    metrics_from_flows(&flows)
}

fn symbol_cash_flows(
    txns: &[Transaction],
    symbol: &str,
    terminal_value: Option<f64>,
) -> Vec<CashFlow> {
    symbol_cash_flows_raw(txns, symbol, terminal_value)
}

fn symbol_cash_flows_raw(
    txns: &[Transaction],
    symbol: &str,
    terminal_value: Option<f64>,
) -> Vec<CashFlow> {
    let mut flows: Vec<CashFlow> = txns
        .iter()
        .filter(|t| t.symbol.as_deref() == Some(symbol))
        .filter_map(txn_cash_flow)
        .collect();

    if let Some(tv) = terminal_value {
        if tv >= 0.0 {
            flows.push(CashFlow {
                date: Utc::now().date_naive(),
                amount: tv,
            });
        }
    }
    flows
}

fn metrics_from_flows(flows: &[CashFlow]) -> ReturnMetrics {
    if flows.is_empty() {
        return ReturnMetrics {
            total_return: 0.0,
            return_pct: None,
            return_method: None,
            net_invested: 0.0,
        };
    }

    let net_invested: f64 = flows
        .iter()
        .filter(|f| f.amount < 0.0)
        .map(|f| -f.amount)
        .sum();

    let total_return: f64 = flows.iter().map(|f| f.amount).sum();

    let (return_pct, return_method) = compute_annualized_return(flows, net_invested, total_return);

    ReturnMetrics {
        total_return,
        return_pct,
        return_method,
        net_invested,
    }
}

fn compute_annualized_return(
    flows: &[CashFlow],
    net_invested: f64,
    total_return: f64,
) -> (Option<f64>, Option<ReturnMethod>) {
    if let Some(xirr) = xirr(flows) {
        return (Some(xirr), Some(ReturnMethod::Xirr));
    }

    if net_invested <= 0.0 {
        return (None, None);
    }

    let first = match flows.iter().map(|f| f.date).min() {
        Some(d) => d,
        None => return simple_return_pct(net_invested, total_return),
    };
    let last = match flows.iter().map(|f| f.date).max() {
        Some(d) => d,
        None => return simple_return_pct(net_invested, total_return),
    };
    let days = (last - first).num_days();
    if days <= 0 {
        return simple_return_pct(net_invested, total_return);
    }
    let years = days as f64 / 365.25;

    let ending_value = (net_invested + total_return).max(0.0);
    if ending_value > 0.0 {
        if let Some(pct) = cagr(net_invested, ending_value, years) {
            return (Some(pct), Some(ReturnMethod::Cagr));
        }
    }

    if let Some(pct) = annualized_return_from_total(total_return, net_invested, years) {
        return (Some(pct), Some(ReturnMethod::Cagr));
    }

    simple_return_pct(net_invested, total_return)
}

fn simple_return_pct(net_invested: f64, total_return: f64) -> (Option<f64>, Option<ReturnMethod>) {
    if net_invested <= 0.0 {
        return (None, None);
    }
    let pct = (total_return / net_invested) * 100.0;
    if pct.is_finite() {
        (Some(pct), Some(ReturnMethod::Simple))
    } else {
        (None, None)
    }
}

/// Annualized return from cumulative profit/loss (handles negative and total-loss cases).
fn annualized_return_from_total(total_return: f64, net_invested: f64, years: f64) -> Option<f64> {
    if net_invested <= 0.0 || years <= 0.0 {
        return None;
    }
    let growth = 1.0 + total_return / net_invested;
    if growth <= 0.0 {
        return Some(-100.0);
    }
    let pct = (growth.powf(1.0 / years) - 1.0) * 100.0;
    pct.is_finite().then_some(pct)
}

fn txn_cash_flow(txn: &Transaction) -> Option<CashFlow> {
    let date = parse_trade_date(&txn.trade_date)?;
    let amount = match txn.txn_type {
        TransactionType::OpeningBalance
        | TransactionType::Buy
        | TransactionType::MergerInvestment
        | TransactionType::DemergerInvestment
        | TransactionType::Rights => -buy_cash_out(txn),
        TransactionType::Sell
        | TransactionType::MergerRedemption
        | TransactionType::DemergerRedemption => sell_cash_in(txn),
        TransactionType::Dividend => dividend_cash_in(txn),
        TransactionType::Split | TransactionType::Bonus | TransactionType::Sip | TransactionType::Swp => return None,
    };
    if amount.abs() < 1e-9 {
        return None;
    }
    Some(CashFlow { date, amount })
}

fn buy_cash_out(txn: &Transaction) -> f64 {
    if let Some(net) = txn.net_amount {
        return net.abs();
    }
    let qty = txn.quantity.unwrap_or(0.0);
    let price = txn.price.unwrap_or(0.0);
    let gross = txn.gross_amount.unwrap_or(qty * price).abs();
    gross + txn.brokerage.unwrap_or(0.0).abs() + txn.taxes.unwrap_or(0.0).abs()
}

fn sell_cash_in(txn: &Transaction) -> f64 {
    if let Some(net) = txn.net_amount.filter(|n| n.abs() > 0.0) {
        return net.abs();
    }
    if let Some(gross) = txn.gross_amount.filter(|g| g.abs() > 0.0) {
        return gross.abs();
    }
    let qty = txn.quantity.unwrap_or(0.0);
    let price = txn.price.unwrap_or(0.0);
    (qty * price).abs()
}

fn dividend_cash_in(txn: &Transaction) -> f64 {
    txn.gross_amount
        .or(txn.net_amount)
        .unwrap_or(0.0)
        .abs()
}

fn parse_trade_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

/// Extended internal rate of return. Returns annualized % (e.g. -12.5 = -12.5%).
fn xirr(flows: &[CashFlow]) -> Option<f64> {
    if flows.len() < 2 {
        return None;
    }
    let has_outflow = flows.iter().any(|f| f.amount < 0.0);
    let has_inflow = flows.iter().any(|f| f.amount > 0.0);
    if !has_outflow || !has_inflow {
        return None;
    }

    let base = flows.iter().map(|f| f.date).min()?;
    let year_frac = |d: NaiveDate| -> f64 { (d - base).num_days() as f64 / 365.0 };

    let npv = |rate: f64| -> f64 {
        if rate <= -1.0 {
            return f64::INFINITY;
        }
        flows
            .iter()
            .map(|f| {
                let t = year_frac(f.date);
                f.amount / (1.0 + rate).powf(t)
            })
            .sum()
    };

    let npv_deriv = |rate: f64| -> f64 {
        if rate <= -1.0 {
            return f64::INFINITY;
        }
        flows
            .iter()
            .map(|f| {
                let t = year_frac(f.date);
                -t * f.amount / (1.0 + rate).powf(t + 1.0)
            })
            .sum()
    };

    for guess in [0.1, 0.0, -0.1, -0.25, -0.5, -0.75] {
        if let Some(rate) = newton_xirr(guess, &npv, &npv_deriv) {
            return Some(rate * 100.0);
        }
    }

    let (lo, hi) = find_xirr_bracket(&npv)?;
    bisect_xirr(lo, hi, &npv).map(|rate| rate * 100.0)
}

fn newton_xirr(
    mut rate: f64,
    npv: &dyn Fn(f64) -> f64,
    npv_deriv: &dyn Fn(f64) -> f64,
) -> Option<f64> {
    for _ in 0..64 {
        let value = npv(rate);
        if value.abs() < 1e-7 {
            return rate.is_finite().then_some(rate);
        }
        let deriv = npv_deriv(rate);
        if deriv.abs() < 1e-12 {
            return None;
        }
        let next = rate - value / deriv;
        if (next - rate).abs() < 1e-10 {
            return next.is_finite().then_some(next);
        }
        rate = next.clamp(-0.999, 10.0);
    }
    None
}

fn find_xirr_bracket(npv: &dyn Fn(f64) -> f64) -> Option<(f64, f64)> {
    const RATES: &[f64] = &[
        -0.99, -0.9, -0.75, -0.5, -0.25, -0.1, 0.0, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0,
    ];
    let mut prev_rate = RATES[0];
    let mut prev_val = npv(prev_rate);
    for &rate in &RATES[1..] {
        let val = npv(rate);
        if prev_val.is_finite()
            && val.is_finite()
            && prev_val.signum() != 0.0
            && val.signum() != 0.0
            && prev_val.signum() != val.signum()
        {
            return Some((prev_rate, rate));
        }
        prev_rate = rate;
        prev_val = val;
    }
    None
}

fn bisect_xirr(mut lo: f64, mut hi: f64, npv: &dyn Fn(f64) -> f64) -> Option<f64> {
    let mut f_lo = npv(lo);
    let f_hi = npv(hi);
    if f_lo.signum() == f_hi.signum() {
        return None;
    }
    for _ in 0..256 {
        let mid = (lo + hi) / 2.0;
        let f_mid = npv(mid);
        if f_mid.abs() < 1e-7 || (hi - lo).abs() < 1e-10 {
            return mid.is_finite().then_some(mid);
        }
        if f_mid.signum() == f_lo.signum() {
            lo = mid;
            f_lo = f_mid;
        } else {
            hi = mid;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Transaction;

    fn txn(
        txn_type: TransactionType,
        date: &str,
        symbol: &str,
        net: Option<f64>,
        gross: Option<f64>,
    ) -> Transaction {
        Transaction {
            id: 1,
            user_id: 1,
            portfolio_id: 1,
            txn_type,
            trade_date: date.to_string(),
            symbol: Some(symbol.to_string()),
            quantity: None,
            price: None,
            gross_amount: gross,
            brokerage: None,
            taxes: None,
            net_amount: net,
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
            source: "manual".to_string(),
            corporate_action_key: None,
            schedule_id: None,
            created_at: 0,
            updated_at: 0,
            labels: vec![],
        }
    }

    #[test]
    fn total_return_includes_dividends() {
        let txns = vec![
            txn(TransactionType::Buy, "2020-01-01", "ITC.NS", Some(1000.0), None),
            txn(TransactionType::Dividend, "2021-06-01", "ITC.NS", None, Some(50.0)),
        ];
        let m = symbol_metrics(&txns, "ITC.NS", Some(1200.0));
        assert!((m.total_return - 250.0).abs() < 1e-6);
        assert!((m.net_invested - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn xirr_single_buy_with_dividend_and_terminal() {
        let txns = vec![
            txn(TransactionType::Buy, "2020-01-01", "ITC.NS", Some(1000.0), None),
            txn(TransactionType::Dividend, "2022-01-01", "ITC.NS", None, Some(100.0)),
        ];
        let flows = symbol_cash_flows(&txns, "ITC.NS", Some(1500.0));
        let irr = xirr(&flows).expect("xirr");
        assert!(irr > 5.0);
        assert!(irr < 40.0);
    }

    #[test]
    fn two_point_flows_compute_annualized_return() {
        let flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                amount: 2000.0,
            },
        ];
        let m = metrics_from_flows(&flows);
        assert!(m.return_pct.unwrap() > 0.0);
        assert!((m.total_return - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn negative_xirr_for_loss() {
        let flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                amount: 500.0,
            },
        ];
        let m = metrics_from_flows(&flows);
        assert!((m.total_return + 500.0).abs() < 1e-6);
        let pct = m.return_pct.expect("return pct");
        assert!(pct < 0.0);
        assert!(pct > -100.0);
    }

    #[test]
    fn total_loss_shows_negative_return() {
        let txns = vec![txn(TransactionType::Buy, "2020-01-01", "LOSS.NS", Some(1000.0), None)];
        let m = symbol_metrics(&txns, "LOSS.NS", Some(0.0));
        assert!((m.total_return + 1000.0).abs() < 1e-6);
        let pct = m.return_pct.expect("return pct");
        assert!(pct < 0.0);
    }

    #[test]
    fn loss_with_dividend_still_negative() {
        let txns = vec![
            txn(TransactionType::Buy, "2020-01-01", "LOSS.NS", Some(1000.0), None),
            txn(TransactionType::Dividend, "2022-01-01", "LOSS.NS", None, Some(50.0)),
        ];
        let m = symbol_metrics(&txns, "LOSS.NS", Some(400.0));
        assert!((m.total_return + 550.0).abs() < 1e-6);
        let pct = m.return_pct.expect("return pct");
        assert!(pct < 0.0);
    }
}
