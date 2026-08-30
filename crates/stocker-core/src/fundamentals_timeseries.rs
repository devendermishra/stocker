//! Yahoo `fundamentalsTimeSeries` API (replaces sparse `quoteSummary` statement modules since ~Nov 2024).
//!
//! See: https://github.com/gadicc/yahoo-finance2/issues/965

use std::collections::BTreeMap;

use reqwest::Client;
use serde_json::Value;

use crate::fetcher::http_client;
use crate::models::{BalanceSheetRow, CashflowRow, IncomeStatementRow, StatementBundle};

const FINANCIALS_KEYS: &[&str] = &[
    "TotalRevenue",
    "CostOfRevenue",
    "GrossProfit",
    "EBITDA",
    "EBIT",
    "OperatingIncome",
    "PretaxIncome",
    "InterestExpense",
    "TaxProvision",
    "NetIncome",
    "NetIncomeCommonStockholders",
    "DilutedEPS",
    "ReconciledDepreciation",
    "DepreciationAndAmortization",
];

const BALANCE_KEYS: &[&str] = &[
    "TotalAssets",
    "CurrentAssets",
    "CurrentLiabilities",
    "TotalDebt",
    "StockholdersEquity",
    "CommonStockEquity",
    "TotalEquityGrossMinorityInterest",
    "TotalLiabilitiesNetMinorityInterest",
    "Inventory",
    "AccountsReceivable",
    "GrossAccountsReceivable",
    "CashAndCashEquivalents",
    "OtherShortTermInvestments",
    "CashCashEquivalentsAndShortTermInvestments",
    "RetainedEarnings",
    "Goodwill",
    "GoodwillAndOtherIntangibleAssets",
    "OtherIntangibleAssets",
    "NetLoan",
    "GrossLoan",
    "TotalDeposits",
];

const INCOME_EXTRA_KEYS: &[&str] = &[
    "OtherIncomeExpense",
    "NetInterestIncome",
    "InterestIncome",
];

const CASHFLOW_KEYS: &[&str] = &[
    "OperatingCashFlow",
    "CashFlowFromContinuingOperatingActivities",
    "CapitalExpenditure",
    "FreeCashFlow",
];

fn build_type_param(period: &str, keys: &[&str]) -> String {
    let mut out = String::from(period);
    for key in keys {
        out.push(',');
        out.push_str(period);
        out.push_str(key);
    }
    out
}

fn period_start_secs(years_back: i64) -> i64 {
    let now = chrono::Utc::now().timestamp();
    now - years_back * 365 * 24 * 3600
}

fn normalize_epoch_secs(ts: i64) -> i64 {
    // Yahoo occasionally emits milliseconds.
    if ts.abs() > 10_000_000_000 {
        ts / 1000
    } else {
        ts
    }
}

fn parse_as_of_date_ts(s: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()?;
    Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
}

fn decamel_field(key: &str, period: &str) -> String {
    let short = key
        .strip_prefix(period)
        .or_else(|| key.strip_prefix("trailing"))
        .unwrap_or(key);
    if short.is_empty() {
        return key.to_string();
    }
    let mut chars = short.chars();
    let Some(first) = chars.next() else {
        return short.to_string();
    };
    let rest: String = chars.collect();
    if rest.is_empty() || rest.chars().all(|c| !c.is_ascii_lowercase()) {
        short.to_ascii_lowercase()
    } else {
        format!("{}{}", first.to_ascii_lowercase(), rest)
    }
}

/// One fiscal period: unix `date` + camelCase metric fields (e.g. `totalRevenue`).
#[derive(Default, Clone)]
struct PeriodSnap {
    vals: BTreeMap<String, f64>,
    period_type: String,
}

fn merge_period_maps(target: &mut BTreeMap<i64, PeriodSnap>, add: BTreeMap<i64, PeriodSnap>) {
    for (ts, snap) in add {
        let e = target.entry(ts).or_default();
        e.vals.extend(snap.vals);
        if e.period_type.is_empty()
            || snap.period_type.eq_ignore_ascii_case("3M")
        {
            if !snap.period_type.is_empty() {
                e.period_type = snap.period_type;
            }
        }
    }
}

fn parse_timeseries_response(json: &Value, period: &str) -> BTreeMap<i64, PeriodSnap> {
    let mut keyed: BTreeMap<i64, PeriodSnap> = BTreeMap::new();
    let Some(results) = json["timeseries"]["result"].as_array() else {
        return keyed;
    };

    for result in results {
        let Some(timestamps) = result["timestamp"].as_array() else {
            continue;
        };
        let data_key = result
            .as_object()
            .map(|o| {
                o.keys()
                    .find(|k| *k != "meta" && *k != "timestamp")
                    .map(String::as_str)
            })
            .flatten();
        let Some(data_key) = data_key else {
            continue;
        };
        let Some(series) = result[data_key].as_array() else {
            continue;
        };
        let field = decamel_field(data_key, period);
        for (i, ts_val) in timestamps.iter().enumerate() {
            let item = series.get(i);
            let as_of_ts = item
                .and_then(|v| v.get("asOfDate"))
                .and_then(|d| d.as_str())
                .and_then(parse_as_of_date_ts);
            let array_ts = ts_val
                .as_i64()
                .or_else(|| ts_val.as_f64().map(|f| f as i64))
                .map(normalize_epoch_secs);
            let Some(ts) = as_of_ts.or(array_ts) else {
                continue;
            };
            let raw = item
                .and_then(|v| v["reportedValue"]["raw"].as_f64())
                .filter(|v| v.is_finite());
            let Some(raw) = raw else {
                continue;
            };
            if period == "quarterly" {
                let ptype = item
                    .and_then(|v| v.get("periodType"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if ptype.eq_ignore_ascii_case("12M") || ptype.eq_ignore_ascii_case("TTM") {
                    continue;
                }
            }
            let e = keyed.entry(ts).or_default();
            e.vals.insert(field.clone(), raw);
            if period == "quarterly" {
                let ptype = item
                    .and_then(|v| v.get("periodType"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if !ptype.is_empty() {
                    e.period_type = ptype.to_string();
                }
            } else if e.period_type.is_empty() {
                e.period_type = "12M".to_string();
            }
        }
    }
    keyed
}

async fn fetch_timeseries(
    client: &Client,
    symbol: &str,
    _period: &str,
    type_param: &str,
    period1: i64,
    period2: i64,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://query1.finance.yahoo.com/ws/fundamentals-timeseries/v1/finance/timeseries/{}?symbol={}&period1={}&period2={}&type={}",
        urlencoding::encode(symbol),
        urlencoding::encode(symbol),
        period1,
        period2,
        urlencoding::encode(type_param),
    );
    let res = crate::http_policy::yahoo_get(client, &url).await?;
    let text = res.text().await?;
    let json: Value = serde_json::from_str(&text)?;
    if let Some(err) = json["timeseries"]["error"]["description"].as_str() {
        return Err(format!("Yahoo fundamentalsTimeSeries error: {err}").into());
    }
    Ok(json)
}

async fn fetch_period_maps(
    symbol: &str,
    period: &str,
) -> BTreeMap<i64, PeriodSnap> {
    let client = http_client();
    let period1 = period_start_secs(12);
    let period2 = chrono::Utc::now().timestamp();
    let fin_type = build_type_param(period, FINANCIALS_KEYS);
    let fin_extra_type = build_type_param(period, INCOME_EXTRA_KEYS);
    let bal_type = build_type_param(period, BALANCE_KEYS);
    let cf_type = build_type_param(period, CASHFLOW_KEYS);

    let (fin, fin_extra, bal, cf) = tokio::join!(
        fetch_timeseries(client, symbol, period, &fin_type, period1, period2),
        fetch_timeseries(client, symbol, period, &fin_extra_type, period1, period2),
        fetch_timeseries(client, symbol, period, &bal_type, period1, period2),
        fetch_timeseries(client, symbol, period, &cf_type, period1, period2),
    );

    let mut merged = BTreeMap::new();
    if let Ok(v) = fin {
        merge_period_maps(&mut merged, parse_timeseries_response(&v, period));
    } else if let Err(e) = fin {
        log::warn!("FTS financials {period} for {symbol}: {e}");
    }
    if let Ok(v) = fin_extra {
        merge_period_maps(&mut merged, parse_timeseries_response(&v, period));
    } else if let Err(e) = fin_extra {
        log::warn!("FTS income extras {period} for {symbol}: {e}");
    }
    if let Ok(v) = bal {
        merge_period_maps(&mut merged, parse_timeseries_response(&v, period));
    } else if let Err(e) = bal {
        log::warn!("FTS balance {period} for {symbol}: {e}");
    }
    if let Ok(v) = cf {
        merge_period_maps(&mut merged, parse_timeseries_response(&v, period));
    } else if let Err(e) = cf {
        log::warn!("FTS cashflow {period} for {symbol}: {e}");
    }
    merged
}

fn get_f64(p: &PeriodSnap, keys: &[&str]) -> f64 {
    get_f64_map(&p.vals, keys)
}

fn get_f64_map(p: &BTreeMap<String, f64>, keys: &[&str]) -> f64 {
    for k in keys {
        if let Some(v) = p.get(*k) {
            if v.is_finite() && *v != 0.0 {
                return *v;
            }
        }
    }
    0.0
}

fn get_opt_f64(p: &PeriodSnap, keys: &[&str]) -> Option<f64> {
    get_opt_f64_map(&p.vals, keys)
}

fn get_opt_f64_map(p: &BTreeMap<String, f64>, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(v) = p.get(*k) {
            if v.is_finite() && *v != 0.0 {
                return Some(*v);
            }
        }
    }
    None
}

fn get_opt_any(p: &PeriodSnap, keys: &[&str]) -> Option<f64> {
    get_opt_any_map(&p.vals, keys)
}

fn get_opt_any_map(p: &BTreeMap<String, f64>, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(v) = p.get(*k) {
            if v.is_finite() {
                return Some(*v);
            }
        }
    }
    None
}

fn fmt_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(normalize_epoch_secs(ts), 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn loan_from_period(p: &PeriodSnap) -> (f64, Option<String>) {
    const KEYS: &[(&str, &str)] = &[
        ("netLoan", "Net Loan"),
        ("grossLoan", "Gross Loan"),
        ("loansReceivable", "Loans Receivable"),
    ];
    for (k, label) in KEYS {
        if let Some(v) = get_opt_f64(p, &[k]) {
            return (v, Some((*label).to_string()));
        }
    }
    (0.0, None)
}

fn income_period_keep(p: &PeriodSnap) -> bool {
    get_f64(p, &["totalRevenue"]) > 0.0
        || get_opt_f64(p, &["netIncome"]).is_some()
        || get_opt_f64(p, &["netIncomeCommonStockholders"]).is_some()
        || get_f64(p, &["netInterestIncome"]) != 0.0
        || get_f64(p, &["interestIncome"]) != 0.0
}

fn income_from_period(ts: i64, p: &PeriodSnap) -> IncomeStatementRow {
    let (net_income, net_income_yahoo_row) = if let Some(v) = get_opt_any(p, &["netIncome"]) {
        (v, Some("Net Income".to_string()))
    } else if let Some(v) = get_opt_any(p, &["netIncomeCommonStockholders"]) {
        (v, Some("Net Income Common Stockholders".to_string()))
    } else {
        (0.0, None)
    };
    IncomeStatementRow {
        end_date_fmt: fmt_date(ts),
        end_ts: Some(ts),
        revenue: get_f64(p, &["totalRevenue"]),
        cost_of_revenue: get_f64(p, &["costOfRevenue"]),
        gross_profit: get_f64(p, &["grossProfit"]),
        ebitda: get_f64(p, &["ebitda", "EBITDA"]),
        operating_income: get_f64(p, &["operatingIncome"]),
        ebit: get_f64(p, &["ebit", "EBIT"]),
        pretax_income: get_f64(p, &["pretaxIncome"]),
        interest_expense: get_f64(p, &["interestExpense"]).abs(),
        income_tax_expense: get_f64(p, &["taxProvision"]).abs(),
        depreciation: get_f64(p, &[
            "reconciledDepreciation",
            "depreciationIncomeStatement",
            "depreciationAndAmortizationInIncomeStatement",
            "depreciationAmortizationDepletionIncomeStatement",
        ]),
        net_income,
        net_income_yahoo_row,
        period_type: p.period_type.clone(),
        diluted_eps: get_opt_f64(p, &["dilutedEPS"]),
        other_income_expense: get_f64(p, &["otherIncomeExpense"]),
        net_interest_income: get_f64(p, &["netInterestIncome"]),
        interest_income: get_f64(p, &["interestIncome"]),
        other_income: get_f64(p, &["nonInterestIncome", "otherNonInterestIncome", "otherIncomeExpense"]),
    }
}

fn balance_from_period(ts: i64, p: &PeriodSnap) -> BalanceSheetRow {
    let cash_cce = get_f64(p, &["cashAndCashEquivalents"]);
    let sti = get_f64(p, &["otherShortTermInvestments"]);
    let cash_lumped = get_f64(p, &["cashCashEquivalentsAndShortTermInvestments"]);
    let cash = if cash_lumped.abs() > 1e-9 {
        cash_lumped
    } else {
        cash_cce + sti
    };
    let loans = loan_from_period(p);
    BalanceSheetRow {
        end_date_fmt: fmt_date(ts),
        end_ts: Some(ts),
        cash,
        cash_and_cash_equivalents: cash_cce,
        short_term_investments: sti,
        total_debt: get_f64(p, &["totalDebt"]),
        total_equity: get_f64(p, &["stockholdersEquity"])
            .max(get_f64(p, &["commonStockEquity"]))
            .max(get_f64(p, &["totalEquityGrossMinorityInterest"])),
        total_assets: get_f64(p, &["totalAssets"]),
        total_liabilities: get_f64(p, &["totalLiabilitiesNetMinorityInterest"]),
        current_assets: get_f64(p, &["currentAssets"]),
        current_liabilities: get_f64(p, &["currentLiabilities"]),
        interest_expense: 0.0,
        inventory: get_f64(p, &["inventory"]),
        net_receivables: get_f64(p, &["accountsReceivable"])
            .max(get_f64(p, &["grossAccountsReceivable"])),
        retained_earnings: get_f64(p, &["retainedEarnings"]),
        goodwill: get_f64(p, &["goodwill"]),
        intangible_assets: get_f64(p, &["otherIntangibleAssets"])
            .max(get_f64(p, &["goodwillAndOtherIntangibleAssets"]) - get_f64(p, &["goodwill"])),
        net_loans: loans.0,
        net_loans_yahoo_row: loans.1,
        total_deposits: get_f64(p, &["totalDeposits"]),
    }
}

fn cashflow_from_period(ts: i64, p: &PeriodSnap) -> CashflowRow {
    let cfo = get_f64(p, &["operatingCashFlow"])
        .max(get_f64(p, &["cashFlowFromContinuingOperatingActivities"]));
    let capex = get_f64(p, &["capitalExpenditure"]).abs();
    let yahoo_fcf = get_opt_any(p, &["freeCashFlow"]);
    let calculated = if cfo.abs() > 1e-9 || capex.abs() > 1e-9 {
        Some(cfo - capex)
    } else {
        None
    };
    let fcf = yahoo_fcf.or(calculated).unwrap_or(0.0);
    CashflowRow {
        end_date_fmt: fmt_date(ts),
        end_ts: Some(ts),
        operating_cashflow: cfo,
        capital_expenditure: capex,
        free_cashflow: fcf,
        yahoo_free_cashflow: yahoo_fcf,
        calculated_fcf: calculated,
    }
}

fn maps_to_rows(
    maps: BTreeMap<i64, PeriodSnap>,
) -> (
    Vec<IncomeStatementRow>,
    Vec<BalanceSheetRow>,
    Vec<CashflowRow>,
) {
    let mut income = Vec::new();
    let mut balance = Vec::new();
    let mut cashflow = Vec::new();
    for (ts, p) in maps {
        if income_period_keep(&p) {
            income.push(income_from_period(ts, &p));
        }
        if get_f64(&p, &["totalAssets"]) > 0.0
            || get_f64(&p, &["currentAssets"]) > 0.0
            || get_f64(&p, &["stockholdersEquity"]) > 0.0
            || get_f64(&p, &["commonStockEquity"]) > 0.0
        {
            balance.push(balance_from_period(ts, &p));
        }
        if get_f64(&p, &["operatingCashFlow"]) != 0.0
            || get_f64(&p, &["cashFlowFromContinuingOperatingActivities"]) != 0.0
            || get_f64(&p, &["freeCashFlow"]) != 0.0
        {
            cashflow.push(cashflow_from_period(ts, &p));
        }
    }
    (income, balance, cashflow)
}

/// Fetch annual + quarterly statements via Yahoo fundamentals time series.
pub async fn fetch_statement_bundle_timeseries(symbol: &str) -> StatementBundle {
    let (annual_maps, quarterly_maps) = tokio::join!(
        fetch_period_maps(symbol, "annual"),
        fetch_period_maps(symbol, "quarterly"),
    );

    let (income_annual, balance_annual, cashflow_annual) = maps_to_rows(annual_maps);
    let (income_quarterly, balance_quarterly, cashflow_quarterly) = maps_to_rows(quarterly_maps);

    if income_annual.is_empty() && balance_annual.is_empty() {
        log::warn!("FTS returned no annual statement rows for {symbol}");
    }

    StatementBundle {
        income_annual,
        income_quarterly,
        balance_annual,
        balance_quarterly,
        cashflow_annual,
        cashflow_quarterly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_of_date_beats_stale_timestamp_array() {
        let json = json!({
            "timeseries": {
                "result": [{
                    "timestamp": [1719705600i64],
                    "quarterlyNetIncome": [{
                        "asOfDate": "2026-06-30",
                        "reportedValue": { "raw": 162579100000.0 }
                    }]
                }]
            }
        });
        let map = parse_timeseries_response(&json, "quarterly");
        let ts = parse_as_of_date_ts("2026-06-30").unwrap();
        assert!(map.contains_key(&ts));
        assert!((map[&ts].vals["netIncome"] - 162579100000.0).abs() < 1.0);
        assert!(!map.contains_key(&1719705600));
    }

    #[test]
    fn quarterly_parse_skips_12m_annual_points() {
        let json = json!({
            "timeseries": {
                "result": [{
                    "timestamp": [1i64, 2i64],
                    "quarterlyNetIncome": [
                        {
                            "asOfDate": "2026-03-31",
                            "periodType": "12M",
                            "reportedValue": { "raw": 700e9 }
                        },
                        {
                            "asOfDate": "2026-06-30",
                            "periodType": "3M",
                            "reportedValue": { "raw": 181e9 }
                        }
                    ]
                }]
            }
        });
        let map = parse_timeseries_response(&json, "quarterly");
        assert!(!map.contains_key(&parse_as_of_date_ts("2026-03-31").unwrap()));
        let q = &map[&parse_as_of_date_ts("2026-06-30").unwrap()];
        assert_eq!(q.period_type, "3M");
        assert!((q.vals["netIncome"] - 181e9).abs() < 1.0);
    }

    #[test]
    fn income_kept_when_only_common_stockholders_ni() {
        let mut p = PeriodSnap::default();
        p.vals.insert("netIncomeCommonStockholders".into(), 181e9);
        assert!(income_period_keep(&p));
        let row = income_from_period(parse_as_of_date_ts("2026-06-30").unwrap(), &p);
        assert!((row.net_income - 181e9).abs() < 1.0);
    }
}
