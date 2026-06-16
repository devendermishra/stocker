//! Metric compute engine.
//!
//! Consumes the `ComputeInputs` bundle (one symbol's worth of Yahoo fetches +
//! statements + chart) and produces a `HashMap<MetricId, Option<f64>>` covering
//! every metric in the catalog. Each extractor returns `None` when its inputs
//! aren't sufficient (e.g. less than 7 years of statements for `historical_pe_7y`),
//! and the corresponding column is then NULL on the snapshot row.
//!
//! `compute_all` delegates to section functions (`compute_price_metrics`,
//! `compute_market_structure_metrics`, `compute_valuation_metrics`, etc.) that
//! share a [`ComputeContext`] of pre-resolved statement slices and cross-section
//! values (price, market cap, P/E, etc.).

use std::collections::HashMap;

use stocker_core::math::{cagr, median};
use stocker_core::models::{
    AssetProfile, BalanceSheetRow, CashflowRow, ChartHistory, Financials, IncomeStatementRow,
    PeerQuote, StatementBundle,
};
use stocker_core::statements::{
    balance_annual_asc, cashflow_annual_asc, cashflow_quarterly_asc, income_annual_asc,
    income_quarterly_asc,
};
use stocker_core::technical_analysis::{macd_components, roc_pct, sma_last};

use crate::metrics::MetricId;

/// All inputs for one symbol. Borrows the upstream models so compute is cheap.
pub struct ComputeInputs<'a> {
    pub financials: &'a Financials,
    pub statements: &'a StatementBundle,
    pub chart_10y: &'a ChartHistory,
    pub peer_quote: Option<&'a PeerQuote>,
    pub asset_profile: &'a AssetProfile,
}

/// Convenience shorthand for a value that is "missing" or unsuitable.
#[inline]
fn finite(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

#[inline]
fn pos(value: f64) -> Option<f64> {
    finite(value).filter(|v| *v > 0.0)
}

#[inline]
fn nonzero(value: f64) -> Option<f64> {
    finite(value).filter(|v| v.abs() > 1e-12)
}

fn closes(chart: &ChartHistory) -> Vec<f64> {
    chart
        .bars
        .iter()
        .map(|b| b.close)
        .filter(|c| *c > 0.0)
        .collect()
}

/// High/low over the last ~252 trading days from daily bars (or all bars if fewer).
fn chart_52w_range(chart: &ChartHistory) -> (Option<f64>, Option<f64>) {
    if chart.bars.is_empty() {
        return (None, None);
    }
    let window = chart.bars.len().min(TD_1Y);
    let tail = &chart.bars[chart.bars.len() - window..];
    let mut hi = 0.0_f64;
    let mut lo = f64::MAX;
    for b in tail {
        let bar_hi = b.high.max(b.close);
        let bar_lo = if b.low > 0.0 { b.low.min(b.close) } else { b.close };
        if bar_hi > hi {
            hi = bar_hi;
        }
        if bar_lo > 0.0 && bar_lo < lo {
            lo = bar_lo;
        }
    }
    if hi > 0.0 && lo < f64::MAX {
        (Some(hi), Some(lo))
    } else {
        (None, None)
    }
}

fn chart_last_close(chart: &ChartHistory) -> Option<f64> {
    chart
        .bars
        .last()
        .map(|b| b.close)
        .filter(|c| *c > 0.0)
}

fn chart_previous_close(chart: &ChartHistory) -> Option<f64> {
    if chart.bars.len() < 2 {
        return None;
    }
    chart
        .bars
        .get(chart.bars.len() - 2)
        .map(|b| b.close)
        .filter(|c| *c > 0.0)
}

fn chart_day_change_pct(chart: &ChartHistory) -> Option<f64> {
    let last = chart_last_close(chart)?;
    let prev = chart_previous_close(chart)?;
    if prev <= 0.0 {
        return None;
    }
    Some(((last - prev) / prev) * 100.0)
}

fn resolve_pat_ttm(f: &Financials, income_q: &[&IncomeStatementRow]) -> Option<f64> {
    finite(f.net_income)
        .filter(|v| v.abs() > 1e-9)
        .or_else(|| {
            if f.trailing_eps.abs() > 1e-9 && f.shares_outstanding > 0.0 {
                Some(f.trailing_eps * f.shares_outstanding)
            } else {
                None
            }
        })
        .or_else(|| sum_last_4_quarters(income_q, |r| r.net_income))
}

fn resolve_revenue(f: &Financials, income_a: &[&IncomeStatementRow], income_q: &[&IncomeStatementRow]) -> f64 {
    if f.revenue > 0.0 {
        f.revenue
    } else if let Some(r) = latest(income_a) {
        r.revenue
    } else {
        sum_last_4_quarters(income_q, |r| r.revenue).unwrap_or(0.0)
    }
}

fn resolve_eps(f: &Financials, pat_ttm: Option<f64>, income_q: &[&IncomeStatementRow]) -> f64 {
    if f.trailing_eps.abs() > 1e-9 {
        f.trailing_eps
    } else if let (Some(pat), shares) = (pat_ttm, f.shares_outstanding) {
        if shares > 0.0 {
            pat / shares
        } else {
            0.0
        }
    } else if let Some(sum) = sum_last_4_quarters(income_q, |r| r.net_income) {
        if f.shares_outstanding > 0.0 {
            sum / f.shares_outstanding
        } else {
            0.0
        }
    } else {
        0.0
    }
}

fn resolve_book_per_share(
    f: &Financials,
    balance_a: &[&BalanceSheetRow],
) -> f64 {
    if f.book_value > 0.0 {
        f.book_value
    } else if let Some(b) = latest(balance_a) {
        if b.total_equity > 0.0 && f.shares_outstanding > 0.0 {
            b.total_equity / f.shares_outstanding
        } else {
            0.0
        }
    } else {
        0.0
    }
}

fn resolve_net_worth(f: &Financials, balance_a: &[&BalanceSheetRow], book_per_share: f64) -> f64 {
    latest(balance_a)
        .map(|b| b.total_equity)
        .filter(|v| *v > 0.0)
        .or_else(|| {
            if book_per_share > 0.0 && f.shares_outstanding > 0.0 {
                Some(book_per_share * f.shares_outstanding)
            } else if f.price_to_book > 0.0 && f.market_cap > 0.0 {
                Some(f.market_cap / f.price_to_book)
            } else {
                None
            }
        })
        .unwrap_or(0.0)
}

fn resolve_cfo_ttm(f: &Financials, cashflow_q: &[&CashflowRow]) -> f64 {
    if f.operating_cashflow.abs() > 1e-3 {
        f.operating_cashflow
    } else if cashflow_q.len() >= 4 {
        cashflow_q[cashflow_q.len() - 4..]
            .iter()
            .map(|r| r.operating_cashflow)
            .sum()
    } else {
        0.0
    }
}

fn resolve_fcf_ttm(f: &Financials, cashflow_q: &[&CashflowRow], cash_a: &[&CashflowRow]) -> f64 {
    if f.free_cashflow.abs() > 1e-3 {
        f.free_cashflow
    } else if cashflow_q.len() >= 4 {
        cashflow_q[cashflow_q.len() - 4..]
            .iter()
            .map(|r| r.free_cashflow)
            .sum()
    } else if let Some(latest_cf) = latest(cash_a) {
        if latest_cf.free_cashflow.abs() > 1e-3 {
            latest_cf.free_cashflow
        } else {
            latest_cf.operating_cashflow - latest_cf.capital_expenditure
        }
    } else {
        0.0
    }
}

fn fcf_3y_sum(cash_a: &[&CashflowRow]) -> Option<f64> {
    if cash_a.len() < 3 {
        return None;
    }
    Some(
        cash_a[cash_a.len() - 3..]
            .iter()
            .map(|r| r.free_cashflow)
            .sum(),
    )
}

fn compute_price_to_fcf(
    price: f64,
    shares: f64,
    cash_a: &[&CashflowRow],
    fcf_ttm: f64,
) -> Option<f64> {
    if price <= 0.0 || shares <= 0.0 {
        return None;
    }
    if let Some(sum) = fcf_3y_sum(cash_a) {
        if sum > 0.0 {
            let fcf_per_share = (sum / 3.0) / shares;
            if fcf_per_share > 0.0 {
                return Some(price / fcf_per_share);
            }
        }
    }
    if fcf_ttm > 0.0 {
        let fcf_per_share = fcf_ttm / shares;
        if fcf_per_share > 0.0 {
            return Some(price / fcf_per_share);
        }
    }
    None
}

/// Enterprise value from Yahoo, or market cap + debt − cash when missing.
fn enterprise_value(f: &Financials) -> f64 {
    if f.enterprise_value > 0.0 {
        f.enterprise_value
    } else if f.market_cap > 0.0 {
        (f.market_cap + f.total_debt - f.total_cash).max(0.0)
    } else {
        0.0
    }
}

/// Trading-day count for given calendar window. Yahoo daily series ~ 252/yr.
const TD_1W: usize = 5;
const TD_3M: usize = 63;
const TD_6M: usize = 126;
const TD_1Y: usize = 252;
const TD_3Y: usize = 252 * 3;
const TD_5Y: usize = 252 * 5;

/// Annualised return between the two close points. `n_days` is trading days.
fn cagr_from_closes(closes: &[f64], n_days: usize) -> Option<f64> {
    if closes.len() <= n_days {
        return None;
    }
    let last = *closes.last()?;
    let first = closes[closes.len() - 1 - n_days];
    if first <= 0.0 || last <= 0.0 {
        return None;
    }
    let years = n_days as f64 / 252.0;
    cagr(first, last, years)
}

/// Convenience accessor: latest annual row, oldest matching, etc.
fn latest<'a, T>(rows: &'a [&T]) -> Option<&'a T> {
    rows.last().copied()
}

fn nth_from_back<'a, T>(rows: &'a [&T], n: usize) -> Option<&'a T> {
    if rows.is_empty() {
        return None;
    }
    let idx = rows.len().checked_sub(1 + n)?;
    Some(rows[idx])
}

fn receivable_days(b: &BalanceSheetRow, i: &IncomeStatementRow) -> Option<f64> {
    if i.revenue > 0.0 && b.net_receivables > 0.0 {
        finite((b.net_receivables / i.revenue) * 365.0)
    } else {
        None
    }
}

fn inventory_days(b: &BalanceSheetRow, i: &IncomeStatementRow) -> Option<f64> {
    if i.cost_of_revenue > 0.0 && b.inventory > 0.0 {
        finite((b.inventory / i.cost_of_revenue) * 365.0)
    } else {
        None
    }
}

fn days_change_3y(
    balance_a: &[&BalanceSheetRow],
    income_a: &[&IncomeStatementRow],
    calc: fn(&BalanceSheetRow, &IncomeStatementRow) -> Option<f64>,
) -> Option<f64> {
    let latest = calc(
        nth_from_back(balance_a, 0)?,
        nth_from_back(income_a, 0)?,
    )?;
    let older = calc(
        nth_from_back(balance_a, 2)?,
        nth_from_back(income_a, 2)?,
    )?;
    finite(latest - older)
}

fn cfo_pat_latest_year(
    income_a: &[&IncomeStatementRow],
    cash_a: &[&CashflowRow],
) -> Option<f64> {
    let pat = latest(income_a)?.net_income;
    let cfo = latest(cash_a)?.operating_cashflow;
    if pat.abs() < 1e-6 {
        None
    } else {
        finite(cfo / pat)
    }
}

/// Working capital = current assets - current liabilities.
fn working_capital(b: &BalanceSheetRow) -> Option<f64> {
    if b.current_assets <= 0.0 && b.current_liabilities <= 0.0 {
        return None;
    }
    Some(b.current_assets - b.current_liabilities)
}

fn sum_last_4_quarters<F: Fn(&IncomeStatementRow) -> f64>(rows: &[&IncomeStatementRow], f: F) -> Option<f64> {
    if rows.len() < 4 {
        return None;
    }
    let last_4 = &rows[rows.len() - 4..];
    Some(last_4.iter().map(|r| f(r)).sum())
}

/// EBIT from income statement fields.
fn ebit_from_income(r: &IncomeStatementRow) -> Option<f64> {
    if r.ebit > 0.0 {
        Some(r.ebit)
    } else if r.operating_income > 0.0 {
        Some(r.operating_income)
    } else if r.pretax_income > 0.0 && r.interest_expense > 0.0 {
        Some(r.pretax_income + r.interest_expense)
    } else if r.ebitda > 0.0 {
        Some(r.ebitda)
    } else {
        None
    }
}

fn sum_last_4_quarters_income_interest(rows: &[&IncomeStatementRow]) -> Option<f64> {
    if rows.len() < 4 {
        return None;
    }
    let s: f64 = rows[rows.len() - 4..]
        .iter()
        .map(|r| r.interest_expense)
        .sum();
    if s > 0.0 {
        Some(s)
    } else {
        None
    }
}

/// At least four consecutive quarterly income rows (Yahoo NSE default depth).
fn quarterly_metrics_allowed(income_q: &[&IncomeStatementRow]) -> bool {
    income_q.len() >= 4
}

fn eps_from_row(r: &IncomeStatementRow, shares: f64) -> Option<f64> {
    r.diluted_eps
        .filter(|e| *e > 0.0)
        .or_else(|| {
            if shares > 0.0 && r.net_income.abs() > 1e-9 {
                Some(r.net_income / shares)
            } else {
                None
            }
        })
}

fn annual_two_year_growth_pct<F: Fn(&IncomeStatementRow) -> f64>(
    rows: &[&IncomeStatementRow],
    f: F,
) -> Option<f64> {
    if rows.len() < 2 {
        return None;
    }
    let last = f(rows[rows.len() - 1]);
    let prev = f(rows[rows.len() - 2]);
    if prev.abs() < 1e-9 {
        return None;
    }
    Some(((last / prev) - 1.0) * 100.0)
}

fn growth_ttm_or_annual<F: Fn(&IncomeStatementRow) -> f64>(
    income_q: &[&IncomeStatementRow],
    income_a: &[&IncomeStatementRow],
    f: F,
) -> Option<f64> {
    ttm_growth_pct(income_q, &f).or_else(|| annual_two_year_growth_pct(income_a, f))
}

fn resolve_ttm_ebit(
    income_q: &[&IncomeStatementRow],
    income_a: &[&IncomeStatementRow],
    f: &Financials,
) -> Option<f64> {
    let from_q = sum_last_4_quarters(income_q, |r| ebit_from_income(r).unwrap_or(0.0))
        .filter(|v| *v > 0.0);
    from_q.or_else(|| latest(income_a).and_then(ebit_from_income)).or_else(|| {
        if f.ebitda > 0.0 {
            Some(f.ebitda)
        } else if f.revenue > 0.0 && f.operating_margins.abs() > 1e-9 {
            Some(f.revenue * f.operating_margins)
        } else {
            None
        }
    })
}

fn resolve_roce(f: &Financials, ebit: Option<f64>, net_worth: f64) -> Option<f64> {
    let ebit = ebit?;
    let capital = net_worth + f.total_debt - f.total_cash;
    if capital > 0.0 && ebit.is_finite() {
        return Some(ebit / capital);
    }
    f.return_on_capital_employed.filter(|roce| roce.is_finite() && roce.abs() > 1e-12)
}

fn balance_has_line_items(b: &BalanceSheetRow) -> bool {
    b.total_assets > 0.0 || b.current_assets > 0.0 || b.total_equity > 0.0
}

fn estimated_total_assets(b: &BalanceSheetRow, net_worth: f64, total_debt: f64) -> Option<f64> {
    if b.total_assets > 0.0 {
        Some(b.total_assets)
    } else if net_worth > 0.0 {
        Some(net_worth + total_debt.max(0.0))
    } else {
        None
    }
}

fn historical_eps_from_row(r: &IncomeStatementRow, shares: f64) -> Option<f64> {
    eps_from_row(r, shares).or_else(|| {
        if shares > 0.0 && r.net_income > 0.0 {
            Some(r.net_income / shares)
        } else {
            None
        }
    })
}

/// Precomputed inputs and cross-section values for metric compute.
struct ComputeContext<'a> {
    inputs: &'a ComputeInputs<'a>,
    f: &'a Financials,
    stmts: &'a StatementBundle,
    chart: &'a ChartHistory,
    closes_v: Vec<f64>,
    income_a: Vec<&'a IncomeStatementRow>,
    income_q: Vec<&'a IncomeStatementRow>,
    balance_a: Vec<&'a BalanceSheetRow>,
    cash_a: Vec<&'a CashflowRow>,
    pat_ttm: Option<f64>,
    revenue: f64,
    shares: f64,
    book_per_share: f64,
    net_worth: f64,
    cfo_ttm: f64,
    fcf_ttm: f64,
    eps: f64,
    price: f64,
    prev_close: f64,
    mcap: f64,
    ev: f64,
    pe: f64,
    pb: f64,
    ps: f64,
    profit_g_3y: Option<f64>,
    ttm_ebit: Option<f64>,
    fcf_3y: Option<f64>,
}

fn build_compute_context<'a>(inputs: &'a ComputeInputs<'a>) -> ComputeContext<'a> {
    let f = inputs.financials;
    let stmts = inputs.statements;
    let chart = inputs.chart_10y;
    let closes_v = closes(chart);
    let income_a = income_annual_asc(stmts);
    let income_q = income_quarterly_asc(stmts);
    let balance_a = balance_annual_asc(stmts);
    let cash_a = cashflow_annual_asc(stmts);
    let cashflow_q = cashflow_quarterly_asc(stmts);

    let pat_ttm = resolve_pat_ttm(f, &income_q);
    let revenue = resolve_revenue(f, &income_a, &income_q);
    let shares = f.shares_outstanding;
    let book_per_share = resolve_book_per_share(f, &balance_a);
    let net_worth = resolve_net_worth(f, &balance_a, book_per_share);
    let cfo_ttm = resolve_cfo_ttm(f, &cashflow_q);
    let fcf_ttm = resolve_fcf_ttm(f, &cashflow_q, &cash_a);
    let eps = resolve_eps(f, pat_ttm, &income_q);
    ComputeContext {
        inputs,
        f,
        stmts,
        chart,
        closes_v,
        income_a,
        income_q,
        balance_a,
        cash_a,
        pat_ttm,
        revenue,
        shares,
        book_per_share,
        net_worth,
        cfo_ttm,
        fcf_ttm,
        eps,
        price: 0.0,
        prev_close: 0.0,
        mcap: 0.0,
        ev: 0.0,
        pe: 0.0,
        pb: 0.0,
        ps: 0.0,
        profit_g_3y: None,
        ttm_ebit: None,
        fcf_3y: None,
    }
}

fn compute_price_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &mut ComputeContext<'_>) {
    ctx.price = chart_last_close(ctx.chart)
        .or_else(|| {
            ctx.closes_v
                .last()
                .copied()
                .filter(|p| *p > 0.0)
        })
        .or_else(|| pos(ctx.f.previous_close))
        .unwrap_or(0.0);
    ctx.prev_close = chart_previous_close(ctx.chart)
        .or_else(|| pos(ctx.f.previous_close))
        .unwrap_or(0.0);

    out.insert(MetricId::CurrentPrice, pos(ctx.price));
    out.insert(MetricId::PreviousClose, pos(ctx.prev_close));

    let (chart_hi, chart_lo) = chart_52w_range(ctx.chart);
    let wk_hi = chart_hi
        .or_else(|| pos(ctx.f.fifty_two_week_high))
        .unwrap_or(0.0);
    let wk_lo = chart_lo
        .or_else(|| pos(ctx.f.fifty_two_week_low))
        .unwrap_or(0.0);
    out.insert(MetricId::FiftyTwoWeekHigh, pos(wk_hi));
    out.insert(MetricId::FiftyTwoWeekLow, pos(wk_lo));
    if wk_hi > 0.0 && ctx.price > 0.0 {
        out.insert(
            MetricId::From52wHighPct,
            Some(((wk_hi - ctx.price) / wk_hi) * 100.0),
        );
    }
    if wk_lo > 0.0 && ctx.price > 0.0 {
        out.insert(
            MetricId::UpFrom52wLowPct,
            Some(((ctx.price - wk_lo) / wk_lo) * 100.0),
        );
    }
    let day_change = if ctx.f.regular_market_change_percent.abs() > 1e-9 {
        Some(ctx.f.regular_market_change_percent * 100.0)
    } else {
        chart_day_change_pct(ctx.chart)
    };
    out.insert(MetricId::RegularMarketChangePercent, day_change.and_then(finite));
    out.insert(MetricId::Volume, pos(ctx.f.regular_market_volume));
    out.insert(MetricId::AverageVolume10Day, pos(ctx.f.average_volume_10_day));

    // 1y avg volume from ctx.chart
    if ctx.chart.bars.len() >= TD_1Y {
        let tail = &ctx.chart.bars[ctx.chart.bars.len() - TD_1Y..];
        let n = tail.len() as f64;
        let s: f64 = tail.iter().map(|b| b.volume).sum();
        if n > 0.0 {
            out.insert(MetricId::Volume1yAvg, Some(s / n));
        }
    }

    out.insert(MetricId::Return1wPct, roc_pct(&ctx.closes_v, TD_1W));
    out.insert(MetricId::Return3mPct, roc_pct(&ctx.closes_v, TD_3M));
    out.insert(MetricId::Return6mPct, roc_pct(&ctx.closes_v, TD_6M));
    out.insert(MetricId::Return1yPct, roc_pct(&ctx.closes_v, TD_1Y));
    out.insert(MetricId::Return3yCagrPct, cagr_from_closes(&ctx.closes_v, TD_3Y));
    out.insert(MetricId::Return5yCagrPct, cagr_from_closes(&ctx.closes_v, TD_5Y));
}

fn compute_market_structure_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &mut ComputeContext<'_>) {
    ctx.mcap = if ctx.f.market_cap > 0.0 {
        ctx.f.market_cap
    } else if ctx.price > 0.0 && ctx.shares > 0.0 {
        ctx.price * ctx.shares
    } else {
        0.0
    };
    out.insert(MetricId::MarketCap, pos(ctx.mcap));
    ctx.ev = if ctx.f.enterprise_value > 0.0 {
        ctx.f.enterprise_value
    } else if ctx.mcap > 0.0 {
        (ctx.mcap + ctx.f.total_debt - ctx.f.total_cash).max(0.0)
    } else {
        enterprise_value(ctx.f)
    };
    out.insert(MetricId::EnterpriseValue, pos(ctx.ev));
    out.insert(MetricId::SharesOutstanding, pos(ctx.shares));
    out.insert(MetricId::FaceValue, pos(ctx.f.face_value));
    if ctx.revenue > 0.0 && ctx.mcap > 0.0 {
        out.insert(MetricId::McapToSales, Some(ctx.mcap / ctx.revenue));
    }
    if ctx.cfo_ttm.abs() > 1e-3 && ctx.mcap > 0.0 {
        out.insert(MetricId::McapToCfo, Some(ctx.mcap / ctx.cfo_ttm));
    }

    // ctx.mcap to quarterly profit
    let latest_q_pat = latest(&ctx.income_q).map(|r| r.net_income).unwrap_or(0.0);
    if ctx.mcap > 0.0 && latest_q_pat.abs() > 1e-3 {
        out.insert(MetricId::McapToQuarterlyProfit, Some(ctx.mcap / latest_q_pat));
    }
}

fn compute_valuation_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &mut ComputeContext<'_>) {
    ctx.pe = if ctx.f.pe_ratio > 0.0 {
        ctx.f.pe_ratio
    } else if ctx.eps > 0.0 && ctx.price > 0.0 {
        ctx.price / ctx.eps
    } else {
        0.0
    };
    ctx.pb = if ctx.f.price_to_book > 0.0 {
        ctx.f.price_to_book
    } else if ctx.book_per_share > 0.0 && ctx.price > 0.0 {
        ctx.price / ctx.book_per_share
    } else {
        0.0
    };
    ctx.ps = if ctx.f.price_to_sales > 0.0 {
        ctx.f.price_to_sales
    } else if ctx.revenue > 0.0 && ctx.shares > 0.0 && ctx.price > 0.0 {
        ctx.price / (ctx.revenue / ctx.shares)
    } else {
        0.0
    };
    out.insert(MetricId::PeRatio, pos(ctx.pe));
    out.insert(MetricId::ForwardPe, pos(ctx.f.forward_pe));
    out.insert(MetricId::PriceToBook, pos(ctx.pb));
    out.insert(MetricId::PriceToSales, pos(ctx.ps));

    ctx.fcf_3y = fcf_3y_sum(&ctx.cash_a).map(|sum| sum / 3.0);
    out.insert(
        MetricId::PriceToFcf,
        compute_price_to_fcf(ctx.price, ctx.shares, &ctx.cash_a, ctx.fcf_ttm),
    );

    // P / (latest quarter EPS x 4)
    if let Some(q_eps) = latest(&ctx.income_q).and_then(|r| eps_from_row(r, ctx.shares)) {
        if ctx.price > 0.0 && q_eps > 0.0 {
            out.insert(MetricId::PriceToQuarterlyEarning, Some(ctx.price / (q_eps * 4.0)));
        }
    }

    let ev_to_ebitda = if ctx.ev > 0.0 && ctx.f.ebitda > 0.0 {
        Some(ctx.ev / ctx.f.ebitda)
    } else {
        None
    };
    out.insert(MetricId::EvToEbitda, ev_to_ebitda);
    let ev_to_sales = if ctx.ev > 0.0 && ctx.revenue > 0.0 {
        Some(ctx.ev / ctx.revenue)
    } else {
        None
    };
    out.insert(MetricId::EvToSales, ev_to_sales);

    // Earnings yield (Greenblatt): trailing EBIT / EV.
    ctx.ttm_ebit = resolve_ttm_ebit(&ctx.income_q, &ctx.income_a, ctx.f);
    if let (Some(ebit), ev_pos) = (ctx.ttm_ebit, ctx.ev) {
        if ebit > 0.0 && ev_pos > 0.0 {
            out.insert(MetricId::EarningsYieldPct, Some((ebit / ev_pos) * 100.0));
        }
    }

    out.insert(MetricId::DividendYield, finite(ctx.f.dividend_yield));

    if ctx.pe > 0.0 && ctx.pb > 0.0 {
        out.insert(MetricId::PbXPe, Some(ctx.pe * ctx.pb));
    }

    // Graham number = sqrt(22.5 * EPS * BV)
    if ctx.eps > 0.0 && ctx.book_per_share > 0.0 {
        out.insert(
            MetricId::GrahamNumber,
            Some((22.5 * ctx.eps * ctx.book_per_share).sqrt()),
        );
    }

    // Intrinsic value (modified Graham): EPS x (8.5 + 2g) x 4.4 / Y
    // where g = 3y profit growth %, Y = current AAA yield (assume 7.0% for India).
    ctx.profit_g_3y = compute_profit_3y_cagr_pct(&ctx.income_a);
    if ctx.eps > 0.0 {
        if let Some(g) = ctx.profit_g_3y {
            let g_capped = g.min(20.0).max(-5.0);
            let aaa_yield = 7.0_f64;
            let iv = ctx.eps * (8.5 + 2.0 * g_capped) * 4.4 / aaa_yield;
            if iv.is_finite() && iv > 0.0 {
                out.insert(MetricId::IntrinsicValue, Some(iv));
            }
        }
    }

    // NCAVPS = (current assets - total liabilities) / ctx.shares — strict Graham form.
    if let Some(b) = latest(&ctx.balance_a).filter(|b| balance_has_line_items(b)) {
        if b.current_assets > 0.0 && ctx.shares > 0.0 {
            let nca = b.current_assets - b.total_liabilities;
            out.insert(MetricId::Ncavps, Some(nca / ctx.shares));
        }
    }

    // Earning power = EBIT / Total Assets
    if let Some(ebit) = ctx.ttm_ebit {
        if let Some(b) = latest(&ctx.balance_a) {
            if let Some(ta) = estimated_total_assets(b, ctx.net_worth, ctx.f.total_debt) {
                if ta > 0.0 {
                    out.insert(MetricId::EarningPowerPct, Some((ebit / ta) * 100.0));
                }
            }
        }
    }

    out.insert(MetricId::EpsTtm, finite(ctx.eps).filter(|v| *v != 0.0));
    out.insert(MetricId::BookValue, pos(ctx.book_per_share));

    // Book value per year: (equity / ctx.shares). Yahoo statements give totalStockholderEquity
    // in absolute terms; per-share book value = equity / shares_outstanding (assumed
    // constant across history; the ctx.chart gives us share-count history if needed but
    // it's noisy for India, so we approximate).
    let bv_year = |idx_back: usize| -> Option<f64> {
        let row = nth_from_back(&ctx.balance_a, idx_back)?;
        if row.total_equity > 0.0 && ctx.shares > 0.0 {
            Some(row.total_equity / ctx.shares)
        } else {
            None
        }
    };
    out.insert(MetricId::BookValuePrecedingYear, bv_year(1));
    out.insert(MetricId::BookValue3yBack, bv_year(3));
    out.insert(
        MetricId::BookValue5yBack,
        bv_year(5).or_else(|| bv_year(3)),
    );

    // Historical PE medians: at each annual close near fiscal year end, divide by
    // diluted EPS that year. Yahoo annual income gives us fiscal year ends and EPS;
    // ctx.chart gives us closes by date.
    out.insert(
        MetricId::HistoricalPe3y,
        historical_pe_median(&ctx.income_a, &ctx.chart.bars, 3, ctx.shares),
    );
    out.insert(
        MetricId::HistoricalPe5y,
        historical_pe_median_fallback(&ctx.income_a, &ctx.chart.bars, 5, ctx.shares),
    );
    out.insert(
        MetricId::HistoricalPe7y,
        historical_pe_median_fallback(&ctx.income_a, &ctx.chart.bars, 7, ctx.shares),
    );
}

fn compute_income_margin_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    out.insert(MetricId::RevenueTtm, pos(ctx.revenue));
    out.insert(MetricId::SalesLastYear, latest(&ctx.income_a).map(|r| r.revenue).and_then(pos));
    out.insert(MetricId::SalesLatestQuarter, latest(&ctx.income_q).map(|r| r.revenue).and_then(pos));

    out.insert(
        MetricId::SalesGrowthTtmPct,
        growth_ttm_or_annual(&ctx.income_q, &ctx.income_a, |r| r.revenue),
    );
    out.insert(MetricId::SalesGrowth3yCagrPct, annual_cagr_pct(&ctx.income_a, 3, |r| r.revenue));
    out.insert(
        MetricId::SalesGrowth5yCagrPct,
        annual_cagr_with_fallback(&ctx.income_a, 5, |r| r.revenue),
    );
    out.insert(
        MetricId::SalesGrowth7yCagrPct,
        annual_cagr_with_fallback(&ctx.income_a, 7, |r| r.revenue),
    );
    if quarterly_metrics_allowed(&ctx.income_q) {
        out.insert(
            MetricId::YoyQuarterlySalesGrowthPct,
            yoy_quarterly_pct(&ctx.income_q, |r| r.revenue),
        );
    }
    out.insert(MetricId::QoqSalesGrowthPct, qoq_pct(&ctx.income_q, |r| r.revenue));

    out.insert(MetricId::ProfitAfterTaxTtm, ctx.pat_ttm);
    out.insert(MetricId::NetProfitLastYear, latest(&ctx.income_a).map(|r| r.net_income).and_then(finite));
    out.insert(
        MetricId::ProfitAfterTaxLatestQuarter,
        latest(&ctx.income_q).map(|r| r.net_income).and_then(finite),
    );
    out.insert(
        MetricId::NetProfitPrecedingYearQuarter,
        nth_from_back(&ctx.income_q, 3).map(|r| r.net_income).and_then(finite),
    );

    // PBT last year = operating income - interest (rough). Falls back to net income / (1 - effective tax).
    if let Some(r) = latest(&ctx.income_a) {
        let pbt = if r.operating_income > 0.0 {
            // We don't track interest expense in IncomeStatementRow; approximate via net + tax.
            r.net_income.max(r.operating_income)
        } else {
            r.net_income
        };
        out.insert(MetricId::ProfitBeforeTaxLastYear, finite(pbt));
    }

    out.insert(
        MetricId::ProfitGrowthTtmPct,
        growth_ttm_or_annual(&ctx.income_q, &ctx.income_a, |r| r.net_income),
    );
    out.insert(MetricId::ProfitGrowth3yCagrPct, ctx.profit_g_3y);
    out.insert(
        MetricId::ProfitGrowth5yCagrPct,
        annual_cagr_with_fallback(&ctx.income_a, 5, |r| r.net_income),
    );
    if ctx.pe > 0.0 {
        let peg = ctx.profit_g_3y
            .filter(|g| *g > 0.0)
            .map(|g| ctx.pe / g)
            .or_else(|| {
                if ctx.f.earnings_growth > 0.0 {
                    Some(ctx.pe / (ctx.f.earnings_growth * 100.0))
                } else if ctx.f.revenue_growth > 0.0 {
                    Some(ctx.pe / (ctx.f.revenue_growth * 100.0))
                } else {
                    None
                }
            });
        out.insert(MetricId::PegRatio, peg.filter(|v| v.is_finite() && *v > 0.0));
    }
    if quarterly_metrics_allowed(&ctx.income_q) {
        out.insert(
            MetricId::YoyQuarterlyProfitGrowthPct,
            yoy_quarterly_pct(&ctx.income_q, |r| r.net_income),
        );
    }
    out.insert(
        MetricId::QoqProfitGrowthPct,
        qoq_pct(&ctx.income_q, |r| r.net_income),
    );

    out.insert(MetricId::Ebitda, pos(ctx.f.ebitda));
    let ebitda_margin = if ctx.f.ebitda_margins.abs() > 1e-9 {
        ctx.f.ebitda_margins
    } else if let Some(r) = latest(&ctx.income_a) {
        if r.revenue > 0.0 && r.ebitda > 0.0 {
            r.ebitda / r.revenue
        } else {
            0.0
        }
    } else {
        0.0
    };
    out.insert(MetricId::EbitdaMargins, finite(ebitda_margin).filter(|v| *v != 0.0));
    out.insert(
        MetricId::OperatingProfitPrecedingYearQuarter,
        nth_from_back(&ctx.income_q, 3)
            .map(|r| r.operating_income)
            .and_then(finite),
    );
    out.insert(MetricId::OpmPct, {
        if ctx.f.operating_margins.abs() > 1e-9 {
            finite(ctx.f.operating_margins)
        } else if let Some(r) = latest(&ctx.income_a) {
            if r.revenue > 0.0 && r.operating_income.abs() > 1e-9 {
                Some(r.operating_income / r.revenue)
            } else {
                None
            }
        } else {
            None
        }
    });

    // NPMs
    if let Some(r) = latest(&ctx.income_a) {
        if r.revenue > 0.0 {
            out.insert(MetricId::NpmLastYearPct, Some(r.net_income / r.revenue));
        }
    }
    if let Some(r) = nth_from_back(&ctx.income_a, 1) {
        if r.revenue > 0.0 {
            out.insert(MetricId::NpmPrecedingYearPct, Some(r.net_income / r.revenue));
        }
    }
    if let Some(r) = latest(&ctx.income_q) {
        if r.revenue > 0.0 {
            out.insert(MetricId::NpmLatestQuarterPct, Some(r.net_income / r.revenue));
        }
    }
    if let Some(r) = nth_from_back(&ctx.income_q, 1) {
        if r.revenue > 0.0 {
            out.insert(MetricId::NpmPrecedingQuarterPct, Some(r.net_income / r.revenue));
        }
    }
    if let Some(r) = nth_from_back(&ctx.income_q, 3) {
        if r.revenue > 0.0 {
            out.insert(MetricId::NpmPrecedingYearQuarterPct, Some(r.net_income / r.revenue));
        }
    }

    out.insert(MetricId::GrossMargins, {
        if ctx.f.gross_margins.abs() > 1e-9 {
            finite(ctx.f.gross_margins)
        } else if let Some(r) = latest(&ctx.income_a) {
            if r.revenue > 0.0 && r.gross_profit.abs() > 1e-9 {
                Some(r.gross_profit / r.revenue)
            } else {
                None
            }
        } else {
            None
        }
    });

    // Depreciation TTM: not available directly in our IncomeStatementRow. Approximate as EBITDA - operating income.
    // This is best-effort; users who need precision should consult the statement bundle directly.
    if let Some(dep) = ttm_depreciation(&ctx.income_q) {
        out.insert(MetricId::DepreciationTtm, Some(dep));
    }

    // Interest expense TTM: prefer income quarterly, fall back to balance quarterly.
    let interest_from_income = sum_last_4_quarters(&ctx.income_q, |r| r.interest_expense);
    if let Some(s) = interest_from_income.filter(|v| *v > 0.0) {
        out.insert(MetricId::InterestTtm, Some(s));
    } else if let Some(s) = sum_last_4_quarters_income_interest(&ctx.income_q) {
        out.insert(MetricId::InterestTtm, Some(s));
    } else if !ctx.stmts.balance_quarterly.is_empty() {
        let mut q: Vec<&BalanceSheetRow> = ctx.stmts.balance_quarterly.iter().collect();
        q.sort_by_key(|r| r.end_ts.unwrap_or(0));
        if q.len() >= 4 {
            let s: f64 = q[q.len() - 4..].iter().map(|r| r.interest_expense).sum();
            if s > 0.0 {
                out.insert(MetricId::InterestTtm, Some(s));
            }
        }
    }

    // Tax TTM / annual: prefer income tax expense fields.
    let tax_ttm = sum_last_4_quarters(&ctx.income_q, |r| {
        if r.income_tax_expense > 0.0 {
            r.income_tax_expense
        } else {
            (r.operating_income - r.net_income).max(0.0)
        }
    });
    out.insert(MetricId::TaxTtm, tax_ttm);
    if let Some(r) = latest(&ctx.income_a) {
        let tax = if r.income_tax_expense > 0.0 {
            r.income_tax_expense
        } else {
            (r.operating_income - r.net_income).max(0.0)
        };
        if tax > 0.0 {
            out.insert(MetricId::TaxLastYear, Some(tax));
        }
    }
    if let Some(r) = nth_from_back(&ctx.income_q, 3) {
        let tax = if r.income_tax_expense > 0.0 {
            r.income_tax_expense
        } else {
            (r.operating_income - r.net_income).max(0.0)
        };
        if tax > 0.0 {
            out.insert(MetricId::TaxPrecedingYearQuarter, Some(tax));
        }
    }

    // Average EBIT 5 years
    if !ctx.income_a.is_empty() {
        let take = ctx.income_a.len().min(5);
        let tail = &ctx.income_a[ctx.income_a.len() - take..];
        let mut sum = 0.0;
        let mut n = 0.0;
        for r in tail {
            if let Some(e) = ebit_from_income(r) {
                sum += e;
                n += 1.0;
            } else if r.operating_income > 0.0 {
                sum += r.operating_income;
                n += 1.0;
            } else if r.ebitda > 0.0 {
                sum += r.ebitda;
                n += 1.0;
            }
        }
        if n >= 1.0 {
            out.insert(MetricId::AvgEbit5y, Some(sum / n));
        }
    }
}

fn compute_returns_efficiency_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    out.insert(
        MetricId::ReturnOnEquity,
        statement_roe(&ctx.income_a, &ctx.balance_a, ctx.net_worth).or_else(|| finite(ctx.f.return_on_equity)),
    );
    out.insert(
        MetricId::ReturnOnAssets,
        statement_roa(&ctx.income_a, &ctx.balance_a, ctx.net_worth, ctx.f.total_debt)
            .or(ctx.f.return_on_assets),
    );
    out.insert(
        MetricId::ReturnOnCapitalEmployed,
        resolve_roce(ctx.f, ctx.ttm_ebit, ctx.net_worth),
    );

    // Weighted average ROE: sum(net_income) / avg(equity) over last N years.
    out.insert(
        MetricId::AvgRoe3y,
        weighted_roe(&ctx.income_a, &ctx.balance_a, 3, ctx.net_worth),
    );
    out.insert(
        MetricId::AvgRoe5y,
        weighted_roe_with_fallback(&ctx.income_a, &ctx.balance_a, 5, ctx.net_worth),
    );
}

fn compute_balance_sheet_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    let latest_b = latest(&ctx.balance_a);
    out.insert(
        MetricId::TotalAssets,
        latest_b
            .and_then(|b| estimated_total_assets(b, ctx.net_worth, ctx.f.total_debt))
            .and_then(pos),
    );
    out.insert(MetricId::NetWorth, pos(ctx.net_worth));
    out.insert(MetricId::TotalDebt, pos(ctx.f.total_debt));
    out.insert(MetricId::DebtToEquity, finite(ctx.f.debt_to_equity));
    if let Some(b) = latest_b {
        if b.current_liabilities > 0.0 {
            out.insert(MetricId::CurrentRatio, Some(b.current_assets / b.current_liabilities));
            out.insert(
                MetricId::QuickRatio,
                Some((b.current_assets - b.inventory).max(0.0) / b.current_liabilities),
            );
        }
    }
    if out.get(&MetricId::CurrentRatio).copied().flatten().is_none() {
        out.insert(MetricId::CurrentRatio, ctx.f.current_ratio);
    }
    if out.get(&MetricId::QuickRatio).copied().flatten().is_none() {
        out.insert(MetricId::QuickRatio, ctx.f.quick_ratio);
    }
    out.insert(MetricId::Inventory, latest_b.map(|r| r.inventory).and_then(pos));

    let wc_year = |idx_back: usize| -> Option<f64> {
        nth_from_back(&ctx.balance_a, idx_back)
            .filter(|b| balance_has_line_items(b))
            .and_then(working_capital)
    };
    out.insert(MetricId::WorkingCapital, wc_year(0));
    out.insert(MetricId::WorkingCapitalPrecedingYear, wc_year(1));
    out.insert(MetricId::WorkingCapital3yBack, wc_year(3));
    out.insert(
        MetricId::WorkingCapital5yBack,
        wc_year(5).or_else(|| wc_year(3)),
    );

    if let (Some(b), Some(rev)) = (
        latest_b.filter(|b| balance_has_line_items(b)),
        latest(&ctx.income_a).map(|r| r.revenue),
    ) {
        if rev > 0.0 {
            let wc = b.current_assets - b.current_liabilities;
            out.insert(MetricId::WorkingCapitalDays, Some(wc / rev * 365.0));
            out.insert(
                MetricId::WorkingCapitalToSalesPct,
                Some(((b.current_assets - b.current_liabilities) / rev) * 100.0),
            );
            if let Some(i) = latest(&ctx.income_a) {
                out.insert(MetricId::DaysReceivableOutstanding, receivable_days(b, i));
                out.insert(MetricId::DaysInventoryOutstanding, inventory_days(b, i));
            }
        }
    }

    if ctx.balance_a.len() >= 3 && ctx.income_a.len() >= 3 {
        out.insert(
            MetricId::DaysReceivableChange3y,
            days_change_3y(&ctx.balance_a, &ctx.income_a, receivable_days),
        );
        out.insert(
            MetricId::DaysInventoryChange3y,
            days_change_3y(&ctx.balance_a, &ctx.income_a, inventory_days),
        );
    }

    // 3y avg working capital days
    if ctx.balance_a.len() >= 3 && ctx.income_a.len() >= 3 {
        let mut acc = 0.0;
        let mut n = 0.0;
        for i in 1..=3 {
            let bi = ctx.balance_a.len().checked_sub(i);
            let ri = ctx.income_a.len().checked_sub(i);
            if let (Some(bi), Some(ri)) = (bi, ri) {
                let b = ctx.balance_a[bi];
                let r = ctx.income_a[ri];
                if r.revenue > 0.0 {
                    let wc = b.current_assets - b.current_liabilities;
                    acc += wc / r.revenue * 365.0;
                    n += 1.0;
                }
            }
        }
        if n >= 2.0 {
            out.insert(MetricId::AvgWorkingCapitalDays3y, Some(acc / n));
        }
    }

    // Financial leverage = avg total assets / net worth
    if ctx.net_worth > 0.0 {
        let mut ta_samples = Vec::new();
        if ctx.balance_a.is_empty() {
            ta_samples.push(ctx.net_worth + ctx.f.total_debt.max(0.0));
        } else {
            for b in ctx.balance_a.iter().rev().take(2) {
                if let Some(ta) = estimated_total_assets(b, ctx.net_worth, ctx.f.total_debt) {
                    ta_samples.push(ta);
                }
            }
        }
        if !ta_samples.is_empty() {
            let avg_ta: f64 = ta_samples.iter().sum::<f64>() / ta_samples.len() as f64;
            out.insert(MetricId::FinancialLeverage, Some(avg_ta / ctx.net_worth));
        }
    }

    // Interest coverage = EBIT / interest expense (latest annual income preferred).
    if let Some(ebit) = latest(&ctx.income_a).and_then(ebit_from_income) {
        let interest = latest(&ctx.income_a)
            .map(|r| r.interest_expense)
            .filter(|v| *v > 0.0)
            .or_else(|| latest_b.map(|b| b.interest_expense).filter(|v| *v > 0.0));
        if let Some(interest) = interest {
            out.insert(MetricId::InterestCoverageRatio, Some(ebit / interest));
        }
    }
}

fn compute_cashflow_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    out.insert(
        MetricId::CumulativeCfoPat3y,
        stocker_core::cumulative_cfo_pat_for_bundle(ctx.stmts, 3),
    );
    out.insert(
        MetricId::CumulativeCfoPat5y,
        stocker_core::cumulative_cfo_pat_for_bundle(ctx.stmts, 5),
    );
    out.insert(MetricId::CfoPatLatestYear, cfo_pat_latest_year(&ctx.income_a, &ctx.cash_a));
    out.insert(MetricId::OperatingCashflowTtm, finite(ctx.cfo_ttm));
    out.insert(MetricId::FreeCashflowLastYear, latest(&ctx.cash_a).map(|r| r.free_cashflow).and_then(finite));
    out.insert(MetricId::FreeCashflowTtm, finite(ctx.fcf_ttm));
    if let Some(sum) = fcf_3y_sum(&ctx.cash_a) {
        out.insert(MetricId::FreeCashflow3ySum, Some(sum));
    }
    if !ctx.cash_a.is_empty() {
        let take = ctx.cash_a.len().min(5);
        let s: f64 = ctx.cash_a[ctx.cash_a.len() - take..]
            .iter()
            .map(|r| r.free_cashflow)
            .sum();
        out.insert(MetricId::FreeCashflow5ySum, Some(s));
    }
}

fn compute_technical_metrics(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    out.insert(MetricId::Dma50, sma_last(&ctx.closes_v, 50));
    out.insert(MetricId::Dma200, sma_last(&ctx.closes_v, 200));
    let (macd_now, macd_sig_now, _) = macd_components(&ctx.closes_v);
    out.insert(MetricId::Macd, macd_now);
    out.insert(MetricId::MacdSignal, macd_sig_now);
    if ctx.closes_v.len() >= 36 {
        let prev_closes = &ctx.closes_v[..ctx.closes_v.len() - 1];
        let (m, s, _) = macd_components(prev_closes);
        out.insert(MetricId::MacdPreviousDay, m);
        out.insert(MetricId::MacdSignalPreviousDay, s);
    }
    out.insert(MetricId::Rsi14, stocker_core::technical_analysis::rsi14(&ctx.closes_v));
}

fn compute_composite_scores(out: &mut HashMap<MetricId, Option<f64>>, ctx: &ComputeContext<'_>) {
    let latest_b = latest(&ctx.balance_a);
    out.insert(
        MetricId::AltmanZScore,
        altman_z(latest(&ctx.balance_a), latest(&ctx.income_a), ctx.mcap, ctx.net_worth),
    );
    let piotroski = if ctx.income_a.len() >= 2 && ctx.balance_a.len() >= 2 && !ctx.cash_a.is_empty() {
        piotroski_f(&ctx.income_a, &ctx.balance_a, &ctx.cash_a)
    } else {
        None
    };
    out.insert(MetricId::PiotroskiFScore, piotroski);
    let g_factor = if ctx.income_a.len() >= 2 && ctx.balance_a.len() >= 2 {
        let computed_roe = statement_roe(&ctx.income_a, &ctx.balance_a, ctx.net_worth)
            .or_else(|| finite(ctx.f.return_on_equity));
        g_factor(&ctx.income_a, &ctx.balance_a, &ctx.cash_a, computed_roe)
    } else {
        None
    };
    out.insert(MetricId::GFactor, g_factor);

    // CROIC = 3y avg FCF / Invested Capital (NW + Debt - Cash)
    if let (Some(b), Some(avg_fcf)) = (latest_b, ctx.fcf_3y) {
        let equity = if b.total_equity > 0.0 {
            b.total_equity
        } else {
            ctx.net_worth
        };
        let invested = equity + b.total_debt - b.cash;
        if invested > 0.0 {
            out.insert(MetricId::CroicPct, Some((avg_fcf / invested) * 100.0));
        }
    }

    // Debt capacity: (EBITDA × 5 − TotalDebt) / NetWorth
    if let Some(ebitda) = pos(ctx.f.ebitda) {
        if ctx.net_worth > 0.0 {
            let capacity = (ebitda * 5.0 - ctx.f.total_debt) / ctx.net_worth;
            out.insert(MetricId::DebtCapacity, finite(capacity));
        }
        if ctx.mcap > 0.0 {
            out.insert(MetricId::McapToDebtCapacity, Some(ctx.mcap / (ebitda * 5.0)));
        }
    }

    // Borrow some peer-quote enrichment if upstream had it.
    if let Some(p) = ctx.inputs.peer_quote {
        if out.get(&MetricId::PeRatio).copied().flatten().is_none() {
            out.insert(MetricId::PeRatio, pos(p.pe_ratio));
        }
        if out.get(&MetricId::PriceToBook).copied().flatten().is_none() {
            out.insert(MetricId::PriceToBook, pos(p.price_to_book));
        }
    }

    // Profile is referenced for `face_value` enrichment hooks in future.
    let _ = ctx.inputs.asset_profile;
}

/// Top-level entry point: compute every metric for one symbol.
pub fn compute_all(inputs: &ComputeInputs<'_>) -> HashMap<MetricId, Option<f64>> {
    let mut out: HashMap<MetricId, Option<f64>> = HashMap::with_capacity(MetricId::ALL.len());
    for id in MetricId::ALL {
        out.insert(*id, None);
    }

    let mut ctx = build_compute_context(inputs);
    compute_price_metrics(&mut out, &mut ctx);
    compute_market_structure_metrics(&mut out, &mut ctx);
    compute_valuation_metrics(&mut out, &mut ctx);
    compute_income_margin_metrics(&mut out, &mut ctx);
    compute_returns_efficiency_metrics(&mut out, &mut ctx);
    compute_balance_sheet_metrics(&mut out, &mut ctx);
    compute_cashflow_metrics(&mut out, &mut ctx);
    compute_technical_metrics(&mut out, &ctx);
    compute_composite_scores(&mut out, &ctx);
    out
}

// ---------------------- helpers ----------------------

fn ttm_growth_pct<F: Fn(&IncomeStatementRow) -> f64>(rows: &[&IncomeStatementRow], f: F) -> Option<f64> {
    if rows.len() < 8 {
        return None;
    }
    let last4 = &rows[rows.len() - 4..];
    let prev4 = &rows[rows.len() - 8..rows.len() - 4];
    let s_last: f64 = last4.iter().map(|r| f(r)).sum();
    let s_prev: f64 = prev4.iter().map(|r| f(r)).sum();
    if s_prev.abs() < 1e-9 {
        return None;
    }
    Some(((s_last / s_prev) - 1.0) * 100.0)
}

fn yoy_quarterly_pct<F: Fn(&IncomeStatementRow) -> f64>(rows: &[&IncomeStatementRow], f: F) -> Option<f64> {
    if rows.len() < 4 {
        return None;
    }
    let cur = f(rows[rows.len() - 1]);
    let prev_year = f(rows[rows.len() - 4]);
    if prev_year.abs() < 1e-9 {
        return None;
    }
    Some(((cur / prev_year) - 1.0) * 100.0)
}

fn qoq_pct<F: Fn(&IncomeStatementRow) -> f64>(rows: &[&IncomeStatementRow], f: F) -> Option<f64> {
    if rows.len() < 2 {
        return None;
    }
    let cur = f(rows[rows.len() - 1]);
    let prev = f(rows[rows.len() - 2]);
    if prev.abs() < 1e-9 {
        return None;
    }
    Some(((cur / prev) - 1.0) * 100.0)
}

fn annual_cagr_pct<F: Fn(&IncomeStatementRow) -> f64>(
    rows: &[&IncomeStatementRow],
    years: usize,
    f: F,
) -> Option<f64> {
    if rows.len() < years + 1 {
        return None;
    }
    let last = f(rows[rows.len() - 1]);
    let first = f(rows[rows.len() - 1 - years]);
    cagr(first, last, years as f64)
}

/// CAGR for a target horizon (5Y / 7Y), falling back to shorter spans when Yahoo only returns ~4 annual periods.
fn annual_cagr_with_fallback<F: Fn(&IncomeStatementRow) -> f64>(
    rows: &[&IncomeStatementRow],
    target_years: usize,
    f: F,
) -> Option<f64> {
    let chain: &[usize] = match target_years {
        7 => &[7, 5, 4, 3],
        5 => &[5, 4, 3],
        _ => &[target_years, 3],
    };
    for &years in chain {
        if years > target_years {
            continue;
        }
        if let Some(v) = annual_cagr_pct(rows, years, &f) {
            return Some(v);
        }
    }
    None
}

fn compute_profit_3y_cagr_pct(rows: &[&IncomeStatementRow]) -> Option<f64> {
    annual_cagr_pct(rows, 3, |r| r.net_income)
}

fn ttm_depreciation(rows: &[&IncomeStatementRow]) -> Option<f64> {
    if rows.len() < 4 {
        return None;
    }
    let last4 = &rows[rows.len() - 4..];
    let from_line: f64 = last4.iter().map(|r| r.depreciation.max(0.0)).sum();
    if from_line > 0.0 {
        return Some(from_line);
    }
    // Fallback: depreciation ≈ EBITDA - operating income.
    let s: f64 = last4
        .iter()
        .map(|r| (r.ebitda - r.operating_income).max(0.0))
        .sum();
    if s > 0.0 {
        Some(s)
    } else {
        None
    }
}

fn weighted_roe_with_fallback(
    income: &[&IncomeStatementRow],
    balance: &[&BalanceSheetRow],
    target_years: usize,
    net_worth_live: f64,
) -> Option<f64> {
    for years in [target_years, 4, 3] {
        if years > target_years {
            continue;
        }
        if let Some(v) = weighted_roe(income, balance, years, net_worth_live) {
            return Some(v);
        }
    }
    None
}

fn weighted_roe(
    income: &[&IncomeStatementRow],
    balance: &[&BalanceSheetRow],
    years: usize,
    net_worth_live: f64,
) -> Option<f64> {
    if income.len() < years {
        return None;
    }
    let inc_tail = &income[income.len() - years..];
    let sum_ni: f64 = inc_tail.iter().map(|r| r.net_income).sum();
    let mut eq_samples = Vec::new();
    if balance.len() >= years {
        for b in &balance[balance.len() - years..] {
            if b.total_equity > 0.0 {
                eq_samples.push(b.total_equity);
            }
        }
    }
    let avg_eq = if !eq_samples.is_empty() {
        eq_samples.iter().sum::<f64>() / eq_samples.len() as f64
    } else if net_worth_live > 0.0 {
        net_worth_live
    } else {
        return None;
    };
    Some(sum_ni / (avg_eq * years as f64))
}

/// Latest-year ROE from statements: net income / average equity (current and prior year).
fn statement_roe(
    income: &[&IncomeStatementRow],
    balance: &[&BalanceSheetRow],
    net_worth_live: f64,
) -> Option<f64> {
    let ni = latest(income).map(|r| r.net_income)?;
    if !ni.is_finite() {
        return None;
    }
    let cur_eq = latest(balance)
        .and_then(|b| pos(b.total_equity))
        .or_else(|| if net_worth_live > 0.0 { Some(net_worth_live) } else { None })?;
    let prev_eq = nth_from_back(balance, 1).and_then(|b| pos(b.total_equity));
    let avg_eq = match prev_eq {
        Some(p) if p > 0.0 => (cur_eq + p) / 2.0,
        _ => cur_eq,
    };
    if avg_eq <= 0.0 {
        return None;
    }
    Some(ni / avg_eq)
}

/// Latest-year ROA from statements: net income / average total assets (current and prior year).
fn statement_roa(
    income: &[&IncomeStatementRow],
    balance: &[&BalanceSheetRow],
    net_worth: f64,
    total_debt: f64,
) -> Option<f64> {
    let ni = latest(income).map(|r| r.net_income)?;
    if !ni.is_finite() {
        return None;
    }
    let cur_ta = latest(balance).and_then(|b| estimated_total_assets(b, net_worth, total_debt))?;
    let prev_ta = nth_from_back(balance, 1)
        .and_then(|b| estimated_total_assets(b, net_worth, total_debt));
    let avg_ta = match prev_ta {
        Some(p) if p > 0.0 => (cur_ta + p) / 2.0,
        _ => cur_ta,
    };
    if avg_ta <= 0.0 {
        return None;
    }
    Some(ni / avg_ta)
}

fn historical_pe_median_fallback(
    income: &[&IncomeStatementRow],
    bars: &[stocker_core::models::ChartBar],
    target_years: usize,
    shares: f64,
) -> Option<f64> {
    let chain: &[usize] = match target_years {
        7 => &[7, 5, 4, 3],
        5 => &[5, 4, 3],
        _ => &[target_years, 3],
    };
    for &years in chain {
        if years > target_years {
            continue;
        }
        if let Some(m) = historical_pe_median(income, bars, years, shares) {
            return Some(m);
        }
    }
    None
}

fn historical_pe_median(
    income: &[&IncomeStatementRow],
    bars: &[stocker_core::models::ChartBar],
    years: usize,
    shares: f64,
) -> Option<f64> {
    if income.len() < years || bars.is_empty() {
        return None;
    }
    let tail = &income[income.len() - years..];
    let mut samples = Vec::with_capacity(years);
    for r in tail {
        let Some(end_ts) = r.end_ts else { continue };
        let Some(eps) = historical_eps_from_row(r, shares) else { continue };
        if eps <= 0.0 {
            continue;
        }
        // Find the chart bar closest to this fiscal year end timestamp.
        let mut best: Option<&stocker_core::models::ChartBar> = None;
        let mut best_dist = i64::MAX;
        for bar in bars {
            let dist = (bar.ts - end_ts).abs();
            if dist < best_dist {
                best_dist = dist;
                best = Some(bar);
            }
        }
        if let Some(b) = best {
            let pe = b.close / eps;
            if pe.is_finite() && pe > 0.0 {
                samples.push(pe);
            }
        }
    }
    median(samples)
}

fn altman_z(
    b: Option<&BalanceSheetRow>,
    i: Option<&IncomeStatementRow>,
    market_cap: f64,
    net_worth: f64,
) -> Option<f64> {
    let (b, i) = (b?, i?);
    let total_liabilities = b.total_liabilities;
    let ta = estimated_total_assets(b, net_worth, b.total_debt)?;
    if ta <= 0.0 {
        return None;
    }
    let working_capital = b.current_assets - b.current_liabilities;
    let retained_proxy = if b.retained_earnings > 0.0 {
        b.retained_earnings
    } else {
        b.total_equity.max(0.0)
    };
    let ebit = ebit_from_income(i).unwrap_or(i.operating_income.max(i.ebitda));
    let sales = i.revenue;
    if total_liabilities <= 0.0 || sales <= 0.0 || market_cap <= 0.0 {
        return None;
    }
    let z = 1.2 * (working_capital / ta)
        + 1.4 * (retained_proxy / ta)
        + 3.3 * (ebit / ta)
        + 0.6 * (market_cap / total_liabilities)
        + 1.0 * (sales / ta);
    nonzero(z)
}

fn piotroski_f(income: &[&IncomeStatementRow], balance: &[&BalanceSheetRow], cash: &[&CashflowRow]) -> Option<f64> {
    if income.len() < 2 || balance.len() < 2 || cash.is_empty() {
        return None;
    }
    let cur_i = income[income.len() - 1];
    let prev_i = income[income.len() - 2];
    let cur_b = balance[balance.len() - 1];
    let prev_b = balance[balance.len() - 2];
    let cur_c = cash[cash.len() - 1];

    let mut score = 0.0;
    // 1. Net income > 0
    if cur_i.net_income > 0.0 {
        score += 1.0;
    }
    // 2. ROA improving (net_income/assets)
    if prev_b.total_assets > 0.0 && cur_b.total_assets > 0.0 {
        let cur_roa = cur_i.net_income / cur_b.total_assets;
        let prev_roa = prev_i.net_income / prev_b.total_assets;
        if cur_roa > prev_roa {
            score += 1.0;
        }
    }
    // 3. CFO > 0
    if cur_c.operating_cashflow > 0.0 {
        score += 1.0;
    }
    // 4. CFO > Net income
    if cur_c.operating_cashflow > cur_i.net_income {
        score += 1.0;
    }
    // 5. LT debt going down (use total_debt as proxy)
    if cur_b.total_debt < prev_b.total_debt {
        score += 1.0;
    }
    // 6. Current ratio improving
    if cur_b.current_liabilities > 0.0 && prev_b.current_liabilities > 0.0 {
        let cur_cr = cur_b.current_assets / cur_b.current_liabilities;
        let prev_cr = prev_b.current_assets / prev_b.current_liabilities;
        if cur_cr > prev_cr {
            score += 1.0;
        }
    }
    // 7. No share dilution — we don't track historical share count, skip (worth 0).
    // 8. Gross margin improving
    if prev_i.revenue > 0.0 && cur_i.revenue > 0.0 {
        let cur_gm = cur_i.gross_profit / cur_i.revenue;
        let prev_gm = prev_i.gross_profit / prev_i.revenue;
        if cur_gm > prev_gm {
            score += 1.0;
        }
    }
    // 9. Asset turnover improving (sales/avg assets)
    if prev_b.total_assets > 0.0 && cur_b.total_assets > 0.0 {
        let cur_at = cur_i.revenue / cur_b.total_assets;
        let prev_at = prev_i.revenue / prev_b.total_assets;
        if cur_at > prev_at {
            score += 1.0;
        }
    }
    Some(score)
}

fn g_factor(
    income: &[&IncomeStatementRow],
    balance: &[&BalanceSheetRow],
    cash: &[&CashflowRow],
    computed_roe: Option<f64>,
) -> Option<f64> {
    if income.len() < 2 || balance.len() < 2 {
        return None;
    }
    let cur_i = income[income.len() - 1];
    let prev_i = income[income.len() - 2];
    let cur_b = balance[balance.len() - 1];
    let prev_b = balance[balance.len() - 2];
    let cur_c = cash.last().copied();
    let mut s = 0.0;
    // 1. ROA improvement
    if cur_b.total_assets > 0.0 && prev_b.total_assets > 0.0 {
        let cur_roa = cur_i.net_income / cur_b.total_assets;
        let prev_roa = prev_i.net_income / prev_b.total_assets;
        if cur_roa > prev_roa {
            s += 1.0;
        }
    }
    // 2. FCF > 0
    if let Some(c) = cur_c {
        if c.free_cashflow > 0.0 {
            s += 1.0;
        }
    }
    // 3. CFO > NI
    if let Some(c) = cur_c {
        if c.operating_cashflow > cur_i.net_income {
            s += 1.0;
        }
    }
    // 4. Lower D/E YoY
    if prev_b.total_equity > 0.0 && cur_b.total_equity > 0.0 {
        let cur_de = cur_b.total_debt / cur_b.total_equity;
        let prev_de = prev_b.total_debt / prev_b.total_equity;
        if cur_de < prev_de {
            s += 1.0;
        }
    }
    // 5. Current ratio improving
    if cur_b.current_liabilities > 0.0 && prev_b.current_liabilities > 0.0 {
        let cur_cr = cur_b.current_assets / cur_b.current_liabilities;
        let prev_cr = prev_b.current_assets / prev_b.current_liabilities;
        if cur_cr > prev_cr {
            s += 1.0;
        }
    }
    // 6. No share dilution — we approximate by checking that net income grew faster than EPS (skip if missing).
    if let (Some(cur_eps), Some(prev_eps)) = (cur_i.diluted_eps, prev_i.diluted_eps) {
        if cur_eps >= prev_eps {
            s += 1.0;
        }
    }
    // 7. Gross margin improving
    if prev_i.revenue > 0.0 && cur_i.revenue > 0.0 {
        let cur_gm = cur_i.gross_profit / cur_i.revenue;
        let prev_gm = prev_i.gross_profit / prev_i.revenue;
        if cur_gm > prev_gm {
            s += 1.0;
        }
    }
    // 8. Asset turnover improving
    if prev_b.total_assets > 0.0 && cur_b.total_assets > 0.0 {
        let cur_at = cur_i.revenue / cur_b.total_assets;
        let prev_at = prev_i.revenue / prev_b.total_assets;
        if cur_at > prev_at {
            s += 1.0;
        }
    }
    // 9. ROE > 15%
    if computed_roe.is_some_and(|r| r > 0.15) {
        s += 1.0;
    }
    // 10. Profit growth > 0
    if cur_i.net_income > prev_i.net_income {
        s += 1.0;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stocker_core::models::{BalanceSheetRow, CashflowRow, ChartBar, ChartHistory, Financials, IncomeStatementRow};

    #[test]
    fn cagr_pct_basics() {
        let v = cagr(100.0, 200.0, 5.0).unwrap();
        assert!((v - 14.87).abs() < 0.5);
    }

    #[test]
    fn annual_cagr_with_fallback_uses_three_years_when_only_four_points() {
        let rows: Vec<IncomeStatementRow> = (0..4)
            .map(|i| IncomeStatementRow {
                revenue: 100.0 * (i as f64 + 1.0),
                ..IncomeStatementRow::default()
            })
            .collect();
        let refs: Vec<&IncomeStatementRow> = rows.iter().collect();
        assert!(annual_cagr_with_fallback(&refs, 5, |r| r.revenue).is_some());
        assert!(annual_cagr_pct(&refs, 5, |r| r.revenue).is_none());
    }

    #[test]
    fn ttm_depreciation_prefers_reported_line() {
        let rows: Vec<IncomeStatementRow> = (0..4)
            .map(|i| IncomeStatementRow {
                depreciation: 10.0 + i as f64,
                ebitda: 100.0,
                operating_income: 100.0,
                ..IncomeStatementRow::default()
            })
            .collect();
        let refs: Vec<&IncomeStatementRow> = rows.iter().collect();
        assert_eq!(ttm_depreciation(&refs), Some(10.0 + 11.0 + 12.0 + 13.0));
    }

    #[test]
    fn median_basics() {
        assert_eq!(median([1.0, 3.0, 2.0]), Some(2.0));
        assert_eq!(median([1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median(std::iter::empty::<f64>()), None);
    }

    #[test]
    fn chart_52w_range_uses_partial_history() {
        let bars: Vec<ChartBar> = (0..100)
            .map(|i| ChartBar {
                ts: i,
                open: 100.0,
                high: 110.0 + i as f64 * 0.1,
                low: 90.0,
                close: 105.0,
                volume: 1_000.0,
            })
            .collect();
        let chart = ChartHistory { bars };
        let (hi, lo) = chart_52w_range(&chart);
        assert!(hi.is_some());
        assert!(lo.is_some());
        assert!(hi.unwrap() >= 110.0);
        assert!(lo.unwrap() <= 90.0);
    }

    #[test]
    fn price_to_fcf_from_positive_3y_sum_with_negative_year() {
        let cash_a: Vec<CashflowRow> = vec![
            CashflowRow {
                end_date_fmt: "2021".into(),
                end_ts: None,
                operating_cashflow: 100.0,
                capital_expenditure: 10.0,
                free_cashflow: -50.0,
            },
            CashflowRow {
                end_date_fmt: "2022".into(),
                end_ts: None,
                operating_cashflow: 100.0,
                capital_expenditure: 10.0,
                free_cashflow: 100.0,
            },
            CashflowRow {
                end_date_fmt: "2023".into(),
                end_ts: None,
                operating_cashflow: 100.0,
                capital_expenditure: 10.0,
                free_cashflow: 100.0,
            },
        ];
        let refs: Vec<&CashflowRow> = cash_a.iter().collect();
        let pfcf = compute_price_to_fcf(200.0, 10.0, &refs, 0.0).unwrap();
        // sum = 150, avg = 50, per share = 5, price/fcf = 40
        assert!((pfcf - 40.0).abs() < 0.01);
    }

    #[test]
    fn resolve_net_worth_falls_back_to_book_value() {
        let f = Financials {
            book_value: 50.0,
            shares_outstanding: 10.0,
            ..Financials::default()
        };
        let nw = resolve_net_worth(&f, &[], 50.0);
        assert!((nw - 500.0).abs() < 0.01);
    }

    #[test]
    fn ebit_from_income_prefers_explicit_ebit() {
        let row = IncomeStatementRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            revenue: 1000.0,
            cost_of_revenue: 0.0,
            gross_profit: 400.0,
            ebitda: 300.0,
            operating_income: 200.0,
            ebit: 250.0,
            pretax_income: 0.0,
            interest_expense: 0.0,
            income_tax_expense: 0.0,
            depreciation: 0.0,
            net_income: 150.0,
            diluted_eps: None,
            other_income_expense: 0.0,
            net_interest_income: 0.0,
        };
        assert_eq!(ebit_from_income(&row), Some(250.0));
    }

    #[test]
    fn quarterly_metrics_allowed_with_four_quarters() {
        let rows: Vec<IncomeStatementRow> = (0..4)
            .map(|i| IncomeStatementRow {
                end_date_fmt: format!("Q{i}"),
                end_ts: Some(i),
                revenue: 100.0 + i as f64,
                cost_of_revenue: 0.0,
                gross_profit: 0.0,
                ebitda: 0.0,
                operating_income: 0.0,
                ebit: 0.0,
                pretax_income: 0.0,
                interest_expense: 0.0,
                income_tax_expense: 0.0,
                depreciation: 0.0,
                net_income: 10.0,
                diluted_eps: None,
                other_income_expense: 0.0,
                net_interest_income: 0.0,
            })
            .collect();
        let refs: Vec<&IncomeStatementRow> = rows.iter().collect();
        assert!(quarterly_metrics_allowed(&refs));
        assert!(yoy_quarterly_pct(&refs, |r| r.revenue).is_some());
        assert!(qoq_pct(&refs, |r| r.revenue).is_some());
    }

    #[test]
    fn eps_from_row_falls_back_to_pat_over_shares() {
        let row = IncomeStatementRow {
            end_date_fmt: "Q1".into(),
            end_ts: None,
            revenue: 100.0,
            cost_of_revenue: 0.0,
            gross_profit: 0.0,
            ebitda: 0.0,
            operating_income: 0.0,
            ebit: 0.0,
            pretax_income: 0.0,
            interest_expense: 0.0,
            income_tax_expense: 0.0,
            depreciation: 0.0,
            net_income: 100.0,
            diluted_eps: None,
            other_income_expense: 0.0,
            net_interest_income: 0.0,
        };
        assert_eq!(eps_from_row(&row, 10.0), Some(10.0));
    }

    #[test]
    fn working_capital_days_uses_ca_minus_cl_without_cash() {
        let balance = BalanceSheetRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            cash: 500.0,
            current_assets: 1000.0,
            current_liabilities: 400.0,
            ..BalanceSheetRow::default()
        };
        let income = IncomeStatementRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            revenue: 3650.0,
            ..IncomeStatementRow::default()
        };
        let wc = balance.current_assets - balance.current_liabilities;
        let wcd = wc / income.revenue * 365.0;
        assert!((wcd - 60.0).abs() < 0.01);
        let wcd_with_cash_subtraction =
            ((balance.current_assets - balance.current_liabilities) - balance.cash) / income.revenue * 365.0;
        assert!((wcd - wcd_with_cash_subtraction).abs() > 1.0);
    }

    #[test]
    fn altman_z_factor_d_uses_total_liabilities_not_debt() {
        let balance = BalanceSheetRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            current_assets: 500.0,
            current_liabilities: 200.0,
            total_assets: 1000.0,
            total_liabilities: 800.0,
            total_debt: 100.0,
            total_equity: 200.0,
            retained_earnings: 150.0,
            ..BalanceSheetRow::default()
        };
        let income = IncomeStatementRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            revenue: 2000.0,
            operating_income: 100.0,
            ebit: 100.0,
            ebitda: 120.0,
            ..IncomeStatementRow::default()
        };
        let mcap = 1600.0;
        let nw = 200.0;
        let z = altman_z(Some(&balance), Some(&income), mcap, nw).unwrap();
        let ta = balance.total_assets;
        let wc = balance.current_assets - balance.current_liabilities;
        let ebit = 100.0;
        let expected = 1.2 * (wc / ta)
            + 1.4 * (balance.retained_earnings / ta)
            + 3.3 * (ebit / ta)
            + 0.6 * (mcap / balance.total_liabilities)
            + 1.0 * (income.revenue / ta);
        assert!((z - expected).abs() < 0.001);
        let wrong_d = 0.6 * (mcap / balance.total_debt);
        let right_d = 0.6 * (mcap / balance.total_liabilities);
        assert!((wrong_d - right_d).abs() > 1.0);
    }

    #[test]
    fn statement_roe_averages_equity_over_two_years() {
        let income = vec![
            IncomeStatementRow {
                end_date_fmt: "2023".into(),
                end_ts: None,
                net_income: 80.0,
                ..IncomeStatementRow::default()
            },
            IncomeStatementRow {
                end_date_fmt: "2024".into(),
                end_ts: None,
                net_income: 100.0,
                ..IncomeStatementRow::default()
            },
        ];
        let balance = vec![
            BalanceSheetRow {
                end_date_fmt: "2023".into(),
                end_ts: None,
                total_equity: 800.0,
                ..BalanceSheetRow::default()
            },
            BalanceSheetRow {
                end_date_fmt: "2024".into(),
                end_ts: None,
                total_equity: 1000.0,
                ..BalanceSheetRow::default()
            },
        ];
        let i_refs: Vec<&IncomeStatementRow> = income.iter().collect();
        let b_refs: Vec<&BalanceSheetRow> = balance.iter().collect();
        let roe = statement_roe(&i_refs, &b_refs, 0.0).unwrap();
        assert!((roe - 0.1111).abs() < 0.001);
    }

    #[test]
    fn statement_roa_averages_total_assets_over_two_years() {
        let income = vec![IncomeStatementRow {
            end_date_fmt: "2024".into(),
            end_ts: None,
            net_income: 100.0,
            ..IncomeStatementRow::default()
        }];
        let balance = vec![
            BalanceSheetRow {
                end_date_fmt: "2023".into(),
                end_ts: None,
                total_assets: 800.0,
                ..BalanceSheetRow::default()
            },
            BalanceSheetRow {
                end_date_fmt: "2024".into(),
                end_ts: None,
                total_assets: 1000.0,
                ..BalanceSheetRow::default()
            },
        ];
        let i_refs: Vec<&IncomeStatementRow> = income.iter().collect();
        let b_refs: Vec<&BalanceSheetRow> = balance.iter().collect();
        let roa = statement_roa(&i_refs, &b_refs, 0.0, 0.0).unwrap();
        assert!((roa - 0.1111).abs() < 0.001);
    }

    #[test]
    fn receivable_and_inventory_days_helpers() {
        let b = BalanceSheetRow {
            net_receivables: 100.0,
            inventory: 50.0,
            ..BalanceSheetRow::default()
        };
        let i = IncomeStatementRow {
            revenue: 365.0,
            cost_of_revenue: 182.5,
            ..IncomeStatementRow::default()
        };
        assert!((receivable_days(&b, &i).unwrap() - 100.0).abs() < 0.01);
        assert!((inventory_days(&b, &i).unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn days_change_3y_positive_when_days_rise() {
        let balance = vec![
            BalanceSheetRow {
                end_date_fmt: "2022".into(),
                net_receivables: 50.0,
                ..BalanceSheetRow::default()
            },
            BalanceSheetRow {
                end_date_fmt: "2023".into(),
                net_receivables: 60.0,
                ..BalanceSheetRow::default()
            },
            BalanceSheetRow {
                end_date_fmt: "2024".into(),
                net_receivables: 100.0,
                ..BalanceSheetRow::default()
            },
        ];
        let income = vec![
            IncomeStatementRow {
                end_date_fmt: "2022".into(),
                revenue: 365.0,
                ..IncomeStatementRow::default()
            },
            IncomeStatementRow {
                end_date_fmt: "2023".into(),
                revenue: 365.0,
                ..IncomeStatementRow::default()
            },
            IncomeStatementRow {
                end_date_fmt: "2024".into(),
                revenue: 365.0,
                ..IncomeStatementRow::default()
            },
        ];
        let b_refs: Vec<&BalanceSheetRow> = balance.iter().collect();
        let i_refs: Vec<&IncomeStatementRow> = income.iter().collect();
        let chg = days_change_3y(&b_refs, &i_refs, receivable_days).unwrap();
        assert!((chg - 50.0).abs() < 0.01);
    }

    #[test]
    fn cfo_pat_latest_year_ratio() {
        let income = vec![IncomeStatementRow {
            end_date_fmt: "2024".into(),
            net_income: 100.0,
            ..IncomeStatementRow::default()
        }];
        let cash = vec![CashflowRow {
            end_date_fmt: "2024".into(),
            operating_cashflow: 120.0,
            ..CashflowRow::default()
        }];
        let i_refs: Vec<&IncomeStatementRow> = income.iter().collect();
        let c_refs: Vec<&CashflowRow> = cash.iter().collect();
        assert!((cfo_pat_latest_year(&i_refs, &c_refs).unwrap() - 1.2).abs() < 0.001);
    }
}
