use crate::math::{cagr, pct_change};
use crate::models::{
    ActionGuidance, AssetProfile, AuditChecklistItem, AuditStatus, BalanceSheetRow, BankingMetrics,
    CashflowRow, DataCoverage, FinancialCompanyType, FinancialStrengthAudit, Financials, IncomeStatementRow,
    MarketSignals, ResearchRating, ScreenerMetricSnapshot, StatementBundle, ValuationAnalysis,
};
use crate::financial_company::{NA_FILING, NA_NOT_USED};
use crate::statements::sort_owned_desc;

pub fn cumulative_cfo_pat_for_bundle(bundle: &StatementBundle, years: usize) -> Option<f64> {
    let mut inc = bundle.income_annual.clone();
    let mut cf = bundle.cashflow_annual.clone();
    inc.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    cf.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    if inc.len() < years || cf.len() < years {
        return None;
    }
    let pat: f64 = inc.iter().take(years).map(|r| r.net_income).sum();
    let cfo: f64 = cf.iter().take(years).map(|r| r.operating_cashflow).sum();
    if pat.abs() < 1e-6 {
        return None;
    }
    Some(cfo / pat)
}

fn fmt_ratio(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.2}x", x),
        _ => "N/A".to_string(),
    }
}

fn fmt_pct(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.1}%", x),
        _ => "N/A".to_string(),
    }
}

fn fmt_days(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.0} days", x),
        _ => "N/A".to_string(),
    }
}

fn status_from_ratio_ge(v: Option<f64>, threshold: f64) -> AuditStatus {
    match v {
        Some(x) if x >= threshold => AuditStatus::Pass,
        Some(x) if x >= threshold * 0.8 => AuditStatus::Watch,
        Some(_) => AuditStatus::Fail,
        None => AuditStatus::InsufficientData,
    }
}

fn status_from_ratio_le(v: Option<f64>, healthy: f64, warn: f64) -> AuditStatus {
    match v {
        Some(x) if x <= healthy => AuditStatus::Pass,
        Some(x) if x <= warn => AuditStatus::Watch,
        Some(_) => AuditStatus::Fail,
        None => AuditStatus::InsufficientData,
    }
}

fn status_from_coverage(v: Option<f64>) -> AuditStatus {
    match v {
        Some(x) if x >= 4.0 => AuditStatus::Pass,
        Some(x) if x >= 2.0 => AuditStatus::Watch,
        Some(x) if x > 0.0 => AuditStatus::Fail,
        _ => AuditStatus::InsufficientData,
    }
}

fn score_from_checklist_opt(items: &[AuditChecklistItem]) -> Option<f64> {
    let scored: Vec<f64> = items
        .iter()
        .filter(|i| i.status != AuditStatus::InsufficientData)
        .map(|i| match i.status {
            AuditStatus::Pass => 100.0,
            AuditStatus::Watch => 55.0,
            AuditStatus::Fail => 15.0,
            AuditStatus::InsufficientData => 50.0,
        })
        .collect();
    if scored.is_empty() {
        None
    } else {
        Some(scored.iter().sum::<f64>() / scored.len() as f64)
    }
}

fn is_asset_light(profile: &AssetProfile) -> bool {
    let sector = profile.sector.as_deref().unwrap_or("").to_lowercase();
    let industry = profile.industry.as_deref().unwrap_or("").to_lowercase();
    sector.contains("technology")
        || sector.contains("communication")
        || industry.contains("software")
        || industry.contains("internet")
        || industry.contains("it services")
}

fn score_from_checklist(items: &[AuditChecklistItem]) -> f64 {
    score_from_checklist_opt(items).unwrap_or(50.0)
}

fn status_from_pct_le(v: Option<f64>, pass: f64, watch: f64) -> AuditStatus {
    match v {
        Some(x) if x <= pass => AuditStatus::Pass,
        Some(x) if x <= watch => AuditStatus::Watch,
        Some(_) => AuditStatus::Fail,
        None => AuditStatus::InsufficientData,
    }
}

fn status_from_pct_ge(v: Option<f64>, pass: f64, watch: f64) -> AuditStatus {
    match v {
        Some(x) if x >= pass => AuditStatus::Pass,
        Some(x) if x >= watch => AuditStatus::Watch,
        Some(_) => AuditStatus::Fail,
        None => AuditStatus::InsufficientData,
    }
}

struct AuditAccumulator {
    checklist: Vec<AuditChecklistItem>,
    red_flags: Vec<String>,
    strengths: Vec<String>,
}

impl AuditAccumulator {
    fn new() -> Self {
        Self {
            checklist: Vec::new(),
            red_flags: Vec::new(),
            strengths: Vec::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn audit_cashflow_checks(
    acc: &mut AuditAccumulator,
    bundle: &StatementBundle,
    inc: &[IncomeStatementRow],
    _bal: &[BalanceSheetRow],
    cf: &[CashflowRow],
    _financials: &Financials,
    _profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
) {
    let cfo_pat_3y = cumulative_cfo_pat_for_bundle(bundle, 3)
        .or_else(|| screener.and_then(|s| s.cumulative_cfo_pat_3y))
        .or_else(|| {
            screener.and_then(|s| {
                let cfo = s.operating_cashflow_ttm?;
                let pat = s.profit_after_tax_ttm?;
                if pat.abs() > 1e-6 {
                    Some(cfo / pat)
                } else {
                    None
                }
            })
        });
    let cfo_pat_5y = cumulative_cfo_pat_for_bundle(bundle, 5)
        .or_else(|| screener.and_then(|s| s.cumulative_cfo_pat_5y));
    let latest_cfo_pat = cf.first().zip(inc.first()).and_then(|(c, i)| {
        if i.net_income.abs() > 1e-6 {
            Some(c.operating_cashflow / i.net_income)
        } else {
            None
        }
    });

    let s3 = status_from_ratio_ge(cfo_pat_3y, 1.0);
    if s3 == AuditStatus::Fail {
        acc.red_flags.push("Cumulative CFO trails PAT over 3 years — earnings quality concern.".to_string());
    } else if s3 == AuditStatus::Pass {
        acc.strengths.push("Cumulative CFO covers PAT over 3 years.".to_string());
    }
    acc.checklist.push(AuditChecklistItem {
        metric: "Cumulative CFO / PAT (3Y)".to_string(),
        value: cfo_pat_3y,
        value_display: fmt_ratio(cfo_pat_3y),
        benchmark: "≥ 1.0".to_string(),
        status: s3,
        note: "Core earnings quality test: profits should convert to operating cash.".to_string(),
    });

    let s5 = status_from_ratio_ge(cfo_pat_5y, 1.0);
    acc.checklist.push(AuditChecklistItem {
        metric: "Cumulative CFO / PAT (5Y)".to_string(),
        value: cfo_pat_5y,
        value_display: fmt_ratio(cfo_pat_5y),
        benchmark: "≥ 1.0".to_string(),
        status: s5,
        note: "Longer window confirms sustained cash conversion.".to_string(),
    });

    let s_latest = status_from_ratio_ge(latest_cfo_pat, 1.0);
    acc.checklist.push(AuditChecklistItem {
        metric: "CFO / PAT (latest year)".to_string(),
        value: latest_cfo_pat,
        value_display: fmt_ratio(latest_cfo_pat),
        benchmark: "≥ 1.0".to_string(),
        status: s_latest,
        note: "Latest-year cash conversion vs reported profit.".to_string(),
    });

    let pat_cagr = if inc.len() >= 4 {
        cagr(inc[3].net_income, inc[0].net_income, 3.0)
    } else {
        None
    };
    let cfo_cagr = if cf.len() >= 4 {
        let start = cf[3].operating_cashflow;
        let end = cf[0].operating_cashflow;
        if start > 0.0 && end > 0.0 {
            cagr(start, end, 3.0)
        } else {
            None
        }
    } else {
        None
    };
    let divergence_fail = matches!((pat_cagr, cfo_cagr), (Some(p), Some(c)) if p > 5.0 && c < 0.0);
    let divergence_status = if divergence_fail {
        AuditStatus::Fail
    } else if matches!((pat_cagr, cfo_cagr), (Some(p), Some(c)) if p > 0.0 && c >= p - 5.0) {
        AuditStatus::Pass
    } else if pat_cagr.is_some() && cfo_cagr.is_some() {
        AuditStatus::Watch
    } else {
        AuditStatus::InsufficientData
    };
    if divergence_fail {
        acc.red_flags.push("PAT growing but CFO stagnant or falling — investigate accruals and collections.".to_string());
    }
    acc.checklist.push(AuditChecklistItem {
        metric: "PAT vs CFO growth (3Y)".to_string(),
        value: pat_cagr.zip(cfo_cagr).map(|(p, c)| p - c),
        value_display: match (pat_cagr, cfo_cagr) {
            (Some(p), Some(c)) => format!("PAT CAGR {:.1}% vs CFO CAGR {:.1}%", p, c),
            _ => "N/A".to_string(),
        },
        benchmark: "CFO growth ≥ PAT growth".to_string(),
        status: divergence_status,
        note: "Rising profit without cash is a classic quality red flag.".to_string(),
    });
}

#[allow(clippy::too_many_arguments)]
fn audit_growth_checks(
    acc: &mut AuditAccumulator,
    _bundle: &StatementBundle,
    inc: &[IncomeStatementRow],
    bal: &[BalanceSheetRow],
    _cf: &[CashflowRow],
    _financials: &Financials,
    _profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
) {
    let rec_dso = bal.first().zip(inc.first()).and_then(|(b, i)| {
        if i.revenue > 0.0 && b.net_receivables > 0.0 {
            Some((b.net_receivables / i.revenue) * 365.0)
        } else {
            screener.and_then(|s| s.days_receivable_outstanding)
        }
    });
    let rec_rev_flag = if let (Some(cur), Some(prev)) = (inc.get(0), inc.get(1)) {
        let rg = pct_change(cur.revenue, prev.revenue);
        let ag = bal
            .get(0)
            .zip(bal.get(1))
            .and_then(|(b0, b1)| pct_change(b0.net_receivables, b1.net_receivables));
        match (rg, ag) {
            (Some(r), Some(a)) if a > r + 10.0 => {
                if rec_dso.map(|d| d < 30.0).unwrap_or(false) {
                    Some((a - r, AuditStatus::Watch))
                } else {
                    acc.red_flags.push("Receivables growing faster than revenue — collection risk.".to_string());
                    Some((a - r, AuditStatus::Fail))
                }
            }
            (Some(r), Some(a)) if a > r + 5.0 => Some((a - r, AuditStatus::Watch)),
            (Some(_), Some(_)) => Some((0.0, AuditStatus::Pass)),
            _ => None,
        }
    } else {
        None
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Receivables growth vs revenue".to_string(),
        value: rec_rev_flag.map(|(d, _)| d),
        value_display: rec_rev_flag
            .map(|(d, _)| format!("{:.1} pp faster", d))
            .unwrap_or_else(|| "N/A".to_string()),
        benchmark: "Receivables ≤ revenue growth".to_string(),
        status: rec_rev_flag.map(|(_, s)| s).unwrap_or(AuditStatus::InsufficientData),
        note: rec_dso
            .map(|d| format!("Receivables growth warrants monitoring, but absolute DSO remains modest at ~{:.0} days.", d))
            .filter(|_| rec_dso.map(|d| d < 30.0).unwrap_or(false))
            .unwrap_or_else(|| "Loose credit terms can inflate top-line without cash.".to_string()),
    });

    let inv_rev_flag = if let (Some(cur), Some(prev)) = (inc.get(0), inc.get(1)) {
        let rg = pct_change(cur.revenue, prev.revenue);
        let ag = bal
            .get(0)
            .zip(bal.get(1))
            .and_then(|(b0, b1)| pct_change(b0.inventory, b1.inventory));
        match (rg, ag) {
            (Some(r), Some(a)) if a > r + 10.0 => {
                acc.red_flags.push("Inventory growing faster than sales — obsolescence / demand risk.".to_string());
                Some((a - r, AuditStatus::Fail))
            }
            (Some(r), Some(a)) if a > r + 5.0 => Some((a - r, AuditStatus::Watch)),
            (Some(_), Some(_)) => Some((0.0, AuditStatus::Pass)),
            _ => None,
        }
    } else {
        None
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Inventory growth vs revenue".to_string(),
        value: inv_rev_flag.map(|(d, _)| d),
        value_display: inv_rev_flag
            .map(|(d, _)| format!("{:.1} pp faster", d))
            .unwrap_or_else(|| "N/A".to_string()),
        benchmark: "Inventory ≤ sales growth".to_string(),
        status: inv_rev_flag.map(|(_, s)| s).unwrap_or(AuditStatus::InsufficientData),
        note: "Inventory pile-up ties up cash and signals weak demand.".to_string(),
    });

    let rec_days = bal.first().zip(inc.first()).and_then(|(b, i)| {
        if i.revenue > 0.0 && b.net_receivables > 0.0 {
            Some((b.net_receivables / i.revenue) * 365.0)
        } else {
            screener.and_then(|s| s.days_receivable_outstanding)
        }
    });
    let rec_days_old = bal.get(2).zip(inc.get(2)).and_then(|(b, i)| {
        if i.revenue > 0.0 && b.net_receivables > 0.0 {
            Some((b.net_receivables / i.revenue) * 365.0)
        } else {
            None
        }
    });
    let rec_trend = match (rec_days, rec_days_old) {
        (Some(n), Some(o)) if n > o + 15.0 && n >= 30.0 => AuditStatus::Fail,
        (Some(n), Some(o)) if n > o + 5.0 => AuditStatus::Watch,
        (Some(_), Some(_)) => AuditStatus::Pass,
        (Some(_), None) => AuditStatus::Watch,
        _ => AuditStatus::InsufficientData,
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Receivable days".to_string(),
        value: rec_days,
        value_display: fmt_days(rec_days),
        benchmark: "Flat or decreasing".to_string(),
        status: rec_trend,
        note: "Rising debtor days trap cash in working capital.".to_string(),
    });

    let inv_days = bal.first().zip(inc.first()).and_then(|(b, i)| {
        if i.cost_of_revenue > 0.0 && b.inventory > 0.0 {
            Some((b.inventory / i.cost_of_revenue) * 365.0)
        } else {
            None
        }
    });
    let inv_days_old = bal.get(2).zip(inc.get(2)).and_then(|(b, i)| {
        if i.cost_of_revenue > 0.0 && b.inventory > 0.0 {
            Some((b.inventory / i.cost_of_revenue) * 365.0)
        } else {
            None
        }
    });
    let inv_trend = match (inv_days, inv_days_old) {
        (Some(n), Some(o)) if n > o + 20.0 => AuditStatus::Fail,
        (Some(n), Some(o)) if n > o + 8.0 => AuditStatus::Watch,
        (Some(_), Some(_)) => AuditStatus::Pass,
        (Some(_), None) => AuditStatus::Watch,
        _ => AuditStatus::InsufficientData,
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Inventory days".to_string(),
        value: inv_days,
        value_display: fmt_days(inv_days),
        benchmark: "Flat or decreasing".to_string(),
        status: inv_trend,
        note: "Rising inventory days signal slower turnover.".to_string(),
    });
}

#[allow(clippy::too_many_arguments)]
fn audit_balance_sheet_checks(
    acc: &mut AuditAccumulator,
    _bundle: &StatementBundle,
    inc: &[IncomeStatementRow],
    bal: &[BalanceSheetRow],
    _cf: &[CashflowRow],
    financials: &Financials,
    profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
) {
    let b0 = bal.first();
    let equity = b0.map(|b| b.total_equity).unwrap_or(0.0);
    let debt = b0.map(|b| b.total_debt).unwrap_or(financials.total_debt);
    let d_eq = if equity.abs() > 1.0 {
        Some(debt / equity)
    } else {
        screener
            .and_then(|s| s.debt_to_equity)
            .or(financials.debt_to_equity)
    };
    let de_healthy = if is_asset_light(profile) { 0.3 } else { 0.5 };
    let de_warn = if is_asset_light(profile) { 0.8 } else { 1.0 };
    let de_status = status_from_ratio_le(d_eq, de_healthy, de_warn);
    if de_status == AuditStatus::Fail {
        acc.red_flags.push(format!(
            "Debt/equity {:.2} is elevated — solvency risk.",
            d_eq.unwrap_or(0.0)
        ));
    } else if de_status == AuditStatus::Pass {
        acc.strengths.push(format!(
            "Debt/equity {:.2} is within conservative range.",
            d_eq.unwrap_or(0.0)
        ));
    }
    acc.checklist.push(AuditChecklistItem {
        metric: "Debt / equity".to_string(),
        value: d_eq,
        value_display: d_eq.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "N/A".to_string()),
        benchmark: if is_asset_light(profile) {
            "< 0.3 (asset-light)".to_string()
        } else {
            "< 0.5 (asset-heavy)".to_string()
        },
        status: de_status,
        note: "Leverage amplifies downturn risk.".to_string(),
    });

    let interest = inc
        .first()
        .map(|r| r.interest_expense)
        .filter(|i| *i > 1e-6)
        .or_else(|| b0.map(|b| b.interest_expense).filter(|i| *i > 1e-6))
        .unwrap_or(0.0);
    let ebit = inc.first().map(|r| {
        if r.operating_income.abs() > 1.0 {
            r.operating_income
        } else {
            r.ebit.max(r.ebitda)
        }
    });
    let int_cov = screener
        .and_then(|s| s.interest_coverage_ratio)
        .or_else(|| {
            match (ebit, interest) {
                (Some(e), i) if i > 1e-6 => Some(e / i),
                _ => None,
            }
        });
    let ic_status = status_from_coverage(int_cov);
    if ic_status == AuditStatus::Fail {
        acc.red_flags.push("Interest coverage below 2x — debt service vulnerability.".to_string());
    } else if ic_status == AuditStatus::Pass {
        acc.strengths.push("Interest coverage above 4x — comfortable debt service.".to_string());
    }
    acc.checklist.push(AuditChecklistItem {
        metric: "Interest coverage".to_string(),
        value: int_cov,
        value_display: fmt_ratio(int_cov),
        benchmark: "> 4x".to_string(),
        status: ic_status,
        note: "EBIT must comfortably cover interest expense.".to_string(),
    });

    let local_roce = {
        let ebit = inc.first().map(|r| {
            if r.operating_income.abs() > 1.0 {
                r.operating_income
            } else {
                r.ebit.max(r.ebitda)
            }
        });
        match (ebit, b0) {
            (Some(e), Some(b)) => crate::yahoo_metrics::local_roce(e, b.total_equity, b.total_debt, b.cash),
            _ => None,
        }
    };
    let roce = screener
        .and_then(|s| s.return_on_capital_employed)
        .or(financials.return_on_capital_employed)
        .or(local_roce)
        .map(|x| x * 100.0);
    let roce_status = match roce {
        Some(x) if x >= 18.0 => AuditStatus::Pass,
        Some(x) if x >= 12.0 => AuditStatus::Watch,
        Some(x) if x > 0.0 => AuditStatus::Fail,
        _ => AuditStatus::InsufficientData,
    };
    if roce_status == AuditStatus::Pass {
        acc.strengths.push(format!("ROCE {:.1}% indicates strong capital efficiency.", roce.unwrap()));
    }
    acc.checklist.push(AuditChecklistItem {
        metric: "ROCE".to_string(),
        value: roce,
        value_display: fmt_pct(roce),
        benchmark: "> 15–20%".to_string(),
        status: roce_status,
        note: "Return on capital employed vs cost of capital.".to_string(),
    });

    let ca = b0.map(|b| b.current_assets).unwrap_or(0.0);
    let cl = b0.map(|b| b.current_liabilities).unwrap_or(0.0);
    let cr = financials
        .current_ratio
        .or_else(|| if cl > 0.0 { Some(ca / cl) } else { None });
    let cr_status = match cr {
        Some(x) if x >= 1.5 => AuditStatus::Pass,
        Some(x) if x >= 1.0 => AuditStatus::Watch,
        Some(x) if x > 0.0 => AuditStatus::Fail,
        _ => AuditStatus::InsufficientData,
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Current ratio".to_string(),
        value: cr,
        value_display: fmt_ratio(cr),
        benchmark: "≥ 1.0".to_string(),
        status: cr_status,
        note: "Short-term liquidity buffer.".to_string(),
    });

    let intangible_pct = b0.and_then(|b| {
        if b.total_assets > 0.0 {
            let intang = b.goodwill + b.intangible_assets;
            if intang > 0.0 {
                Some(intang / b.total_assets * 100.0)
            } else {
                None
            }
        } else {
            None
        }
    });
    let int_status = match intangible_pct {
        Some(x) if x < 15.0 => AuditStatus::Pass,
        Some(x) if x < 35.0 => AuditStatus::Watch,
        Some(_) => AuditStatus::Watch,
        None => AuditStatus::InsufficientData,
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Goodwill + intangibles / assets".to_string(),
        value: intangible_pct,
        value_display: fmt_pct(intangible_pct),
        benchmark: "< 15% preferred".to_string(),
        status: int_status,
        note: "High intangibles raise impairment risk.".to_string(),
    });

    let cash_auth = inc.first().zip(b0).and_then(|(i, b)| {
        if b.cash > 1e8 && i.revenue > 0.0 {
            let other = i.net_interest_income.max(i.other_income_expense);
            Some(other / b.cash * 100.0)
        } else {
            None
        }
    });
    let cash_status = match cash_auth {
        Some(x) if x >= 0.5 => AuditStatus::Pass,
        Some(x) if x >= 0.1 => AuditStatus::Watch,
        Some(_) => AuditStatus::Watch,
        None => AuditStatus::InsufficientData,
    };
    acc.checklist.push(AuditChecklistItem {
        metric: "Cash authenticity proxy".to_string(),
        value: cash_auth,
        value_display: fmt_pct(cash_auth),
        benchmark: "Interest/other income vs cash".to_string(),
        status: cash_status,
        note: "Large cash with negligible interest income warrants scrutiny.".to_string(),
    });
}

#[allow(clippy::too_many_arguments)]
fn audit_valuation_checks(
    acc: &mut AuditAccumulator,
    _bundle: &StatementBundle,
    _inc: &[IncomeStatementRow],
    _bal: &[BalanceSheetRow],
    _cf: &[CashflowRow],
    _financials: &Financials,
    _profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
) {
    if let Some(pf) = screener.and_then(|s| s.piotroski_f_score) {
        let ps = if pf >= 7.0 {
            AuditStatus::Pass
        } else if pf >= 5.0 {
            AuditStatus::Watch
        } else {
            AuditStatus::Fail
        };
        acc.checklist.push(AuditChecklistItem {
            metric: "Piotroski F-Score".to_string(),
            value: Some(pf),
            value_display: format!("{:.0}/9", pf),
            benchmark: "≥ 7".to_string(),
            status: ps,
            note: "Composite financial health score from screener.".to_string(),
        });
    }

    if let Some(az) = screener.and_then(|s| s.altman_z_score) {
        let zs = if az >= 3.0 {
            AuditStatus::Pass
        } else if az >= 1.8 {
            AuditStatus::Watch
        } else {
            AuditStatus::Fail
        };
        acc.checklist.push(AuditChecklistItem {
            metric: "Altman Z-Score".to_string(),
            value: Some(az),
            value_display: format!("{:.2}", az),
            benchmark: "≥ 3.0 safe zone".to_string(),
            status: zs,
            note: "Distress probability screen.".to_string(),
        });
    }
}

pub fn build_financial_strength_audit(
    bundle: &StatementBundle,
    financials: &Financials,
    profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
    bank: Option<&BankingMetrics>,
) -> FinancialStrengthAudit {
    build_financial_strength_audit_for(
        bundle,
        financials,
        profile,
        screener,
        bank,
        FinancialCompanyType::Industrial,
        DataCoverage::default(),
    )
}

pub fn build_financial_strength_audit_for(
    bundle: &StatementBundle,
    financials: &Financials,
    profile: &AssetProfile,
    screener: Option<&ScreenerMetricSnapshot>,
    bank: Option<&BankingMetrics>,
    company_type: FinancialCompanyType,
    coverage: DataCoverage,
) -> FinancialStrengthAudit {
    if company_type.is_lender() {
        return build_lender_financial_strength_audit(profile, screener, bank, company_type, coverage);
    }

    let mut inc = bundle.income_annual.clone();
    let mut bal = bundle.balance_annual.clone();
    let mut cf = bundle.cashflow_annual.clone();
    inc = sort_owned_desc(inc, |r| r.end_ts);
    bal = sort_owned_desc(bal, |r| r.end_ts);
    cf = sort_owned_desc(cf, |r| r.end_ts);

    let confidence = if inc.len() >= 5 && cf.len() >= 3 {
        "High"
    } else if inc.len() >= 3 {
        "Medium"
    } else {
        "Low"
    }
    .to_string();

    let mut acc = AuditAccumulator::new();
    audit_cashflow_checks(&mut acc, bundle, &inc, &bal, &cf, financials, profile, screener);
    audit_growth_checks(&mut acc, bundle, &inc, &bal, &cf, financials, profile, screener);
    audit_balance_sheet_checks(&mut acc, bundle, &inc, &bal, &cf, financials, profile, screener);
    audit_valuation_checks(&mut acc, bundle, &inc, &bal, &cf, financials, profile, screener);

    let earnings_items: Vec<_> = acc.checklist
        .iter()
        .filter(|i| {
            i.metric.contains("CFO")
                || i.metric.contains("PAT")
                || i.metric.contains("Receivable")
                || i.metric.contains("Inventory")
        })
        .cloned()
        .collect();
    let balance_items: Vec<_> = acc.checklist
        .iter()
        .filter(|i| {
            i.metric.contains("Debt")
                || i.metric.contains("Interest")
                || i.metric.contains("ROCE")
                || i.metric.contains("Current")
                || i.metric.contains("Goodwill")
                || i.metric.contains("Piotroski")
                || i.metric.contains("Altman")
                || i.metric.contains("Cash authenticity")
        })
        .cloned()
        .collect();

    let earnings_quality_score = score_from_checklist(&earnings_items);
    let balance_sheet_score = score_from_checklist(&balance_items);
    let overall_strength_score =
        earnings_quality_score * 0.55 + balance_sheet_score * 0.45;

    let fail_count = acc.checklist
        .iter()
        .filter(|i| i.status == AuditStatus::Fail)
        .count();
    let pass_count = acc.checklist
        .iter()
        .filter(|i| i.status == AuditStatus::Pass)
        .count();

    let interpretation = if fail_count >= 3 {
        format!(
            "Financial strength audit flags {fail_count} failed checks. Earnings quality score {:.0}/100 and balance sheet score {:.0}/100 suggest material fundamental risk — verify receivables, leverage, and cash conversion in annual filings before sizing a position.",
            earnings_quality_score, balance_sheet_score
        )
    } else if pass_count >= 5 && overall_strength_score >= 65.0 {
        format!(
            "Financial strength audit passes {pass_count} checks with overall score {:.0}/100. Earnings appear to convert to cash and the balance sheet screens serviceable, though contingent liabilities and related-party items require manual filing review.",
            overall_strength_score
        )
    } else {
        format!(
            "Mixed financial strength profile (overall {:.0}/100). Earnings quality {:.0}/100, balance sheet {:.0}/100 — some metrics need quarterly monitoring.",
            overall_strength_score, earnings_quality_score, balance_sheet_score
        )
    };

    FinancialStrengthAudit {
        earnings_quality_score: Some(earnings_quality_score),
        balance_sheet_score: Some(balance_sheet_score),
        overall_strength_score: Some(overall_strength_score),
        checklist: acc.checklist,
        red_flags: acc.red_flags,
        strengths: acc.strengths,
        interpretation,
        confidence,
        scores_provisional: false,
        financial_quality_display: format!("{:.0}", overall_strength_score),
        data_coverage: coverage,
        quality_weights_note: String::new(),
    }
}

fn filing_item(metric: &str, value: Option<f64>, display: String, benchmark: &str, status: AuditStatus, note: &str) -> AuditChecklistItem {
    AuditChecklistItem {
        metric: metric.to_string(),
        value,
        value_display: if value.is_some() { display } else { NA_FILING.to_string() },
        benchmark: benchmark.to_string(),
        status,
        note: note.to_string(),
    }
}

fn build_lender_financial_strength_audit(
    profile: &AssetProfile,
    _screener: Option<&ScreenerMetricSnapshot>,
    bank: Option<&BankingMetrics>,
    company_type: FinancialCompanyType,
    coverage: DataCoverage,
) -> FinancialStrengthAudit {
    let mut checklist = Vec::new();
    let mut red_flags = Vec::new();
    let mut strengths = Vec::new();
    let b = bank.cloned().unwrap_or_default();
    let is_bank = company_type.is_bank();

    let gnpa_status = status_from_pct_le(b.gnpa_pct, 3.0, 6.0);
    if gnpa_status == AuditStatus::Fail {
        red_flags.push("High GNPA / Stage 3 — asset quality stress.".to_string());
    } else if gnpa_status == AuditStatus::Pass {
        strengths.push("Low GNPA / Stage 3 — healthier asset quality.".to_string());
    }
    checklist.push(filing_item(
        "Gross Stage 3 / GNPA",
        b.gnpa_pct,
        fmt_pct(b.gnpa_pct),
        "< 3% preferred",
        gnpa_status,
        "Core asset-quality metric for lenders.",
    ));
    checklist.push(filing_item(
        "Net Stage 3 / NNPA",
        b.nnpa_pct,
        fmt_pct(b.nnpa_pct),
        "< 1% preferred",
        status_from_pct_le(b.nnpa_pct, 1.0, 3.0),
        "Net NPA after provisions.",
    ));
    let pcr_status = status_from_pct_ge(b.provision_coverage_ratio_pct, 70.0, 50.0);
    if pcr_status == AuditStatus::Fail {
        red_flags.push("Low provision coverage ratio.".to_string());
    } else if pcr_status == AuditStatus::Pass {
        strengths.push("Provision coverage is comfortable.".to_string());
    }
    checklist.push(filing_item(
        "Provision coverage ratio",
        b.provision_coverage_ratio_pct,
        fmt_pct(b.provision_coverage_ratio_pct),
        "> 70% preferred",
        pcr_status,
        "Cushion against future slippages.",
    ));
    for (m, v, bench, note) in [
        ("Credit cost", b.credit_cost_pct, "Lower/stable preferred", "Credit cost as % of loans."),
        ("Slippages", b.slippages_pct, "Trend vs GNPA", "Fresh NPA formation."),
        ("SMA-1", b.sma1_pct, "Watch early stress", "Special mention accounts 1."),
        ("SMA-2", b.sma2_pct, "Watch early stress", "Special mention accounts 2."),
        ("Restructured book", b.restructured_pct, "Low preferred", "Restructured exposure."),
        ("Stage-2 assets", b.stage2_pct, "Trend", "IFRS/Ind-AS Stage 2."),
    ] {
        checklist.push(filing_item(m, v, fmt_pct(v), bench, if v.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData }, note));
    }
    checklist.push(filing_item(
        "Recoveries",
        b.recoveries,
        b.recoveries.map(|x| format!("{:.0}", x)).unwrap_or_default(),
        "Positive",
        if b.recoveries.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
        "Cash recoveries from NPAs.",
    ));
    checklist.push(filing_item(
        "Write-offs",
        b.write_offs,
        b.write_offs.map(|x| format!("{:.0}", x)).unwrap_or_default(),
        "Contextual",
        if b.write_offs.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
        "Write-offs can mask headline GNPA.",
    ));

    checklist.push(filing_item(
        "CRAR",
        b.crar_pct,
        fmt_pct(b.crar_pct),
        if is_bank {
            "RBI minimum + management buffer; compare against bank peers and own history"
        } else {
            "> 15% typical comfort for NBFCs"
        },
        status_from_pct_ge(b.crar_pct, if is_bank { 12.5 } else { 15.0 }, if is_bank { 11.5 } else { 12.0 }),
        if is_bank {
            "Capital adequacy vs RBI Basel / D-SIB rules, not an NBFC comfort band."
        } else {
            "Capital adequacy vs RBI/NHB requirement."
        },
    ));
    checklist.push(filing_item(
        "Tier-I",
        b.tier1_pct,
        fmt_pct(b.tier1_pct),
        "> 12% preferred",
        status_from_pct_ge(b.tier1_pct, 12.0, 9.0),
        "Core capital.",
    ));
    checklist.push(filing_item(
        "NIM",
        b.nim_pct,
        fmt_pct(b.nim_pct),
        "Stable/expanding",
        if b.nim_pct.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
        "Net interest margin — primary operating metric.",
    ));
    checklist.push(filing_item(
        "Spread",
        b.spread_pct,
        fmt_pct(b.spread_pct),
        "Stable",
        if b.spread_pct.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
        "Yield minus cost of funds.",
    ));
    let loan_g = b.loan_book_growth_yoy_pct.or(b.credit_growth_yoy_pct);
    checklist.push(filing_item(
        "Loan book growth (YoY)",
        loan_g,
        fmt_pct(loan_g),
        "Balanced vs asset quality",
        if loan_g.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
        "Balance-sheet growth, not industrial revenue.",
    ));

    if is_bank {
        checklist.push(filing_item(
            "Deposit growth (YoY)",
            b.deposit_growth_yoy_pct,
            fmt_pct(b.deposit_growth_yoy_pct),
            "≥ credit growth",
            if b.deposit_growth_yoy_pct.is_some() { AuditStatus::Watch } else { AuditStatus::InsufficientData },
            "Funding franchise for banks.",
        ));
        checklist.push(filing_item(
            "CASA ratio",
            b.casa_ratio_pct,
            fmt_pct(b.casa_ratio_pct),
            "Higher is better",
            if b.casa_ratio_pct.map(|x| x >= 35.0).unwrap_or(false) {
                AuditStatus::Pass
            } else if b.casa_ratio_pct.is_some() {
                AuditStatus::Watch
            } else {
                AuditStatus::InsufficientData
            },
            "Low-cost deposits — bank-only.",
        ));
    } else {
        checklist.push(AuditChecklistItem {
            metric: "CASA / deposits".to_string(),
            value: None,
            value_display: NA_NOT_USED.to_string(),
            benchmark: "Not applicable to NBFCs".to_string(),
            status: AuditStatus::InsufficientData,
            note: "NBFCs fund via bonds/bank lines, not CASA.".to_string(),
        });
    }

    let aq = score_from_checklist_opt(
        &checklist
            .iter()
            .filter(|i| {
                i.metric.contains("GNPA")
                    || i.metric.contains("NNPA")
                    || i.metric.contains("Stage")
                    || i.metric.contains("PCR")
                    || i.metric.contains("coverage")
            })
            .cloned()
            .collect::<Vec<_>>(),
    );
    let cap = score_from_checklist_opt(
        &checklist
            .iter()
            .filter(|i| i.metric.contains("CRAR") || i.metric.contains("Tier"))
            .cloned()
            .collect::<Vec<_>>(),
    );

    let scores_provisional = coverage.recommendation_gated || aq.is_none();
    let earnings_quality_score = if scores_provisional { None } else { aq };
    let balance_sheet_score = if scores_provisional { None } else { cap };
    let overall_strength_score = if scores_provisional {
        None
    } else {
        match (aq, cap) {
            (Some(a), Some(c)) => Some(a * 0.62 + c * 0.38),
            (Some(a), None) => Some(a),
            (None, Some(c)) => Some(c),
            _ => None,
        }
    };

    let mode = if is_bank { "Bank" } else { "NBFC" };
    let interpretation = if coverage.recommendation_gated {
        format!(
            "{mode}-mode audit for {} / {}. Asset quality, CRAR and NIM are not in Yahoo (coverage {:.0}%; critical {:.0}%). Financial quality score is N/A — missing data is not mediocre asset quality. Do not treat industrial D/E or CFO/PAT as lender weakness.",
            profile.sector.as_deref().unwrap_or("Financials"),
            profile.industry.as_deref().unwrap_or("Credit"),
            coverage.overall_pct,
            coverage.critical_pct
        )
    } else {
        format!(
            "{mode}-mode audit. Asset quality and capital use filing metrics. Industrial EBITDA/ROCE/CFO-PAT are disabled."
        )
    };

    FinancialStrengthAudit {
        earnings_quality_score,
        balance_sheet_score,
        overall_strength_score,
        checklist,
        red_flags,
        strengths,
        interpretation,
        confidence: coverage.confidence.clone(),
        scores_provisional,
        financial_quality_display: if scores_provisional {
            "N/A".to_string()
        } else {
            overall_strength_score
                .map(|s| format!("{:.0}", s))
                .unwrap_or_else(|| "N/A".to_string())
        },
        data_coverage: coverage,
        quality_weights_note: if is_bank {
            "Bank weights: asset quality, CASA, NIM, credit/deposit growth, PCR, capital, ROA/ROE.".to_string()
        } else {
            "NBFC weights: asset quality 25, capital 15, profitability 15, NIM 10, growth 10, funding/ALM 10, governance 5, valuation 10.".to_string()
        },
    }
}

struct AuditRiskFlags {
    eq_fail: bool,
    bs_fail: bool,
    wc_drain: bool,
}

fn audit_risk_flags(audit: &FinancialStrengthAudit) -> AuditRiskFlags {
    if audit.scores_provisional || audit.data_coverage.recommendation_gated {
        return AuditRiskFlags {
            eq_fail: false,
            bs_fail: false,
            wc_drain: false,
        };
    }
    let lender_mode = audit.checklist.iter().any(|i| i.metric.contains("GNPA") || i.metric.contains("Stage 3"));
    let eq_fail = if lender_mode {
        audit.checklist.iter().any(|i| {
            (i.metric.contains("GNPA") || i.metric.contains("NNPA") || i.metric.contains("coverage ratio"))
                && i.status == AuditStatus::Fail
        })
    } else {
        audit.earnings_quality_score.map(|s| s < 45.0).unwrap_or(false)
            || audit.checklist.iter().any(|i| {
                i.metric.contains("Cumulative CFO / PAT (3Y)") && i.status == AuditStatus::Fail
            })
    };
    let bs_fail = if lender_mode {
        audit.checklist.iter().any(|i| {
            (i.metric.contains("CRAR") || i.metric.contains("Tier")) && i.status == AuditStatus::Fail
        })
    } else {
        audit.balance_sheet_score.map(|s| s < 45.0).unwrap_or(false)
            || audit.checklist.iter().any(|i| {
                (i.metric == "Debt / equity" || i.metric == "Interest coverage")
                    && i.status == AuditStatus::Fail
            })
    };
    let wc_drain = audit
        .red_flags
        .iter()
        .any(|f| f.contains("Receivables") || f.contains("Inventory"));
    AuditRiskFlags {
        eq_fail,
        bs_fail,
        wc_drain,
    }
}

fn guidance_from_audit_status(
    audit: &FinancialStrengthAudit,
    eq_fail: bool,
    expensive: bool,
    market: &MarketSignals,
) -> (Vec<String>, Vec<String>) {
    let mut wait_for_events = Vec::new();
    let mut rationale = Vec::new();
    if eq_fail {
        wait_for_events.push(
            "Next quarterly results — confirm CFO ≥ PAT and working capital stable.".to_string(),
        );
        rationale.push("Earnings quality screen failed — profits may not convert to cash.".to_string());
    }
    if audit
        .checklist
        .iter()
        .any(|i| i.metric == "Receivable days" && i.status == AuditStatus::Fail)
    {
        wait_for_events
            .push("Receivable days declining for two consecutive quarters.".to_string());
    }
    if audit
        .checklist
        .iter()
        .any(|i| i.metric == "Interest coverage" && i.status != AuditStatus::Pass)
    {
        wait_for_events.push("Interest coverage recovering above 4x.".to_string());
    }
    if market.analyst.net_bullish_score < -2 {
        wait_for_events.push(
            "Analyst consensus shifting from Sell toward Hold or Buy.".to_string(),
        );
    }
    if market
        .insider_transactions
        .iter()
        .map(|t| t.shares)
        .sum::<f64>()
        < 0.0
    {
        wait_for_events
            .push("Insider net buying after recent selling period.".to_string());
    }
    if expensive {
        wait_for_events.push(
            "Valuation correction toward historical median P/E or EV/EBITDA.".to_string(),
        );
    }
    (wait_for_events, rationale)
}

fn guidance_from_valuation(
    risks: &AuditRiskFlags,
    _strong: bool,
    _cheap: bool,
    expensive: bool,
    rating: &ResearchRating,
    rationale: &mut Vec<String>,
) -> (String, String) {
    if risks.eq_fail && (risks.wc_drain || risks.bs_fail) {
        rationale.push("Combined earnings quality and balance sheet stress.".to_string());
        ("Sell".to_string(), "Avoid".to_string())
    } else if risks.eq_fail || risks.bs_fail {
        ("Trim".to_string(), "Avoid".to_string())
    } else if rating.rating_label.to_lowercase().contains("buy")
        && rating.overall_score.is_some_and(|s| s >= 70.0)
        && !rating.technical_rating.to_lowercase().contains("weak")
        && !expensive
        && !risks.eq_fail
    {
        rationale.push("Rating supports accumulation; technical trend is not weak.".to_string());
        ("Hold".to_string(), "Buy".to_string())
    } else if rating.overall_score.is_some_and(|s| s >= 60.0) {
        rationale.push("Watchlist band: do not treat as a fresh full Buy.".to_string());
        ("Hold".to_string(), "Watch / stagger only".to_string())
    } else if rating.overall_score.is_some_and(|s| s >= 50.0)
        || expensive
        || rating.rating_label.contains("Avoid")
    {
        ("Hold".to_string(), "Wait".to_string())
    } else {
        ("Hold".to_string(), "Avoid".to_string())
    }
}

pub fn build_action_guidance(
    audit: &FinancialStrengthAudit,
    rating: &ResearchRating,
    valuation: &ValuationAnalysis,
    market: &MarketSignals,
    company_type: FinancialCompanyType,
) -> ActionGuidance {
    let risks = audit_risk_flags(audit);
    let expensive = valuation.valuation_label.contains("Expensive")
        || valuation.valuation_label.contains("Value Trap");
    let cheap = valuation.valuation_label == "Cheap"
        || valuation.valuation_label == "Very Cheap"
        || valuation.valuation_label == "Fairly Valued";
    let strong = audit.overall_strength_score.is_some_and(|s| s >= 65.0)
        && rating.financial_quality_score.is_some_and(|s| s >= 55.0);

    let (mut wait_for_events, mut rationale) =
        guidance_from_audit_status(audit, risks.eq_fail, expensive, market);
    let (if_holding, if_considering_entry) = if audit.data_coverage.recommendation_gated {
        if company_type.is_bank() {
            rationale.push(
                "Critical bank metrics (asset quality, capital, NIM, CASA, deposit/credit growth) are insufficient in Yahoo.".to_string(),
            );
            wait_for_events.push(
                "Annual report / investor presentation: GNPA, NNPA, PCR, CET1/CRAR, NIM, CASA, deposit growth, credit growth, LDR.".to_string(),
            );
        } else {
            rationale.push(
                "Critical lender metrics (asset quality, capital, NIM, loan growth) are insufficient in Yahoo.".to_string(),
            );
            wait_for_events.push("Annual report / investor presentation: GNPA, NNPA, PCR, CRAR, NIM, loan book.".to_string());
        }
        (
            "Hold pending filing verification".to_string(),
            "Incomplete — verify filings".to_string(),
        )
    } else {
        guidance_from_valuation(
            &risks,
            strong,
            cheap,
            expensive,
            rating,
            &mut rationale,
        )
    };

    if wait_for_events.is_empty() {
        wait_for_events.push("Verify next quarterly results against this audit checklist.".to_string());
    }
    wait_for_events.truncate(5);

    let headline = if audit.data_coverage.recommendation_gated {
        let noun = if company_type.is_bank() {
            "bank"
        } else if company_type.is_lender() {
            "NBFC"
        } else {
            "company"
        };
        format!(
            "Yahoo multiples are not a complete {noun} quality call: asset-quality, capital-adequacy and margin data are insufficient. Do not classify as a value trap solely from debt/CFO metrics."
        )
    } else {
        format!(
            "If holding: {if_holding}. If considering entry: {if_considering_entry}. Financial strength {}.",
            if audit.scores_provisional {
                "N/A".to_string()
            } else {
                audit
                    .overall_strength_score
                    .map(|s| format!("{:.0}/100", s))
                    .unwrap_or_else(|| "N/A".to_string())
            }
        )
    };

    ActionGuidance {
        if_holding,
        if_considering_entry,
        wait_for_events,
        headline,
        rationale_bullets: rationale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AssetProfile, BalanceSheetRow, CashflowRow, FinancialCompanyType, Financials, IncomeStatementRow,
        ResearchRating, ValuationAnalysis,
    };

    fn sample_bundle_pass() -> StatementBundle {
        let mut income = Vec::new();
        let mut cashflow = Vec::new();
        let mut balance = Vec::new();
        for i in 0..5 {
            let ts = 1_700_000_000 - i * 31_536_000;
            income.push(IncomeStatementRow {
                end_date_fmt: format!("202{}-03-31", 4 - i),
                end_ts: Some(ts),
                revenue: 1000.0 + i as f64 * 100.0,
                cost_of_revenue: 600.0,
                gross_profit: 400.0,
                ebitda: 200.0,
                operating_income: 150.0,
                ebit: 150.0,
                pretax_income: 140.0,
                interest_expense: 10.0,
                income_tax_expense: 30.0,
                depreciation: 20.0,
                net_income: 100.0 + i as f64 * 10.0,
                net_income_yahoo_row: None,
                period_type: String::new(),
                diluted_eps: Some(10.0),
                other_income_expense: 5.0,
                net_interest_income: 8.0,
                interest_income: 0.0,
                other_income: 0.0,
            });
            cashflow.push(CashflowRow {
                end_date_fmt: format!("202{}-03-31", 4 - i),
                end_ts: Some(ts),
                operating_cashflow: 120.0 + i as f64 * 10.0,
                capital_expenditure: 20.0,
                free_cashflow: 100.0,
                ..Default::default()
            });
            balance.push(BalanceSheetRow {
                end_date_fmt: format!("202{}-03-31", 4 - i),
                end_ts: Some(ts),
                cash: 500.0,
                total_debt: 200.0,
                total_equity: 800.0,
                total_assets: 1200.0,
                total_liabilities: 400.0,
                current_assets: 400.0,
                current_liabilities: 200.0,
                interest_expense: 10.0,
                inventory: 50.0 + i as f64,
                net_receivables: 80.0 + i as f64 * 2.0,
                retained_earnings: 300.0,
                goodwill: 20.0,
                intangible_assets: 10.0,
                ..Default::default()
            });
        }
        StatementBundle {
            income_annual: income,
            income_quarterly: vec![],
            balance_annual: balance,
            balance_quarterly: vec![],
            cashflow_annual: cashflow,
            cashflow_quarterly: vec![],
        }
    }

    #[test]
    fn cumulative_cfo_pat_passes_on_healthy_fixture() {
        let bundle = sample_bundle_pass();
        let audit = build_financial_strength_audit(
            &bundle,
            &Financials::default(),
            &AssetProfile::default(),
            None,
            None,
        );
        let cfo3 = audit
            .checklist
            .iter()
            .find(|i| i.metric.contains("3Y"))
            .unwrap();
        assert!(matches!(cfo3.status, AuditStatus::Pass | AuditStatus::Watch));
        assert!(audit.overall_strength_score.unwrap_or(0.0) > 50.0);
    }

    #[test]
    fn action_guidance_buy_when_strong_and_cheap() {
        let bundle = sample_bundle_pass();
        let audit = build_financial_strength_audit(
            &bundle,
            &Financials {
                return_on_capital_employed: Some(0.22),
                debt_to_equity: Some(0.3),
                ..Financials::default()
            },
            &AssetProfile::default(),
            None,
            None,
        );
        let rating = ResearchRating {
            overall_score: Some(72.0),
            financial_quality_score: Some(70.0),
            rating_label: "Buy / Accumulate".to_string(),
            technical_rating: "Neutral".to_string(),
            ..ResearchRating::default()
        };
        let valuation = ValuationAnalysis {
            valuation_label: "Cheap".to_string(),
            ..ValuationAnalysis::default()
        };
        let action = build_action_guidance(&audit, &rating, &valuation, &MarketSignals::default(), FinancialCompanyType::Industrial);
        assert_eq!(action.if_considering_entry, "Buy");
    }

    #[test]
    fn action_guidance_watchlist_is_not_buy() {
        let bundle = sample_bundle_pass();
        let audit = build_financial_strength_audit(
            &bundle,
            &Financials {
                return_on_capital_employed: Some(0.22),
                debt_to_equity: Some(0.3),
                ..Financials::default()
            },
            &AssetProfile::default(),
            None,
            None,
        );
        let rating = ResearchRating {
            overall_score: Some(65.5),
            financial_quality_score: Some(70.0),
            rating_label: "Watchlist".to_string(),
            technical_rating: "Neutral".to_string(),
            ..ResearchRating::default()
        };
        let valuation = ValuationAnalysis {
            valuation_label: "Cheap".to_string(),
            ..ValuationAnalysis::default()
        };
        let action = build_action_guidance(&audit, &rating, &valuation, &MarketSignals::default(), FinancialCompanyType::Industrial);
        assert_eq!(action.if_considering_entry, "Watch / stagger only");
    }

    #[test]
    fn rec_yahoo_only_audit_does_not_score_missing_as_fifty() {
        let profile = AssetProfile {
            sector: Some("Financial Services".into()),
            industry: Some("Credit Services".into()),
            long_name: Some("REC Limited".into()),
            long_business_summary: Some("Rural electrification project finance.".into()),
            ..Default::default()
        };
        let coverage = crate::financial_company::build_data_coverage(
            FinancialCompanyType::NbfcProjectFinance,
            &Financials::default(),
            &crate::models::CanonicalMetrics::default(),
            None,
        );
        let audit = build_financial_strength_audit_for(
            &sample_bundle_pass(),
            &Financials::default(),
            &profile,
            None,
            None,
            FinancialCompanyType::NbfcProjectFinance,
            coverage,
        );
        assert!(audit.scores_provisional);
        assert_eq!(audit.financial_quality_display, "N/A");
        assert!(audit.earnings_quality_score.is_none());
        assert!(audit.balance_sheet_score.is_none());
        assert!(audit.overall_strength_score.is_none());
        assert!(audit.interpretation.contains("not in Yahoo") || audit.interpretation.contains("N/A"));
        assert!(audit.checklist.iter().any(|i| i.metric.contains("CASA") && (i.value_display.contains("Not") || i.note.contains("NBFCs"))));
    }

    #[test]
    fn bank_crar_benchmark_is_not_nbfc_text() {
        let profile = AssetProfile {
            sector: Some("Financial Services".into()),
            industry: Some("Banks - Regional".into()),
            ..Default::default()
        };
        let coverage = crate::financial_company::build_data_coverage(
            FinancialCompanyType::Bank,
            &Financials::default(),
            &crate::models::CanonicalMetrics::default(),
            None,
        );
        let audit = build_financial_strength_audit_for(
            &sample_bundle_pass(),
            &Financials::default(),
            &profile,
            None,
            None,
            FinancialCompanyType::Bank,
            coverage,
        );
        let crar = audit.checklist.iter().find(|i| i.metric == "CRAR").unwrap();
        assert!(crar.benchmark.contains("RBI minimum"));
        assert!(!crar.benchmark.contains("NBFCs"));
    }

    #[test]
    fn bank_gated_guidance_uses_bank_quality_wording() {
        let coverage = crate::financial_company::build_data_coverage(
            FinancialCompanyType::Bank,
            &Financials::default(),
            &crate::models::CanonicalMetrics::default(),
            None,
        );
        let audit = build_financial_strength_audit_for(
            &sample_bundle_pass(),
            &Financials::default(),
            &AssetProfile {
                sector: Some("Financial Services".into()),
                industry: Some("Banks - Regional".into()),
                ..Default::default()
            },
            None,
            None,
            FinancialCompanyType::Bank,
            coverage,
        );
        assert!(audit.data_coverage.recommendation_gated);
        let action = build_action_guidance(
            &audit,
            &ResearchRating::default(),
            &ValuationAnalysis::default(),
            &MarketSignals::default(),
            FinancialCompanyType::Bank,
        );
        assert!(action.headline.contains("bank quality call"));
        assert!(!action.headline.contains("bank/NBFC"));
        assert!(action
            .wait_for_events
            .iter()
            .any(|e| e.contains("CASA") && e.contains("LDR") && e.contains("CET1")));
    }

    #[test]
    fn action_guidance_avoid_when_cfo_trails_pat() {
        let mut bundle = sample_bundle_pass();
        for cf in bundle.cashflow_annual.iter_mut() {
            cf.operating_cashflow = 5.0;
        }
        let audit = build_financial_strength_audit(
            &bundle,
            &Financials::default(),
            &AssetProfile::default(),
            None,
            None,
        );
        let cfo3 = audit
            .checklist
            .iter()
            .find(|i| i.metric.contains("3Y"))
            .unwrap();
        assert_eq!(cfo3.status, AuditStatus::Fail);
        let rating = ResearchRating::default();
        let valuation = ValuationAnalysis::default();
        let action = build_action_guidance(&audit, &rating, &valuation, &MarketSignals::default(), FinancialCompanyType::Industrial);
        assert_eq!(action.if_considering_entry, "Avoid");
        assert!(action.if_holding == "Sell" || action.if_holding == "Trim");
    }
}
