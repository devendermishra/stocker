//! Canonical Yahoo quoteSummary parsing and derived report metrics.
//!
//! Missing Yahoo fields stay `None`. Do not coerce them to `0.0`.

use serde_json::Value;

use crate::models::{CashflowRow, Financials, PeerQuote, StatementBundle};
use crate::statements::{cashflow_annual_asc, cashflow_quarterly_asc};

pub fn yahoo_raw_f64(v: &Value) -> f64 {
    yahoo_opt_f64(v).unwrap_or(0.0)
}

pub fn yahoo_opt_f64(v: &Value) -> Option<f64> {
    v.get("raw")
        .and_then(|x| x.as_f64())
        .or_else(|| v.as_f64())
        .filter(|x| x.is_finite())
}

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

pub fn parse_quote_pe(summary_detail: &Value) -> (f64, Option<f64>) {
    let trailing = yahoo_opt_f64(&summary_detail["trailingPE"]).filter(|x| *x > 0.0);
    let forward = yahoo_opt_f64(&summary_detail["forwardPE"]).filter(|x| *x > 0.0);
    (trailing.unwrap_or(0.0), forward)
}

pub fn quote_price_to_book(key_stats: &Value, summary_detail: &Value) -> f64 {
    yahoo_raw_f64(&key_stats["priceToBook"]).max(yahoo_raw_f64(&summary_detail["priceToBook"]))
}

pub fn quote_price_to_sales(key_stats: &Value, summary_detail: &Value) -> f64 {
    yahoo_raw_f64(&key_stats["priceToSalesTrailing12Months"])
        .max(yahoo_raw_f64(&summary_detail["priceToSalesTrailing12Months"]))
}

pub fn parse_roce_from_financial_data(fd: &Value) -> Option<f64> {
    yahoo_opt_f64(&fd["returnOnCapital"]).or_else(|| yahoo_opt_f64(&fd["returnOnInvestedCapital"]))
}

/// Prefer Yahoo EV; otherwise market cap + debt − cash from the same snapshot.
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

/// Statement FCF: CFO − capex only (not Yahoo levered FCF).
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

/// Yield uses statement CFO−capex FCF only (not Yahoo `freeCashflow`).
pub fn report_fcf(_yahoo: Option<f64>, bundle: &StatementBundle) -> Option<f64> {
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

fn pick_income_net(item: &Value) -> (f64, Option<String>) {
    if let Some(v) = yahoo_opt_f64(&item["netIncome"]) {
        (v, Some("Net Income".to_string()))
    } else if let Some(v) = yahoo_opt_f64(&item["netIncomeCommonStockholders"]) {
        (v, Some("Net Income Common Stockholders".to_string()))
    } else {
        (0.0, None)
    }
}

pub fn income_net_from_legacy_item(item: &Value) -> (f64, Option<String>) {
    pick_income_net(item)
}

pub fn financials_from_quote_result(result: &Value) -> Financials {
    let financial_data = &result["financialData"];
    let summary_detail = &result["summaryDetail"];
    let key_stats = &result["defaultKeyStatistics"];
    let price_mod = &result["price"];

    let (trailing_pe, forward_pe) = parse_quote_pe(summary_detail);

    let ex_div = summary_detail["exDividendDate"]
        .get("fmt")
        .and_then(|f| f.as_str())
        .map(String::from);

    let book_value = yahoo_raw_f64(&key_stats["bookValue"]);
    let price_to_book = quote_price_to_book(key_stats, summary_detail);
    let trailing_eps = yahoo_raw_f64(&key_stats["trailingEps"]);
    let forward_eps = yahoo_opt_f64(&key_stats["forwardEps"]).filter(|x| x.is_finite());

    let div_yield = yahoo_raw_f64(&summary_detail["dividendYield"]);
    let payout = yahoo_raw_f64(&summary_detail["payoutRatio"]);

    let rev_growth = yahoo_opt_f64(&financial_data["revenueGrowth"]);
    let earn_growth = yahoo_opt_f64(&financial_data["earningsGrowth"]);

    let reg_change_pct = yahoo_raw_f64(&price_mod["regularMarketChangePercent"]);
    let prev_close = yahoo_raw_f64(&summary_detail["previousClose"])
        .max(yahoo_raw_f64(&price_mod["regularMarketPreviousClose"]));

    let wk_hi = yahoo_raw_f64(&price_mod["fiftyTwoWeekHigh"])
        .max(yahoo_raw_f64(&summary_detail["fiftyTwoWeekHigh"]));
    let wk_lo = yahoo_raw_f64(&price_mod["fiftyTwoWeekLow"])
        .max(yahoo_raw_f64(&summary_detail["fiftyTwoWeekLow"]));

    let beta_ks = yahoo_raw_f64(&key_stats["beta"]);
    let beta_sd = yahoo_raw_f64(&summary_detail["beta"]);
    let beta = if beta_ks > 0.0 { beta_ks } else { beta_sd };

    let revenue = yahoo_raw_f64(&financial_data["totalRevenue"]);
    let ebitda_v = yahoo_raw_f64(&financial_data["ebitda"]);
    let gross_m = yahoo_raw_f64(&financial_data["grossMargins"]);
    let op_m = yahoo_raw_f64(&financial_data["operatingMargins"]);
    let ebitda_m = yahoo_raw_f64(&financial_data["ebitdaMargins"]);
    let ebitda_m = if ebitda_m > 0.0 {
        ebitda_m
    } else if revenue > 0.0 && ebitda_v > 0.0 {
        ebitda_v / revenue
    } else {
        0.0
    };
    let yahoo_ev = yahoo_opt_f64(&financial_data["enterpriseValue"])
        .or_else(|| yahoo_opt_f64(&key_stats["enterpriseValue"]));
    let yahoo_ev_to_ebitda = yahoo_opt_f64(&key_stats["enterpriseToEbitda"])
        .or_else(|| yahoo_opt_f64(&summary_detail["enterpriseToEbitda"]))
        .filter(|x| x.is_finite() && *x > 0.0);
    let cash = yahoo_raw_f64(&financial_data["totalCash"]);
    let roa = yahoo_opt_f64(&financial_data["returnOnAssets"]);
    let roce = parse_roce_from_financial_data(financial_data);
    let ps = quote_price_to_sales(key_stats, summary_detail);
    let vol = yahoo_raw_f64(&price_mod["regularMarketVolume"]);
    let avg10 = yahoo_raw_f64(&price_mod["averageDailyVolume10Day"]);
    let current_ratio = yahoo_opt_f64(&financial_data["currentRatio"]);
    let quick_ratio = yahoo_opt_f64(&financial_data["quickRatio"]);
    let face_value = yahoo_raw_f64(&key_stats["parValue"])
        .max(yahoo_raw_f64(&key_stats["faceValue"]))
        .max(yahoo_raw_f64(&summary_detail["parValue"]));

    let net_income_to_common = yahoo_opt_f64(&financial_data["netIncomeToCommon"]);
    let net_income = net_income_to_common;

    let total_debt = yahoo_raw_f64(&financial_data["totalDebt"]);
    let market_cap = yahoo_raw_f64(&price_mod["marketCap"]);

    Financials {
        revenue,
        net_income,
        net_income_to_common,
        pe_ratio: trailing_pe,
        forward_pe,
        total_debt,
        ebitda: ebitda_v,
        profit_margins: yahoo_raw_f64(&financial_data["profitMargins"]),
        gross_margins: gross_m,
        operating_margins: op_m,
        ebitda_margins: ebitda_m,
        return_on_equity: yahoo_opt_f64(&financial_data["returnOnEquity"]),
        return_on_assets: roa,
        return_on_capital_employed: roce,
        debt_to_equity: normalize_debt_to_equity_opt(yahoo_opt_f64(&financial_data["debtToEquity"])),
        free_cashflow: yahoo_opt_f64(&financial_data["freeCashflow"]),
        operating_cashflow: yahoo_opt_f64(&financial_data["operatingCashflow"]),
        shares_outstanding: yahoo_raw_f64(&key_stats["sharesOutstanding"]),
        market_cap,
        enterprise_value: yahoo_ev.filter(|x| *x > 0.0),
        yahoo_ev_to_ebitda,
        total_cash: cash,
        book_value,
        price_to_book,
        price_to_sales: ps,
        trailing_eps,
        forward_eps,
        dividend_yield: div_yield,
        payout_ratio: payout,
        revenue_growth: rev_growth,
        earnings_growth: earn_growth,
        regular_market_change_percent: reg_change_pct,
        previous_close: prev_close,
        fifty_two_week_high: wk_hi,
        fifty_two_week_low: wk_lo,
        beta: beta.max(0.0),
        ex_dividend_date: ex_div,
        regular_market_volume: vol,
        average_volume_10_day: avg10,
        current_ratio,
        quick_ratio,
        face_value,
        industrial_yahoo_fields_analysis_applicable: true,
    }
}

pub fn peer_quote_from_quote_result(
    result: &Value,
    fallback_symbol: &str,
    officer_pay: f64,
) -> PeerQuote {
    let f = financials_from_quote_result(result);
    let price_mod = &result["price"];
    let symbol = price_mod["symbol"]
        .as_str()
        .unwrap_or(fallback_symbol)
        .to_string();
    let price = yahoo_opt_f64(&price_mod["regularMarketPrice"]).unwrap_or(0.0);
    let ev = resolve_enterprise_value(
        f.enterprise_value,
        f.market_cap,
        f.total_debt,
        f.total_cash,
    );
    let calculated_ev_ebitda = ev_to_ebitda(ev, f.ebitda);
    PeerQuote {
        symbol,
        short_name: price_mod["shortName"].as_str().map(String::from),
        price,
        pe_ratio: f.pe_ratio,
        forward_pe: f.forward_pe,
        price_to_book: f.price_to_book,
        price_to_sales: f.price_to_sales,
        ev_to_ebitda: f.yahoo_ev_to_ebitda.or(calculated_ev_ebitda),
        ev_to_sales: ev_to_sales(ev, f.revenue),
        market_cap: f.market_cap,
        revenue: f.revenue,
        revenue_growth: f.revenue_growth,
        pat_growth: f.earnings_growth,
        ebitda: f.ebitda,
        ebitda_margin: if f.revenue > 0.0 && f.ebitda > 0.0 {
            Some(f.ebitda / f.revenue)
        } else {
            None
        },
        return_on_equity: f.return_on_equity,
        return_on_capital_employed: f.return_on_capital_employed,
        return_on_assets: f.return_on_assets,
        profit_margins: f.profit_margins,
        debt_to_equity: f.debt_to_equity,
        free_cashflow: f.free_cashflow,
        officer_pay,
        average_volume_10_day: f.average_volume_10_day,
        dividend_yield: f.dividend_yield,
        industrial_metrics_analysis_applicable: true,
    }
}

pub fn split_cashflow_fcf(yahoo_fcf: Option<f64>, cfo: f64, capex: f64) -> (Option<f64>, Option<f64>, f64) {
    let calculated = if cfo.abs() > 1e-9 || capex.abs() > 1e-9 {
        Some(cfo - capex)
    } else {
        None
    };
    let canonical = yahoo_fcf.or(calculated).unwrap_or(0.0);
    (yahoo_fcf, calculated, canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CashflowRow;
    use serde_json::json;

    fn raw(v: f64) -> Value {
        json!({ "raw": v })
    }

    #[test]
    fn missing_return_on_equity_is_none() {
        let result = json!({
            "financialData": {},
            "summaryDetail": {},
            "defaultKeyStatistics": {},
            "price": {}
        });
        let f = financials_from_quote_result(&result);
        assert!(f.return_on_equity.is_none());
        assert!(f.free_cashflow.is_none());
        assert!(f.forward_pe.is_none());
        assert!(f.debt_to_equity.is_none());
        assert!(f.enterprise_value.is_none());
        assert!(f.net_income_to_common.is_none());
        assert!(f.net_income.is_none());
        assert_eq!(f.pe_ratio, 0.0);
        assert!(f.yahoo_ev_to_ebitda.is_none());
        assert!(f.revenue_growth.is_none());
        assert!(f.earnings_growth.is_none());
    }

    #[test]
    fn present_roe_and_de_and_ni_to_common() {
        let result = json!({
            "financialData": {
                "returnOnEquity": raw(0.09139),
                "debtToEquity": raw(36.65),
                "netIncomeToCommon": raw(747e9),
                "netIncome": raw(438e9),
                "freeCashflow": raw(218e9),
                "totalRevenue": raw(11.3e12),
            },
            "summaryDetail": { "trailingPE": raw(23.81), "forwardPE": raw(18.37) },
            "defaultKeyStatistics": { "trailingEps": raw(55.27), "enterpriseToEbitda": raw(13.07) },
            "price": { "regularMarketPrice": raw(1316.0), "marketCap": raw(17.81e12) }
        });
        let f = financials_from_quote_result(&result);
        assert!((f.return_on_equity.unwrap() - 0.09139).abs() < 1e-9);
        assert!((f.debt_to_equity.unwrap() - 0.3665).abs() < 1e-9);
        assert!((f.net_income_to_common.unwrap() - 747e9).abs() < 1.0);
        assert!((f.net_income.unwrap() - 747e9).abs() < 1.0);
        assert!((f.pe_ratio - 23.81).abs() < 1e-9);
        assert!((f.forward_pe.unwrap() - 18.37).abs() < 1e-9);
        assert!((f.yahoo_ev_to_ebitda.unwrap() - 13.07).abs() < 1e-9);
        let ey = earnings_yield_eps_pct(1316.0, 55.27, 23.81).unwrap();
        assert!((ey - 100.0 / 23.81).abs() < 0.05);
    }

    #[test]
    fn trailing_pe_does_not_fall_back_to_forward() {
        let result = json!({
            "financialData": {},
            "summaryDetail": { "forwardPE": raw(18.37) },
            "defaultKeyStatistics": {},
            "price": {}
        });
        let f = financials_from_quote_result(&result);
        assert_eq!(f.pe_ratio, 0.0);
        assert!((f.forward_pe.unwrap() - 18.37).abs() < 1e-9);
    }

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
    fn ttm_pat_ignores_yahoo_net_income_when_to_common_missing() {
        let result = json!({
            "financialData": { "netIncome": raw(438e9) },
            "summaryDetail": {},
            "defaultKeyStatistics": {},
            "price": {}
        });
        let f = financials_from_quote_result(&result);
        assert!(f.net_income_to_common.is_none());
        assert!(f.net_income.is_none());
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
