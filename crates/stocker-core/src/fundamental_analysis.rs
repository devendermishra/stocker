use crate::math::pct_change;
use crate::models::{
    BalanceSheetRow, FinancialCompanyType, Financials, FundamentalAnalysis, FundamentalSection,
    IncomeStatementRow, StatementBundle,
};
use crate::financial_company::{
    NA_FILING, NA_NOT_APPLICABLE, NA_NOT_MEANINGFUL, NA_NOT_PRIMARY, NA_NOT_USED, NA_YAHOO_FILINGS_MAY_EXIST,
};
use crate::statements::{income_annual_desc, sort_owned_desc};

fn line(label: &str, val: Option<f64>, suffix: &str) -> (String, String) {
    match val {
        Some(x) if x.is_finite() => (
            label.to_string(),
            if suffix == "pct" {
                format!("{:.1}%", x)
            } else {
                format!("{:.2}{}", x, suffix)
            },
        ),
        _ => (label.to_string(), "N/A".to_string()),
    }
}

fn fundamental_confidence(inc_len: usize) -> String {
    if inc_len >= 3 {
        "High".to_string()
    } else if inc_len >= 1 {
        "Medium".to_string()
    } else {
        "Low".to_string()
    }
}

fn metric_present(v: Option<f64>) -> usize {
    usize::from(v.filter(|x| x.is_finite()).is_some())
}

fn lender_section_confidence(present: usize, total: usize) -> String {
    if total == 0 || present == 0 {
        return "Low".to_string();
    }
    let pct = present as f64 / total as f64;
    if pct >= 0.75 {
        "High".to_string()
    } else if pct >= 0.4 {
        "Medium".to_string()
    } else {
        "Low".to_string()
    }
}

fn fundamental_growth_section(
    inc: &[&IncomeStatementRow],
    financials: &Financials,
    confidence: &str,
) -> FundamentalSection {
    let rev_yoy = inc
        .first()
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.revenue, b.revenue));
    let ebitda_yoy = inc
        .first()
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.ebitda, b.ebitda));
    let ni_yoy = inc
        .first()
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.net_income, b.net_income));
    let eps_yoy = inc.first().zip(inc.get(1)).and_then(|(a, b)| {
        let e0 = a.diluted_eps?;
        let e1 = b.diluted_eps?;
        pct_change(e0, e1)
    });

    let rev_asc: Vec<f64> = inc.iter().rev().map(|r| r.revenue).collect();
    let yahoo_g = financials.revenue_growth.filter(|x| x.abs() > 1e-9);
    let rev_cagr_3 = crate::series_integrity::trailing_cagr_pct(&rev_asc, 3, yahoo_g).value;
    let rev_cagr_5 = crate::series_integrity::trailing_cagr_pct(&rev_asc, 5, yahoo_g).value;
    let eps_asc: Vec<f64> = inc.iter().rev().filter_map(|r| r.diluted_eps).collect();
    let eps_cagr_3 = crate::series_integrity::trailing_cagr_pct(&eps_asc, 3, None).value;
    let eps_cagr_5 = crate::series_integrity::trailing_cagr_pct(&eps_asc, 5, None).value;

    let rev_disp = rev_yoy.filter(|_| crate::series_integrity::latest_step_usable(&rev_asc, yahoo_g));
    let ni_asc: Vec<f64> = inc.iter().rev().map(|r| r.net_income).collect();
    let ni_disp = ni_yoy.filter(|_| crate::series_integrity::latest_step_usable(&ni_asc, None));
    let q_earn_pct = financials.earnings_growth.map(|g| g * 100.0);

    let g_interp = {
        let r_ok = rev_disp.map(|x| x > 2.0).unwrap_or(false);
        let p_ok = ni_yoy.map(|x| x > 2.0).unwrap_or(false);
        let r_bad = rev_disp.map(|x| x < 0.0).unwrap_or(false);
        let p_bad = ni_yoy.map(|x| x < 0.0).unwrap_or(false);
        let q_down = q_earn_pct.map(|x| x < -5.0).unwrap_or(false);
        if (r_ok || p_ok) && q_down {
            "Medium-term growth remains positive on annual statements, but latest-quarter earnings declined YoY (Yahoo earningsGrowth — distinct from annual PAT)."
        } else if r_ok && p_ok {
            "Annual statement growth: revenue and profit both trending up. Distinct from latest-quarter Yahoo earningsGrowth."
        } else if r_bad && p_bad {
            "Weak growth: annual revenue and profit under pressure."
        } else if (r_ok && !p_ok) || (!r_ok && p_ok) {
            "Mixed growth: annual revenue and profit trends diverge — review drivers."
        } else {
            "Growth assessment limited by sparse or missing history."
        }
        .to_string()
    };

    let mut g_flags = Vec::new();
    if inc.len() < 2 {
        g_flags.push("Fewer than 2 annual income points; YoY may be N/A.".to_string());
    }
    if rev_cagr_3.is_none() && inc.len() >= 4 {
        g_flags.push("3Y revenue CAGR excluded: historical series failed scope/consistency checks.".to_string());
    }

    FundamentalSection {
        title: String::new(),
        interpretation: g_interp,
        flags: g_flags,
        confidence: confidence.to_string(),
        lines: vec![
            line("FY annual revenue growth YoY", rev_disp, "pct"),
            line("Revenue CAGR (3Y, consistent series)", rev_cagr_3, "pct"),
            line("Revenue CAGR (5Y, consistent series)", rev_cagr_5, "pct"),
            line("EBITDA growth YoY (annual)", ebitda_yoy, "pct"),
            line("Net profit growth YoY (annual PAT)", ni_disp, "pct"),
            line("Yahoo current revenueGrowth (not FY)", financials.revenue_growth.map(|g| g * 100.0), "pct"),
            line("Yahoo current earningsGrowth (not FY PAT)", q_earn_pct, "pct"),
            line("EPS growth YoY", eps_yoy, "pct"),
            line("EPS CAGR (3Y)", eps_cagr_3, "pct"),
            line("EPS CAGR (5Y)", eps_cagr_5, "pct"),
        ],
    }
}

fn fundamental_profitability_section(
    latest: Option<&IncomeStatementRow>,
    b0: Option<&BalanceSheetRow>,
    financials: &Financials,
    confidence: &str,
) -> FundamentalSection {
    let gross_m = latest
        .filter(|r| r.revenue > 0.0 && r.gross_profit > 0.0)
        .map(|r| r.gross_profit / r.revenue)
        .filter(|x| *x > 0.0)
        .unwrap_or(financials.gross_margins);
    let op_m = latest
        .filter(|r| r.revenue > 0.0)
        .map(|r| r.operating_income / r.revenue)
        .filter(|x| x.is_finite())
        .unwrap_or(financials.operating_margins);
    let ebitda_m = latest
        .filter(|r| r.revenue > 0.0 && r.ebitda > 0.0)
        .map(|r| r.ebitda / r.revenue)
        .filter(|x| *x > 0.0)
        .unwrap_or(financials.ebitda_margins);
    let net_m = financials.profit_margins;
    let roe_pct = financials.return_on_equity.map(|x| x * 100.0);
    let ebit = latest.map(|r| {
        if r.operating_income.abs() > 1.0 {
            r.operating_income
        } else {
            r.ebit.max(r.ebitda)
        }
    });
    let local_roce = match (ebit, b0) {
        (Some(e), Some(b)) => crate::yahoo_metrics::local_roce(e, b.total_equity, b.total_debt, b.cash),
        _ => None,
    };
    let roce = financials
        .return_on_capital_employed
        .or(local_roce)
        .map(|x| x * 100.0);
    let roa = financials.return_on_assets.map(|x| x * 100.0);

    let mut p_lines = vec![
        line("Gross margin", Some(gross_m * 100.0), "pct"),
        line("EBITDA margin", Some(ebitda_m * 100.0), "pct"),
        line("Operating margin", Some(op_m * 100.0), "pct"),
        line("Net profit margin", Some(net_m * 100.0), "pct"),
        line("ROE", roe_pct, "pct"),
        line("ROCE", roce, "pct"),
    ];
    if let Some(roa) = roa {
        p_lines.push(line("ROA", Some(roa), "pct"));
    }

    let p_interp = if roe_pct.map(|r| r > 15.0).unwrap_or(false) && gross_m > 0.25 {
        "High-quality profitability screen: ROE elevated with healthy gross margin."
    } else if net_m < 0.0 {
        "Poor profitability: negative net margin on latest data."
    } else if roe_pct.map(|r| r < 5.0).unwrap_or(false) {
        "Weak profitability: ROE on the low side."
    } else if roe_pct.is_none() {
        "Profitability mixed; Yahoo ROE unavailable for this name."
    } else {
        "Profitability is mixed vs typical quality screens."
    }
    .to_string();

    FundamentalSection {
        title: String::new(),
        interpretation: p_interp,
        flags: vec![],
        confidence: confidence.to_string(),
        lines: p_lines,
    }
}

fn fundamental_balance_section(
    b0: Option<&BalanceSheetRow>,
    latest: Option<&IncomeStatementRow>,
    financials: &Financials,
    confidence: &str,
) -> FundamentalSection {
    let debt = b0.map(|b| b.total_debt).unwrap_or(financials.total_debt);
    let cce = b0
        .map(|b| {
            if b.cash_and_cash_equivalents.abs() > 1.0 {
                b.cash_and_cash_equivalents
            } else {
                b.cash
            }
        })
        .unwrap_or(financials.total_cash);
    let sti = b0.map(|b| b.short_term_investments).unwrap_or(0.0);
    let liquid = cce + sti;
    let equity = b0.map(|b| b.total_equity).unwrap_or(0.0);
    let liab = b0.map(|b| b.total_liabilities).unwrap_or(0.0);
    let ca = b0.map(|b| b.current_assets).unwrap_or(0.0);
    let cl = b0.map(|b| b.current_liabilities).unwrap_or(0.0);
    let interest = latest
        .map(|r| r.interest_expense)
        .filter(|i| *i > 1e-6)
        .or_else(|| b0.map(|b| b.interest_expense).filter(|i| *i > 1e-6))
        .unwrap_or(0.0);
    let net_vs_cce = debt - cce;
    let net_vs_liquid = debt - liquid;
    let d_eq = if equity.abs() > 1.0 {
        Some(debt / equity)
    } else {
        financials.debt_to_equity
    };
    let current_ratio = if cl > 0.0 { Some(ca / cl) } else { None };
    let ebit_op = latest.map(|r| {
        if r.operating_income.abs() > 1.0 {
            r.operating_income
        } else {
            r.ebitda
        }
    });
    let int_cov = match (ebit_op, interest) {
        (Some(e), i) if i > 1e-6 => Some(e / i),
        _ => None,
    };
    let de_screen = d_eq.or(financials.debt_to_equity);
    let bs_interp = if cce > debt && debt >= 0.0 {
        "Cash and cash equivalents exceed total debt (net cash on a cash-equivalent basis)."
    } else if liquid > debt && debt >= 0.0 {
        "Gross cash and liquid investments exceed debt, but cash-and-equivalents do not — not labelled a net-cash company."
    } else if de_screen.map(|d| d > 1.50).unwrap_or(false)
        || current_ratio.map(|c| c < 1.0).unwrap_or(false)
    {
        "Risky balance sheet screen: high leverage or weak liquidity."
    } else if int_cov.map(|c| c < 2.0).unwrap_or(false) && interest > 0.0 {
        "Interest coverage is tight — monitor refinancing and EBITDA."
    } else {
        "Balance sheet looks serviceable vs heuristic thresholds."
    }
    .to_string();

    FundamentalSection {
        title: String::new(),
        interpretation: bs_interp,
        flags: if b0.is_none() {
            vec!["Balance sheet history sparse; using quote-level debt/cash where possible.".to_string()]
        } else {
            vec![]
        },
        confidence: confidence.to_string(),
        lines: vec![
            ("Total debt".to_string(), format!("{:.0}", debt)),
            ("Cash & cash equivalents".to_string(), format!("{:.0}", cce)),
            ("Short-term investments".to_string(), format!("{:.0}", sti)),
            ("Gross cash + liquid investments".to_string(), format!("{:.0}", liquid)),
            ("Net debt vs cash equivalents (debt − CCE)".to_string(), format!("{:.0}", net_vs_cce)),
            ("Net debt vs liquid (debt − CCE − STI)".to_string(), format!("{:.0}", net_vs_liquid)),
            ("Debt / equity".to_string(), d_eq.map(|d| format!("{:.2}", d)).unwrap_or_else(|| "N/A".to_string())),
            line("Current ratio", current_ratio, "x"),
            line("Interest coverage", int_cov, "x"),
            ("Total liabilities".to_string(), format!("{:.0}", liab)),
            ("Total equity".to_string(), format!("{:.0}", equity)),
        ],
    }
}

fn fundamental_cashflow_section(
    c0: Option<&crate::models::CashflowRow>,
    latest: Option<&IncomeStatementRow>,
    financials: &Financials,
    confidence: &str,
) -> FundamentalSection {
    let cfo = c0
        .map(|c| c.operating_cashflow)
        .or(financials.operating_cashflow)
        .unwrap_or(0.0);
    let capex = c0.map(|c| c.capital_expenditure).unwrap_or(0.0);
    let fcf = c0
        .and_then(crate::yahoo_metrics::row_statement_fcf)
        .or(financials.free_cashflow)
        .unwrap_or(0.0);
    let pat = financials
        .net_income
        .filter(|v| v.abs() > 1.0)
        .or_else(|| latest.map(|r| r.net_income).filter(|v| v.abs() > 1.0));
    let cfo_pat = match pat {
        Some(p) if p.abs() > 1.0 => Some(cfo / p),
        _ => None,
    };
    let fcf_margin = if financials.revenue > 0.0 {
        Some((fcf / financials.revenue) * 100.0)
    } else {
        None
    };
    let fcf_yield = crate::yahoo_metrics::fcf_yield_pct(
        c0.and_then(crate::yahoo_metrics::row_statement_fcf)
            .or(financials.free_cashflow),
        financials.market_cap,
    );
    let payout = financials.payout_ratio * 100.0;
    let cf_interp = match cfo_pat {
        Some(r) if r >= 1.0 && pat.map(|p| p > 0.0).unwrap_or(false) => "Good earnings quality: CFO at or above PAT.",
        Some(r) if r < 0.5 && pat.map(|p| p > 0.0).unwrap_or(false) => {
            "Weak earnings quality: PAT positive but CFO conversion weak."
        }
        _ => "Cash flow quality mixed or data incomplete — verify with filings.",
    }
    .to_string();

    FundamentalSection {
        title: String::new(),
        interpretation: cf_interp,
        flags: vec![],
        confidence: confidence.to_string(),
        lines: vec![
            ("Operating cash flow".to_string(), format!("{:.0}", cfo)),
            ("Capital expenditure".to_string(), format!("{:.0}", capex)),
            ("Free cash flow (CFO − capex)".to_string(), format!("{:.0}", fcf)),
            line("CFO / PAT", cfo_pat, "x"),
            line("FCF margin", fcf_margin, "pct"),
            line("FCF yield", fcf_yield, "pct"),
            line(
                "Dividend payout ratio",
                if financials.payout_ratio > 0.0 {
                    Some(payout)
                } else {
                    None
                },
                "pct",
            ),
        ],
    }
}

fn fundamental_efficiency_section(
    bundle: &StatementBundle,
    inc: &[&IncomeStatementRow],
    b0: Option<&BalanceSheetRow>,
    latest: Option<&IncomeStatementRow>,
    confidence: String,
) -> FundamentalSection {
    let asset_turn = b0
        .zip(latest)
        .filter(|(bal, row)| bal.total_assets > 0.0 && row.revenue > 0.0)
        .map(|(bal, row)| row.revenue / bal.total_assets);
    let inv_days = b0.zip(latest).and_then(|(bal, row)| {
        if row.cost_of_revenue > 0.0 && bal.inventory > 0.0 {
            Some((bal.inventory / row.cost_of_revenue) * 365.0)
        } else {
            None
        }
    });
    let rec_days = b0.zip(latest).and_then(|(bal, row)| {
        if row.revenue > 0.0 && bal.net_receivables > 0.0 {
            Some((bal.net_receivables / row.revenue) * 365.0)
        } else {
            None
        }
    });
    let wc_trend = if inc.len() >= 2 {
        let b_new = bundle
            .balance_annual
            .iter()
            .max_by_key(|x| x.end_ts.unwrap_or(0));
        let b_old = bundle
            .balance_annual
            .iter()
            .filter(|x| x.end_ts.unwrap_or(0) < b_new.and_then(|n| n.end_ts).unwrap_or(0))
            .max_by_key(|x| x.end_ts.unwrap_or(0));
        match (b_new, b_old) {
            (Some(bn), Some(bo)) => {
                let wc_n = bn.current_assets - bn.current_liabilities;
                let wc_o = bo.current_assets - bo.current_liabilities;
                Some(wc_n - wc_o)
            }
            _ => None,
        }
    } else {
        None
    };

    let mut eff_flags = Vec::new();
    let bal_sorted = sort_owned_desc(bundle.balance_annual.clone(), |r| r.end_ts);
    if let (Some(cur), Some(prev)) = (inc.first(), inc.get(1)) {
        let ag = if bal_sorted.len() >= 2 {
            pct_change(bal_sorted[0].net_receivables, bal_sorted[1].net_receivables)
        } else {
            None
        };
        if let (Some(rg), Some(ag)) = (pct_change(cur.revenue, prev.revenue), ag) {
            if ag > rg + 10.0 && bal_sorted[0].net_receivables > 0.0 {
                eff_flags.push(
                    "Receivables growing faster than revenue — watch collection quality."
                        .to_string(),
                );
            }
        }
    }

    FundamentalSection {
        title: String::new(),
        interpretation:
            "Efficiency metrics from latest statements; days ratios need COGS/receivables present."
                .to_string(),
        flags: eff_flags,
        confidence,
        lines: vec![
            line("Asset turnover", asset_turn, "x"),
            line("Inventory days", inv_days, ""),
            line("Receivable days", rec_days, ""),
            line("Working capital Δ (latest vs prior)", wc_trend, ""),
        ],
    }
}

fn na_line(label: &str, why: &str) -> (String, String) {
    (label.to_string(), why.to_string())
}

fn opt_money(label: &str, v: Option<f64>) -> (String, String) {
    match v {
        Some(x) if x.is_finite() => (label.to_string(), format!("{:.0}", x)),
        _ => (label.to_string(), NA_FILING.to_string()),
    }
}

fn lender_growth_section(
    inc: &[&IncomeStatementRow],
    financials: &Financials,
    _confidence: &str,
) -> FundamentalSection {
    let ii_yoy = inc.first().zip(inc.get(1)).and_then(|(a, b)| {
        if a.interest_income.abs() > 1.0 && b.interest_income.abs() > 1.0 {
            pct_change(a.interest_income, b.interest_income)
        } else {
            None
        }
    });
    let nii_yoy = inc
        .first()
        .zip(inc.get(1))
        .and_then(|(a, b)| crate::canonical::preferred_nii_yoy(a, b));
    let nii_def = inc.first().map(|r| {
        if crate::canonical::reported_nii(r).is_some() {
            "Yahoo Net Interest Income statement row"
        } else if crate::canonical::calculated_nii(r).is_some() {
            "calculated (interest income − interest expense)"
        } else {
            "unavailable"
        }
    });
    let ni_yoy = inc
        .first()
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.net_income, b.net_income));
    let ni_asc: Vec<f64> = inc.iter().rev().map(|r| r.net_income).collect();
    let pat_cagr_3 = crate::series_integrity::trailing_cagr_pct(&ni_asc, 3, None).value;
    let q_earn_pct = financials.earnings_growth.map(|g| g * 100.0);
    let interp = if ni_yoy.map(|x| x > 0.0).unwrap_or(false) {
        "Income growth uses interest income, NII, and PAT — not Yahoo totalRevenue (unverified as total income)."
    } else if ni_yoy.map(|x| x < 0.0).unwrap_or(false) {
        "Latest-year PAT declined; still distinct from industrial revenue CAGR."
    } else {
        "Lender income growth from interest income / NII / PAT where available. NII prefers Yahoo netInterestIncome; otherwise II − IE (may differ from company-reported NII)."
    }
    .to_string();
    let present = metric_present(ii_yoy) + metric_present(nii_yoy) + metric_present(ni_yoy) + metric_present(pat_cagr_3);
    let conf = lender_section_confidence(present, 4);
    FundamentalSection {
        title: "Income growth".to_string(),
        interpretation: interp,
        flags: vec![],
        confidence: conf,
        lines: vec![
            line("Interest income growth YoY", ii_yoy, "pct"),
            line("NII growth YoY", nii_yoy, "pct"),
            (
                "NII definition".to_string(),
                nii_def.unwrap_or("unavailable").to_string(),
            ),
            line("PAT growth YoY (annual)", ni_yoy, "pct"),
            line("3Y PAT CAGR", pat_cagr_3, "pct"),
            line("Yahoo current earningsGrowth (not FY PAT)", q_earn_pct, "pct"),
            na_line("Revenue CAGR (industrial)", NA_NOT_MEANINGFUL),
            na_line("Yahoo totalRevenue growth (unverified total income)", NA_NOT_USED),
            na_line("Gross margin", NA_NOT_MEANINGFUL),
            na_line("Operating margin", NA_NOT_MEANINGFUL),
        ],
    }
}

fn lender_profitability_section(
    financials: &Financials,
    bank: Option<&crate::models::BankingMetrics>,
    _confidence: &str,
) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let roe_pct = financials.return_on_equity.map(|x| x * 100.0);
    let roa = financials.return_on_assets.map(|x| x * 100.0);
    let nim_disp = b.nim_pct;
    let spread = b.spread_pct.or_else(|| match (b.yield_on_assets_pct, b.cost_of_funds_pct) {
        (Some(y), Some(c)) => Some(y - c),
        _ => None,
    });
    let interp = match (nim_disp, spread) {
        (Some(n), _) if n > 0.1 => {
            if b.nim_pct.unwrap_or(0.0) >= 3.0 {
                "Lending economics: NIM present. Expanding/stable/compressing needs a multi-year series from filings."
            } else {
                "NIM available from filings overlay — interpret vs cost of funds, not EBITDA margin."
            }
        }
        _ => "NIM/spread/cost-of-funds are not in Yahoo. Do not use EBITDA margin or ROCE for this lender.",
    }
    .to_string();
    let present = metric_present(nim_disp)
        + metric_present(spread)
        + metric_present(b.cost_of_funds_pct)
        + metric_present(b.yield_on_assets_pct)
        + metric_present(roe_pct)
        + metric_present(roa);
    let conf = lender_section_confidence(present, 6);
    FundamentalSection {
        title: "Lending economics".to_string(),
        interpretation: interp,
        flags: vec![],
        confidence: conf,
        lines: vec![
            line("Yield on loan assets", b.yield_on_assets_pct, "pct"),
            line("Cost of funds", b.cost_of_funds_pct, "pct"),
            line("Spread", spread, "pct"),
            line("NIM", nim_disp, "pct"),
            line("Borrowing cost trend", b.incremental_borrowing_cost_pct, "pct"),
            line("ROE", roe_pct, "pct"),
            line("ROA", roa, "pct"),
            na_line("EBITDA margin", NA_NOT_MEANINGFUL),
            na_line("ROCE", NA_NOT_MEANINGFUL),
        ],
    }
}

fn lender_capital_section(
    b0: Option<&BalanceSheetRow>,
    financials: &Financials,
    bank: Option<&crate::models::BankingMetrics>,
    _confidence: &str,
) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let equity = b0.map(|x| x.total_equity).unwrap_or(0.0);
    let debt = b0.map(|x| x.total_debt).unwrap_or(financials.total_debt);
    let gearing = b.gearing.or_else(|| {
        if equity.abs() > 1.0 {
            Some(debt / equity)
        } else {
            financials.debt_to_equity
        }
    });
    let net_worth = b.net_worth.or_else(|| (equity.abs() > 1.0).then_some(equity));
    let rating_present = b
        .credit_rating
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let present = metric_present(b.crar_pct)
        + metric_present(b.tier1_pct)
        + usize::from(rating_present)
        + metric_present(b.alm_lcr_pct);
    let conf = lender_section_confidence(present, 4);
    FundamentalSection {
        title: "Capital & solvency".to_string(),
        interpretation: "Leverage for a lender is judged against capital adequacy and asset quality, not industrial D/E < 0.5x. Yahoo typically lacks CRAR/Tier-I.".to_string(),
        flags: vec![],
        confidence: conf,
        lines: vec![
            line("CRAR", b.crar_pct, "pct"),
            line("Tier-I capital ratio", b.tier1_pct, "pct"),
            opt_money("Net worth", net_worth),
            (
                "Gearing / leverage".to_string(),
                gearing.map(|g| format!("{:.2}x", g)).unwrap_or_else(|| NA_FILING.to_string()),
            ),
            (
                "Credit rating".to_string(),
                b.credit_rating.unwrap_or_else(|| NA_FILING.to_string()),
            ),
            line("ALM / liquidity coverage", b.alm_lcr_pct, "pct"),
            na_line("Current ratio", NA_NOT_PRIMARY),
            na_line("Interest coverage", NA_NOT_MEANINGFUL),
            na_line("Industrial D/E screen (<0.5x)", NA_NOT_USED),
        ],
    }
}

fn bank_funding_section(
    bundle: &StatementBundle,
    canonical: Option<&crate::models::CanonicalMetrics>,
    bank: Option<&crate::models::BankingMetrics>,
) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let c = canonical.cloned().unwrap_or_default();
    let bs = latest_bank_balance(bundle);
    let deposits = bs.and_then(|r| nonzero_f(r.total_deposits));
    let verified = c.canonical_advances.or(b.loan_book);
    let ldr = match (verified, deposits) {
        (Some(a), Some(d)) if d.abs() > 1.0 => Some(a / d),
        _ => None,
    };
    let present = metric_present(verified)
        + metric_present(b.credit_growth_yoy_pct)
        + metric_present(deposits)
        + metric_present(b.deposit_growth_yoy_pct)
        + metric_present(b.casa_ratio_pct)
        + metric_present(ldr)
        + metric_present(b.cost_of_funds_pct)
        + metric_present(b.alm_lcr_pct);
    let conf = lender_section_confidence(present, 8);
    FundamentalSection {
        title: "Funding and balance-sheet growth".to_string(),
        interpretation: "Yahoo loans/receivables are shown with row provenance and are not treated as HDFC-style gross advances. CASA, official credit growth, LDR and LCR belong in filings when Yahoo is blank.".to_string(),
        flags: vec![],
        confidence: conf,
        lines: vec![
            match c.yahoo_loan_book_field {
                Some(x) if x.is_finite() => (
                    "Yahoo loans/receivables (unverified vs gross advances)".to_string(),
                    format!("{:.0}", x),
                ),
                _ => (
                    "Yahoo loans/receivables (unverified vs gross advances)".to_string(),
                    NA_YAHOO_FILINGS_MAY_EXIST.to_string(),
                ),
            },
            (
                "Yahoo loan row".to_string(),
                if c.yahoo_loan_book_row.is_empty() {
                    NA_YAHOO_FILINGS_MAY_EXIST.to_string()
                } else {
                    c.yahoo_loan_book_row.clone()
                },
            ),
            line("Yahoo loans YoY (not official credit growth)", c.yahoo_loan_book_growth_yoy_pct, "pct"),
            match verified {
                Some(x) if x.is_finite() => ("Canonical advances (gross)".to_string(), format!("{:.0}", x)),
                _ => (
                    "Canonical advances (gross)".to_string(),
                    "N/A — Yahoo row not verified as company-reported gross advances".to_string(),
                ),
            },
            line("Credit growth (filings)", b.credit_growth_yoy_pct, "pct"),
            match deposits {
                Some(x) if x.is_finite() => ("Deposits".to_string(), format!("{:.0}", x)),
                _ => ("Deposits".to_string(), NA_YAHOO_FILINGS_MAY_EXIST.to_string()),
            },
            line("Deposit growth", b.deposit_growth_yoy_pct, "pct"),
            line("CASA", b.casa_ratio_pct, "pct"),
            match ldr {
                Some(x) if x.is_finite() => ("Loan/deposit ratio".to_string(), format!("{:.2}x", x)),
                _ => ("Loan/deposit ratio".to_string(), NA_FILING.to_string()),
            },
            line("Cost of deposits", b.cost_of_funds_pct, "pct"),
            na_line("Wholesale funding share", NA_FILING),
            line("LCR", b.alm_lcr_pct, "pct"),
            na_line("CFO/PAT", NA_NOT_USED),
            na_line("FCF yield", NA_NOT_USED),
        ],
    }
}

fn bank_portfolio_efficiency_section(bank: Option<&crate::models::BankingMetrics>) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let present = metric_present(b.nim_pct);
    let conf = lender_section_confidence(present, 9);
    FundamentalSection {
        title: "Portfolio mix & operating efficiency".to_string(),
        interpretation: "Retail vs wholesale mix, unsecured share, cost-to-income and fee income are filing metrics. Yahoo typically does not provide them; missing values are N/A, not zeros.".to_string(),
        flags: vec![],
        confidence: conf,
        lines: vec![
            na_line("Retail loans %", NA_FILING),
            na_line("Corporate/wholesale %", NA_FILING),
            na_line("Unsecured retail %", NA_FILING),
            na_line("Mortgage %", NA_FILING),
            na_line("Credit cards / personal loans", NA_FILING),
            na_line("Sector concentration", NA_FILING),
            na_line("Cost-to-income", NA_FILING),
            na_line("Branch productivity", NA_FILING),
            na_line("Fee income share", NA_FILING),
        ],
    }
}

fn latest_bank_balance(bundle: &StatementBundle) -> Option<&BalanceSheetRow> {
    bundle
        .balance_quarterly
        .iter()
        .chain(bundle.balance_annual.iter())
        .max_by_key(|r| crate::statements::parse_period_end_date(&r.end_date_fmt))
}

fn nonzero_f(v: f64) -> Option<f64> {
    if v.is_finite() && v.abs() > 1e-9 {
        Some(v)
    } else {
        None
    }
}

fn lender_loan_book_section(
    canonical_loan: Option<f64>,
    canonical_g: Option<f64>,
    bank: Option<&crate::models::BankingMetrics>,
    _confidence: &str,
) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let present = metric_present(b.loan_book.or(canonical_loan))
        + metric_present(b.loan_book_growth_yoy_pct.or(canonical_g))
        + metric_present(b.disbursements)
        + metric_present(b.sanctions);
    let conf = lender_section_confidence(present, 4);
    FundamentalSection {
        title: "Balance-sheet growth".to_string(),
        interpretation: "Loan-book and disbursement economics replace CFO/FCF conversion. Ordinary CFO/PAT is not used for lenders. Missing Yahoo loan-book figures are often in annual reports.".to_string(),
        flags: vec![],
        confidence: conf,
        lines: vec![
            match b.loan_book.or(canonical_loan) {
                Some(x) if x.is_finite() => ("Loan book".to_string(), format!("{:.0}", x)),
                _ => ("Loan book".to_string(), NA_YAHOO_FILINGS_MAY_EXIST.to_string()),
            },
            match b.loan_book_growth_yoy_pct.or(canonical_g) {
                Some(x) if x.is_finite() => ("Loan book growth YoY".to_string(), format!("{:.1}%", x)),
                _ => ("Loan book growth YoY".to_string(), NA_YAHOO_FILINGS_MAY_EXIST.to_string()),
            },
            opt_money("Sanctions", b.sanctions),
            opt_money("Disbursements", b.disbursements),
            opt_money("Repayments", b.repayments),
            line("Disbursement growth", b.disbursement_growth_yoy_pct, "pct"),
            opt_money("Renewable-energy loan book", b.renewable_loan_book),
            opt_money("Infrastructure loan book", b.infrastructure_loan_book),
            line("Private-sector exposure", b.private_sector_pct, "pct"),
            line("State-sector exposure", b.state_sector_pct, "pct"),
            na_line("CFO/PAT", NA_NOT_USED),
            na_line("FCF yield", NA_NOT_USED),
        ],
    }
}

fn lender_portfolio_section(
    company_type: FinancialCompanyType,
    bank: Option<&crate::models::BankingMetrics>,
    _confidence: &str,
) -> FundamentalSection {
    let b = bank.cloned().unwrap_or_default();
    let extra = if company_type == FinancialCompanyType::NbfcProjectFinance {
        "For project-finance NBFCs, sector mix (generation/T&D/renewables/state vs private) matters more than inventory or receivable days."
    } else {
        "Portfolio mix and borrower concentration belong in filings; Yahoo does not provide GNPA-quality composition."
    };
    let present = metric_present(b.renewable_loan_book)
        + metric_present(b.infrastructure_loan_book)
        + metric_present(b.state_sector_pct)
        + metric_present(b.private_sector_pct);
    let conf = lender_section_confidence(present, 4);
    FundamentalSection {
        title: "Portfolio composition".to_string(),
        interpretation: extra.to_string(),
        flags: vec![],
        confidence: conf,
        lines: vec![
            na_line("Generation / T&D / distribution mix", NA_FILING),
            opt_money("Renewable energy book", b.renewable_loan_book),
            opt_money("Infrastructure/logistics book", b.infrastructure_loan_book),
            line("State government entities", b.state_sector_pct, "pct"),
            line("Private borrowers", b.private_sector_pct, "pct"),
            na_line("Top sector concentration", NA_FILING),
            na_line("Top borrower concentration", NA_FILING),
            na_line("Inventory days", NA_NOT_APPLICABLE),
            na_line("Receivable days", NA_NOT_APPLICABLE),
            na_line("Asset turnover", NA_NOT_MEANINGFUL),
        ],
    }
}

pub fn build_fundamental_analysis(
    bundle: &StatementBundle,
    financials: &Financials,
) -> FundamentalAnalysis {
    build_fundamental_analysis_for(bundle, financials, FinancialCompanyType::Industrial, None, None)
}

pub fn build_fundamental_analysis_for(
    bundle: &StatementBundle,
    financials: &Financials,
    company_type: FinancialCompanyType,
    bank: Option<&crate::models::BankingMetrics>,
    canonical: Option<&crate::models::CanonicalMetrics>,
) -> FundamentalAnalysis {
    let inc = income_annual_desc(bundle);
    let confidence = fundamental_confidence(inc.len());
    let latest = inc.first().copied();

    let bal = sort_owned_desc(bundle.balance_annual.clone(), |r| r.end_ts);
    let b0 = bal.first();
    let cf = sort_owned_desc(bundle.cashflow_annual.clone(), |r| r.end_ts);
    let c0 = cf.first();

    if company_type.is_bank() {
        return FundamentalAnalysis {
            growth: lender_growth_section(&inc, financials, &confidence),
            profitability: lender_profitability_section(financials, bank, &confidence),
            balance_sheet: lender_capital_section(b0, financials, bank, &confidence),
            cash_flow: bank_funding_section(bundle, canonical, bank),
            efficiency: bank_portfolio_efficiency_section(bank),
        };
    }

    if company_type.is_lender() {
        return FundamentalAnalysis {
            growth: lender_growth_section(&inc, financials, &confidence),
            profitability: lender_profitability_section(financials, bank, &confidence),
            balance_sheet: lender_capital_section(b0, financials, bank, &confidence),
            cash_flow: lender_loan_book_section(
                canonical.and_then(|c| c.loan_book),
                canonical.and_then(|c| c.loan_book_growth_yoy_pct),
                bank,
                &confidence,
            ),
            efficiency: lender_portfolio_section(company_type, bank, &confidence),
        };
    }

    FundamentalAnalysis {
        growth: fundamental_growth_section(&inc, financials, &confidence),
        profitability: fundamental_profitability_section(latest, b0, financials, &confidence),
        balance_sheet: fundamental_balance_section(b0, latest, financials, &confidence),
        cash_flow: fundamental_cashflow_section(c0, latest, financials, &confidence),
        efficiency: fundamental_efficiency_section(bundle, &inc, b0, latest, confidence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::IncomeStatementRow;

    #[test]
    fn lender_section_confidence_is_low_when_filing_metrics_missing() {
        let bundle = StatementBundle {
            income_annual: vec![
                IncomeStatementRow {
                    interest_income: 100.0,
                    interest_expense: 40.0,
                    net_interest_income: 60.0,
                    net_income: 20.0,
                    ..Default::default()
                },
                IncomeStatementRow {
                    interest_income: 80.0,
                    interest_expense: 35.0,
                    net_interest_income: 45.0,
                    net_income: 15.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let fa = build_fundamental_analysis_for(
            &bundle,
            &Financials::default(),
            FinancialCompanyType::NbfcProjectFinance,
            None,
            None,
        );
        assert_eq!(fa.balance_sheet.confidence, "Low");
        assert_eq!(fa.profitability.confidence, "Low");
        assert_eq!(fa.cash_flow.confidence, "Low");
        assert_eq!(fa.efficiency.confidence, "Low");
        assert_ne!(fa.growth.confidence, "Low");
    }

    #[test]
    fn bank_fundamentals_use_funding_not_project_finance() {
        let fa = build_fundamental_analysis_for(
            &StatementBundle::default(),
            &Financials::default(),
            FinancialCompanyType::Bank,
            None,
            None,
        );
        assert_eq!(fa.cash_flow.title, "Funding and balance-sheet growth");
        assert!(fa.cash_flow.lines.iter().any(|(k, _)| k.contains("Yahoo loans/receivables")));
        assert!(fa.cash_flow.lines.iter().any(|(k, v)| k.contains("Canonical advances") && v.contains("not verified")));
        assert!(!fa.cash_flow.lines.iter().any(|(k, _)| k == "Advances"));
        assert!(!fa.cash_flow.lines.iter().any(|(k, _)| k.contains("Disbursement") || k.contains("Renewable")));
        assert_eq!(fa.efficiency.title, "Portfolio mix & operating efficiency");
        assert!(fa.efficiency.lines.iter().any(|(k, _)| k == "Retail loans %"));
        assert!(!fa.efficiency.lines.iter().any(|(k, _)| k.contains("Renewable") || k.contains("Generation")));
    }
}
