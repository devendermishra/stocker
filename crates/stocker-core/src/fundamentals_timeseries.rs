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
pub type TimeseriesPeriod = BTreeMap<String, f64>;

fn merge_period_maps(target: &mut BTreeMap<i64, TimeseriesPeriod>, add: BTreeMap<i64, TimeseriesPeriod>) {
    for (ts, vals) in add {
        target.entry(ts).or_default().extend(vals);
    }
}

fn parse_timeseries_response(json: &Value, period: &str) -> BTreeMap<i64, TimeseriesPeriod> {
    let mut keyed: BTreeMap<i64, TimeseriesPeriod> = BTreeMap::new();
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
            let Some(ts) = ts_val.as_i64() else {
                continue;
            };
            let raw = series
                .get(i)
                .and_then(|v| v["reportedValue"]["raw"].as_f64())
                .filter(|v| v.is_finite());
            let Some(raw) = raw else {
                continue;
            };
            keyed.entry(ts).or_default().insert(field.clone(), raw);
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
) -> BTreeMap<i64, TimeseriesPeriod> {
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

fn get_f64(p: &TimeseriesPeriod, keys: &[&str]) -> f64 {
    for k in keys {
        if let Some(v) = p.get(*k) {
            if v.is_finite() && *v != 0.0 {
                return *v;
            }
        }
    }
    0.0
}

fn get_opt_f64(p: &TimeseriesPeriod, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(v) = p.get(*k) {
            if v.is_finite() && *v != 0.0 {
                return Some(*v);
            }
        }
    }
    None
}

fn fmt_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| ts.to_string())
}

fn income_from_period(ts: i64, p: &TimeseriesPeriod) -> IncomeStatementRow {
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
        net_income: get_f64(p, &["netIncome"]),
        diluted_eps: get_opt_f64(p, &["dilutedEPS"]),
        other_income_expense: get_f64(p, &["otherIncomeExpense"]),
        net_interest_income: get_f64(p, &["netInterestIncome", "interestIncome"]),
    }
}

fn balance_from_period(ts: i64, p: &TimeseriesPeriod) -> BalanceSheetRow {
    let cash = get_f64(p, &["cashAndCashEquivalents"])
        + get_f64(p, &["otherShortTermInvestments"])
        + get_f64(p, &["cashCashEquivalentsAndShortTermInvestments"]);
    BalanceSheetRow {
        end_date_fmt: fmt_date(ts),
        end_ts: Some(ts),
        cash,
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
    }
}

fn cashflow_from_period(ts: i64, p: &TimeseriesPeriod) -> CashflowRow {
    let cfo = get_f64(p, &["operatingCashFlow"])
        .max(get_f64(p, &["cashFlowFromContinuingOperatingActivities"]));
    let capex = get_f64(p, &["capitalExpenditure"]).abs();
    let mut fcf = get_f64(p, &["freeCashFlow"]);
    if fcf == 0.0 && cfo != 0.0 {
        fcf = cfo - capex;
    }
    CashflowRow {
        end_date_fmt: fmt_date(ts),
        end_ts: Some(ts),
        operating_cashflow: cfo,
        capital_expenditure: capex,
        free_cashflow: fcf,
    }
}

fn maps_to_rows(
    maps: BTreeMap<i64, TimeseriesPeriod>,
) -> (
    Vec<IncomeStatementRow>,
    Vec<BalanceSheetRow>,
    Vec<CashflowRow>,
) {
    let mut income = Vec::new();
    let mut balance = Vec::new();
    let mut cashflow = Vec::new();
    for (ts, p) in maps {
        if get_f64(&p, &["totalRevenue"]) > 0.0 || get_f64(&p, &["netIncome"]) != 0.0 {
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
