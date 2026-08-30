//! Statement-derived metrics every scorer and narrative should prefer over Yahoo quoteSummary zeros.

use crate::models::{CanonicalMetrics, FinancialCompanyType, Financials, IncomeStatementRow, StatementBundle};
use crate::statements::{
    balance_annual_desc, cashflow_annual_desc, cashflow_quarterly_asc, income_annual_asc,
    income_annual_desc,
};
use crate::yahoo_metrics::{canonical_statement_fcf, local_roce};

fn nonzero(v: f64) -> Option<f64> {
    if v.is_finite() && v.abs() > 1e-9 {
        Some(v)
    } else {
        None
    }
}

pub fn reported_nii(row: &IncomeStatementRow) -> Option<f64> {
    nonzero(row.net_interest_income)
}

pub fn calculated_nii(row: &IncomeStatementRow) -> Option<f64> {
    match (nonzero(row.interest_income), nonzero(row.interest_expense)) {
        (Some(ii), Some(ie)) => Some(ii - ie),
        _ => None,
    }
}

/// Prefer Yahoo `netInterestIncome` when both years disclose it; else II − IE.
pub fn preferred_nii_yoy(latest: &IncomeStatementRow, prior: &IncomeStatementRow) -> Option<f64> {
    match (reported_nii(latest), reported_nii(prior)) {
        (Some(a), Some(b)) if b.abs() > 1.0 => Some((a / b - 1.0) * 100.0),
        _ => match (calculated_nii(latest), calculated_nii(prior)) {
            (Some(a), Some(b)) if b.abs() > 1.0 => Some((a / b - 1.0) * 100.0),
            _ => None,
        },
    }
}

fn ttm_sum_income(q: &[&IncomeStatementRow], f: impl Fn(&IncomeStatementRow) -> f64) -> Option<f64> {
    if q.len() < 4 {
        return None;
    }
    let s: f64 = q[q.len() - 4..].iter().map(|r| f(r)).sum();
    nonzero(s)
}

pub fn build_canonical(bundle: &StatementBundle, yahoo: &Financials) -> CanonicalMetrics {
    build_canonical_for(bundle, yahoo, FinancialCompanyType::Industrial)
}

pub fn build_canonical_for(
    bundle: &StatementBundle,
    yahoo: &Financials,
    company_type: FinancialCompanyType,
) -> CanonicalMetrics {
    let inc_a = income_annual_desc(bundle);
    let inc_q = crate::statements::true_quarterly_income_rows(bundle);
    let cf_a = cashflow_annual_desc(bundle);
    let cf_q = cashflow_quarterly_asc(bundle);
    let bal = balance_annual_desc(bundle);

    let latest_i = inc_a.first().copied();
    let latest_c = cf_a.first().copied();
    let latest_b = bal.first().copied();

    let ttm_pat = ttm_sum_income(&inc_q, |r| r.net_income);
    let fy_pat = latest_i.and_then(|r| nonzero(r.net_income));
    let q_pat_row = crate::statements::latest_quarterly_income_row(bundle);
    let latest_yahoo_quarter_pat = q_pat_row.and_then(|r| nonzero(r.net_income));
    let latest_yahoo_quarter_pat_period = q_pat_row
        .map(|r| r.end_date_fmt.clone())
        .unwrap_or_default();
    let latest_yahoo_quarter_pat_source_column = q_pat_row
        .and_then(|r| r.net_income_yahoo_row.clone())
        .unwrap_or_default();
    let as_of = chrono::Utc::now().date_naive();
    let freshness = crate::statements::yahoo_quarter_freshness(
        if latest_yahoo_quarter_pat_period.is_empty() {
            None
        } else {
            Some(latest_yahoo_quarter_pat_period.as_str())
        },
        as_of,
    );
    let (pat, pat_period) = if ttm_pat.is_some() {
        (ttm_pat, "ttm")
    } else if fy_pat.is_some() {
        (fy_pat, "fy")
    } else if yahoo.net_income.is_some() {
        (yahoo.net_income, "yahoo_quote_ttm")
    } else {
        (None, "")
    };
    let pat_yahoo_row = latest_i.and_then(|r| r.net_income_yahoo_row.clone());
    let pat_scope = "unknown — Yahoo does not label standalone vs consolidated".to_string();
    let revenue = ttm_sum_income(&inc_q, |r| r.revenue)
        .or_else(|| latest_i.and_then(|r| nonzero(r.revenue)))
        .or(nonzero(yahoo.revenue));

    let cfo = if cf_q.len() >= 4 {
        nonzero(cf_q[cf_q.len() - 4..].iter().map(|r| r.operating_cashflow).sum())
    } else {
        None
    }
    .or_else(|| latest_c.and_then(|r| nonzero(r.operating_cashflow)))
    .or(yahoo.operating_cashflow);

    let capex = if cf_q.len() >= 4 {
        nonzero(cf_q[cf_q.len() - 4..].iter().map(|r| r.capital_expenditure).sum())
    } else {
        None
    }
    .or_else(|| latest_c.and_then(|r| nonzero(r.capital_expenditure)));

    let fcf = if company_type.is_lender() {
        None
    } else {
        canonical_statement_fcf(bundle)
            .or_else(|| latest_c.and_then(|r| r.calculated_fcf).filter(|x| x.abs() > 1e-9))
    };

    let cce = latest_b.and_then(|b| nonzero(b.cash_and_cash_equivalents).or(nonzero(b.cash)));
    let sti = latest_b.and_then(|b| nonzero(b.short_term_investments));
    let liquid = match (cce, sti) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        _ => latest_b.and_then(|b| nonzero(b.cash)).or(nonzero(yahoo.total_cash)),
    };
    let debt = latest_b
        .and_then(|b| nonzero(b.total_debt))
        .or(nonzero(yahoo.total_debt));
    let net_vs_cce_raw = match (debt, cce) {
        (Some(d), Some(c)) => Some(d - c),
        _ => None,
    };
    let net_vs_liquid_raw = match (debt, liquid) {
        (Some(d), Some(l)) => Some(d - l),
        _ => None,
    };
    let is_net_cash_equivalents = if company_type.is_lender() {
        false
    } else {
        matches!((cce, debt), (Some(c), Some(d)) if c > d && d >= 0.0)
    };
    let (net_vs_cce, net_vs_liquid) = if company_type.is_lender() {
        (None, None)
    } else {
        (net_vs_cce_raw, net_vs_liquid_raw)
    };

    let current_ratio = if company_type.is_lender() {
        None
    } else {
        latest_b
            .and_then(|b| {
                if b.current_liabilities > 1e-6 {
                    Some(b.current_assets / b.current_liabilities)
                } else {
                    None
                }
            })
            .or(yahoo.current_ratio)
    };

    let ebit = latest_i.map(|r| {
        if r.operating_income.abs() > 1.0 {
            r.operating_income
        } else {
            r.ebit.max(r.ebitda)
        }
    });
    let interest = latest_i
        .map(|r| r.interest_expense)
        .filter(|i| *i > 1e-6);
    let interest_coverage = if company_type.is_lender() {
        None
    } else {
        match (ebit, interest) {
            (Some(e), Some(i)) => Some(e / i),
            _ => None,
        }
    };

    let roce = if company_type.is_lender() {
        None
    } else {
        match (ebit, latest_b) {
            (Some(e), Some(b)) => local_roce(e, b.total_equity, b.total_debt, b.cash_and_cash_equivalents.max(b.cash)),
            _ => None,
        }
        .or(yahoo.return_on_capital_employed)
    };

    let inc_asc = income_annual_asc(bundle);
    let rev_levels: Vec<f64> = inc_asc.iter().map(|r| r.revenue).collect();
    let ni_levels: Vec<f64> = inc_asc.iter().map(|r| r.net_income).collect();
    let yahoo_rev_g = yahoo.revenue_growth.filter(|x| x.abs() > 1e-9);
    let rev_cagr = crate::series_integrity::trailing_cagr_pct(&rev_levels, 3, yahoo_rev_g);
    let ni_cagr = crate::series_integrity::trailing_cagr_pct(&ni_levels, 3, None);
    let mut revenue_cagr_3y_pct = rev_cagr.value;
    let mut pat_cagr_3y_pct = ni_cagr.value;
    if let Some(latest) = inc_asc.last() {
        if !crate::series_integrity::aligns_with_quote(latest.revenue, nonzero(yahoo.revenue)) {
            revenue_cagr_3y_pct = None;
        }
        if !crate::series_integrity::aligns_with_quote(latest.net_income, yahoo.net_income) {
            pat_cagr_3y_pct = None;
        }
    }
    let fy_revenue_yoy_pct = crate::series_integrity::fy_yoy_pct(&rev_levels).filter(|_| {
        crate::series_integrity::latest_step_usable(&rev_levels, yahoo_rev_g)
            && inc_asc
                .last()
                .map(|r| crate::series_integrity::aligns_with_quote(r.revenue, nonzero(yahoo.revenue)))
                .unwrap_or(true)
    });
    let fy_pat_yoy_pct = crate::series_integrity::fy_yoy_pct(&ni_levels).filter(|_| {
        crate::series_integrity::latest_step_usable(&ni_levels, None)
            && inc_asc
                .last()
                .map(|r| crate::series_integrity::aligns_with_quote(r.net_income, yahoo.net_income))
                .unwrap_or(true)
    });
    let ii_levels: Vec<f64> = inc_asc.iter().map(|r| r.interest_income).collect();
    let interest_income_yoy_pct = crate::series_integrity::fy_yoy_pct(&ii_levels).filter(|_| {
        ii_levels.iter().rev().take(2).all(|x| x.abs() > 1.0)
    });
    let (revenue_cagr_3y_pct, fy_revenue_yoy_pct) = if company_type.is_lender() {
        (None, None)
    } else {
        (revenue_cagr_3y_pct, fy_revenue_yoy_pct)
    };

    let mut notes = Vec::new();
    notes.push("Yahoo quoteSummary fields stay on `financials`; this object is statement-first.".to_string());
    if !freshness.note.is_empty() {
        notes.push(freshness.note.clone());
    }
    notes.extend(rev_cagr.flags.iter().cloned());
    notes.extend(ni_cagr.flags.iter().cloned());
    if yahoo.net_income.is_none() && pat.is_some() {
        notes.push("Yahoo TTM PAT missing; PAT taken from statements.".to_string());
    }
    if !company_type.is_lender()
        && !is_net_cash_equivalents
        && net_vs_liquid.map(|n| n < 0.0).unwrap_or(false)
    {
        notes.push("Liquid investments exceed debt, but cash-and-equivalents do not — not labelled net-cash.".to_string());
    }
    if company_type.is_lender() {
        notes.push("Industrial metrics (EBITDA, ROCE, FCF, current ratio, interest coverage, net debt) suppressed for lending companies.".to_string());
        notes.push("Cash is statement cash only — not a liquidity-strength signal. Lender liquidity needs ALM, undrawn lines, liquid investments, maturity profile, and borrowing access.".to_string());
        notes.push("Yahoo totalRevenue is stored as yahoo_revenue_field — not labelled total income (it can be below interest income). Industrial revenue CAGR is suppressed.".to_string());
        notes.push(format!("Selected PAT is {pat_period} ({pat_scope})."));
    }

    let interest_income = latest_i.and_then(|r| nonzero(r.interest_income));
    let interest_expense = latest_i.and_then(|r| nonzero(r.interest_expense));
    let reported_nii = latest_i.and_then(|r| nonzero(r.net_interest_income));
    let calculated_nii = match (interest_income, interest_expense) {
        (Some(ii), Some(ie)) => Some(ii - ie),
        _ => None,
    };
    let (nii, nii_definition, nii_source) = if reported_nii.is_some() {
        (
            reported_nii,
            "Yahoo Net Interest Income statement row",
            "yahoo_reported_nii",
        )
    } else if calculated_nii.is_some() {
        (calculated_nii, "calculated", "calculated_nii")
    } else {
        (None, "", "")
    };
    let nii_reconciliation_difference = match (calculated_nii, reported_nii) {
        (Some(c), Some(r)) => Some(c - r),
        _ => None,
    };
    let nii_yoy_pct = latest_i.zip(inc_a.get(1).copied()).and_then(|(a, b)| preferred_nii_yoy(a, b));
    if company_type.is_lender() {
        if nii_definition == "calculated" {
            notes.push("NII is interest income minus interest expense; Yahoo netInterestIncome was missing. This can differ from company-reported NII if finance-cost rows differ.".to_string());
        } else if nii_definition == "Yahoo Net Interest Income statement row" {
            notes.push("NII is Yahoo netInterestIncome (statement row), not necessarily company-presented NII.".to_string());
        }
    }
    let other_income = latest_i.and_then(|r| nonzero(r.other_income).or(nonzero(r.other_income_expense)));
    let yahoo_loan_book_field = latest_b.and_then(|b| nonzero(b.net_loans));
    let yahoo_loan_book_row = latest_b
        .and_then(|b| b.net_loans_yahoo_row.clone())
        .unwrap_or_default();
    let loan_prev = bal.get(1).and_then(|b| nonzero(b.net_loans));
    let yahoo_loan_book_growth_yoy_pct = match (yahoo_loan_book_field, loan_prev) {
        (Some(a), Some(p)) if p.abs() > 1e-6 => Some((a / p - 1.0) * 100.0),
        _ => None,
    };
    let canonical_advances = None;
    let (loan_book, loan_book_growth_yoy_pct) = if company_type.is_bank() {
        (None, None)
    } else {
        (yahoo_loan_book_field, yahoo_loan_book_growth_yoy_pct)
    };
    if company_type.is_bank() && yahoo_loan_book_field.is_some() {
        notes.push(
            "Yahoo loans/receivables are not treated as canonical gross advances; row name is recorded. Official bank advances belong in filings.".to_string(),
        );
    }

    CanonicalMetrics {
        cfo: if company_type.is_lender() { None } else { cfo },
        capex: if company_type.is_lender() { None } else { capex },
        fcf,
        pat,
        fy_pat,
        ttm_pat,
        latest_yahoo_quarter_pat,
        latest_yahoo_quarter_pat_period,
        latest_yahoo_quarter_pat_source_column,
        latest_reported_quarter_end: None,
        quarterly_statement_stale: freshness.stale,
        quarterly_statement_age_days: freshness.age_days,
        quarterly_statement_stale_note: freshness.note.clone(),
        pat_period: pat_period.to_string(),
        pat_scope,
        pat_yahoo_row,
        revenue: if company_type.is_lender() { None } else { revenue },
        roce,
        current_ratio,
        interest_coverage,
        cash_and_cash_equivalents: cce,
        short_term_investments: sti,
        gross_cash_and_liquid_investments: liquid,
        total_debt: debt,
        net_debt_vs_cash_equivalents: net_vs_cce,
        net_debt_vs_liquid: net_vs_liquid,
        is_net_cash_equivalents,
        raw_balance_sheet: if company_type.is_lender() {
            Some(crate::models::RawBalanceSheetMetrics {
                cash_and_cash_equivalents: cce,
                short_term_investments: sti,
                total_debt: debt,
                net_debt_vs_cash_equivalents: net_vs_cce_raw,
                net_debt_vs_liquid: net_vs_liquid_raw,
                note: "Industrial net-debt arithmetic from the statement. Not a lender solvency or liquidity score.".to_string(),
            })
        } else {
            None
        },
        revenue_cagr_3y_pct,
        pat_cagr_3y_pct,
        fy_revenue_yoy_pct,
        fy_pat_yoy_pct,
        interest_income,
        yahoo_revenue_field: revenue,
        interest_expense,
        net_interest_income: nii,
        canonical_nii: nii,
        canonical_nii_source: nii_source.to_string(),
        nii_reconciliation_difference,
        calculated_nii,
        yahoo_reported_nii: reported_nii,
        nii_definition: nii_definition.to_string(),
        other_income,
        yahoo_loan_book_field,
        yahoo_loan_book_row,
        yahoo_loan_book_growth_yoy_pct,
        canonical_advances,
        loan_book,
        loan_book_growth_yoy_pct,
        interest_income_yoy_pct,
        nii_yoy_pct,
        industrial_metrics_suppressed: company_type.is_lender(),
        notes,
    }
}

pub fn peer_comparability(industry: Option<&str>, summary: Option<&str>) -> String {
    let ind = industry.unwrap_or("").to_lowercase();
    let sum = summary.unwrap_or("").to_lowercase();
    let oilish = ind.contains("oil")
        || ind.contains("refin")
        || ind.contains("gas")
        || ind.contains("energy");
    let conglomerate = sum.contains("telecom")
        || sum.contains("retail")
        || sum.contains("digital")
        || sum.contains("jio")
        || sum.contains("conglomerate")
        || sum.contains("diversif");
    if oilish && conglomerate {
        "low".to_string()
    } else if oilish || conglomerate {
        "medium".to_string()
    } else {
        "medium".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BalanceSheetRow, CashflowRow, FinancialCompanyType, IncomeStatementRow, StatementBundle};

    #[test]
    fn cash_flow_quality_inputs_come_from_statements_not_yahoo_zero() {
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                net_income: 100.0,
                revenue: 1000.0,
                operating_income: 150.0,
                interest_expense: 10.0,
                ..Default::default()
            }],
            cashflow_annual: vec![CashflowRow {
                operating_cashflow: 120.0,
                capital_expenditure: 40.0,
                free_cashflow: 80.0,
                calculated_fcf: Some(80.0),
                ..Default::default()
            }],
            ..Default::default()
        };
        let yahoo = Financials {
            net_income: None,
            operating_cashflow: None,
            free_cashflow: None,
            ..Default::default()
        };
        let c = build_canonical(&bundle, &yahoo);
        assert_eq!(c.pat, Some(100.0));
        assert_eq!(c.cfo, Some(120.0));
        assert_eq!(c.fcf, Some(80.0));
        assert!(c.interest_coverage.unwrap() > 10.0);
    }

    #[test]
    fn mixed_scope_annual_revenue_does_not_yield_cagr() {
        let bundle = StatementBundle {
            income_annual: vec![
                IncomeStatementRow {
                    end_date_fmt: "2026-03-31".into(),
                    end_ts: Some(4),
                    revenue: 10.57219e12,
                    net_income: 807.75e9,
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2025-03-31".into(),
                    end_ts: Some(3),
                    revenue: 5.17349e12,
                    net_income: 352.62e9,
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2024-03-31".into(),
                    end_ts: Some(2),
                    revenue: 5.34534e12,
                    net_income: 420.42e9,
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2023-03-31".into(),
                    end_ts: Some(1),
                    revenue: 5.29773e12,
                    net_income: 442.05e9,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let yahoo = Financials {
            revenue: 11.296e12,
            revenue_growth: Some(0.297),
            net_income: Some(750e9),
            ..Default::default()
        };
        let c = build_canonical(&bundle, &yahoo);
        assert!(c.revenue_cagr_3y_pct.is_none());
        assert!(c.notes.iter().any(|n| n.contains("SCOPE") || n.contains("inconsistent")));
    }

    #[test]
    fn reported_nii_is_preferred_over_interest_spread() {
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                interest_income: 578.62e9,
                interest_expense: 362.38e9,
                net_interest_income: 207.50e9,
                net_income: 162.82e9,
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = build_canonical_for(&bundle, &Financials::default(), FinancialCompanyType::NbfcProjectFinance);
        assert!((c.calculated_nii.unwrap() - 216.24e9).abs() < 1e7);
        assert!((c.yahoo_reported_nii.unwrap() - 207.50e9).abs() < 1e7);
        assert_eq!(c.nii_definition, "Yahoo Net Interest Income statement row");
        assert_eq!(c.canonical_nii_source, "yahoo_reported_nii");
        assert!((c.net_interest_income.unwrap() - 207.50e9).abs() < 1e7);
        assert!((c.canonical_nii.unwrap() - 207.50e9).abs() < 1e7);
        assert!((c.nii_reconciliation_difference.unwrap() - 8.74e9).abs() < 1e8);
        assert!(c.revenue.is_none());
        assert!(c.net_debt_vs_cash_equivalents.is_none());
        assert!(c.raw_balance_sheet.is_some());
    }

    #[test]
    fn equal_calculated_and_reported_nii_still_records_source() {
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                interest_income: 578.6187e9,
                interest_expense: 362.3812e9,
                net_interest_income: 216.2375e9,
                net_income: 162.82e9,
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = build_canonical_for(
            &bundle,
            &Financials::default(),
            FinancialCompanyType::NbfcProjectFinance,
        );
        assert!((c.calculated_nii.unwrap() - 216.2375e9).abs() < 1e5);
        assert!((c.yahoo_reported_nii.unwrap() - 216.2375e9).abs() < 1e5);
        assert_eq!(c.canonical_nii_source, "yahoo_reported_nii");
        assert!(c.nii_reconciliation_difference.unwrap().abs() < 1e5);
    }

    #[test]
    fn calculated_nii_when_yahoo_net_interest_income_missing() {
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                interest_income: 578.62e9,
                interest_expense: 362.38e9,
                net_interest_income: 0.0,
                net_income: 163.08e9,
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = build_canonical_for(&bundle, &Financials::default(), FinancialCompanyType::NbfcProjectFinance);
        assert_eq!(c.nii_definition, "calculated");
        assert_eq!(c.canonical_nii_source, "calculated_nii");
        assert!((c.net_interest_income.unwrap() - 216.24e9).abs() < 1e7);
        assert!(c.nii_reconciliation_difference.is_none());
    }

    #[test]
    fn pat_period_distinguishes_ttm_from_fy() {
        let q = |ni: f64| IncomeStatementRow {
            net_income: ni,
            ..Default::default()
        };
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                net_income: 163.08e9,
                net_income_yahoo_row: Some("Net Income".into()),
                ..Default::default()
            }],
            income_quarterly: vec![q(40e9), q(40e9), q(40e9), q(40.86e9)],
            ..Default::default()
        };
        let c = build_canonical(&bundle, &Financials::default());
        assert!((c.fy_pat.unwrap() - 163.08e9).abs() < 1.0);
        assert!((c.ttm_pat.unwrap() - 160.86e9).abs() < 1e7);
        assert_eq!(c.pat_period, "ttm");
        assert!(c.pat_scope.contains("unknown"));
        assert!((c.pat.unwrap() - c.ttm_pat.unwrap()).abs() < 1.0);
    }

    #[test]
    fn latest_quarter_pat_pairs_with_period_not_fy() {
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                end_date_fmt: "2026-03-31".into(),
                net_income: 700e9,
                ..Default::default()
            }],
            income_quarterly: vec![
                IncomeStatementRow {
                    end_date_fmt: "2025-06-30".into(),
                    period_type: "3M".into(),
                    net_income: 162.5791e9,
                    net_income_yahoo_row: Some("Net Income".into()),
                    ..Default::default()
                },
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
            ..Default::default()
        };
        let c = build_canonical(&bundle, &Financials::default());
        assert!((c.latest_yahoo_quarter_pat.unwrap() - 181e9).abs() < 1.0);
        assert_eq!(c.latest_yahoo_quarter_pat_period, "2026-06-30");
        assert_eq!(
            c.latest_yahoo_quarter_pat_source_column,
            "Net Income Common Stockholders"
        );
    }

    #[test]
    fn bank_yahoo_loans_are_not_canonical_advances() {
        let bundle = StatementBundle {
            balance_annual: vec![
                BalanceSheetRow {
                    end_date_fmt: "2025-03-31".into(),
                    net_loans: 28.1e12,
                    net_loans_yahoo_row: Some("Net Loan".into()),
                    ..Default::default()
                },
                BalanceSheetRow {
                    end_date_fmt: "2026-03-31".into(),
                    end_ts: Some(2),
                    net_loans: 30.9938865e12,
                    net_loans_yahoo_row: Some("Net Loan".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let c = build_canonical_for(&bundle, &Financials::default(), FinancialCompanyType::Bank);
        assert!((c.yahoo_loan_book_field.unwrap() - 30.9938865e12).abs() < 1e6);
        assert_eq!(c.yahoo_loan_book_row, "Net Loan");
        assert!(c.yahoo_loan_book_growth_yoy_pct.unwrap() > 9.0);
        assert!(c.canonical_advances.is_none());
        assert!(c.loan_book.is_none());
        assert!(c.loan_book_growth_yoy_pct.is_none());
    }
}
