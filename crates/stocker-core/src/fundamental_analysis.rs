use crate::models::{
    AnnualReport, Financials, FundamentalAnalysis, FundamentalSection, StatementBundle,
};

fn pct_change(cur: f64, prev: f64) -> Option<f64> {
    if !prev.is_finite() || prev.abs() < 1e-9 {
        return None;
    }
    Some(((cur / prev) - 1.0) * 100.0)
}

fn cagr(start: f64, end: f64, years: f64) -> Option<f64> {
    if years <= 0.0 || start <= 0.0 || end <= 0.0 {
        return None;
    }
    Some(((end / start).powf(1.0 / years) - 1.0) * 100.0)
}

fn sort_income_annual(bundle: &StatementBundle) -> Vec<&crate::models::IncomeStatementRow> {
    let mut v: Vec<_> = bundle.income_annual.iter().collect();
    v.sort_by(|a, b| {
        let ta = a.end_ts.unwrap_or(0);
        let tb = b.end_ts.unwrap_or(0);
        tb.cmp(&ta)
    });
    v
}

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

pub fn build_fundamental_analysis(
    bundle: &StatementBundle,
    financials: &Financials,
    _annual_reports: &[AnnualReport],
) -> FundamentalAnalysis {
    let inc = sort_income_annual(bundle);
    let conf = if inc.len() >= 3 {
        "High"
    } else if inc.len() >= 1 {
        "Medium"
    } else {
        "Low"
    }
    .to_string();

    // Growth
    let rev_yoy = inc
        .get(0)
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.revenue, b.revenue));
    let ebitda_yoy = inc
        .get(0)
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.ebitda, b.ebitda));
    let ni_yoy = inc
        .get(0)
        .zip(inc.get(1))
        .and_then(|(a, b)| pct_change(a.net_income, b.net_income));
    let eps_yoy = inc.get(0).zip(inc.get(1)).and_then(|(a, b)| {
        let e0 = a.diluted_eps?;
        let e1 = b.diluted_eps?;
        pct_change(e0, e1)
    });

    let (rev_cagr_3, eps_cagr_3) = if inc.len() >= 4 {
        let a = inc[0];
        let b = inc[3];
        (
            cagr(b.revenue, a.revenue, 3.0),
            match (a.diluted_eps, b.diluted_eps) {
                (Some(e0), Some(e1)) if e1 > 0.0 && e0 > 0.0 => cagr(e1, e0, 3.0),
                _ => None,
            },
        )
    } else {
        (None, None)
    };

    let (rev_cagr_5, eps_cagr_5) = if inc.len() >= 6 {
        let a = inc[0];
        let b = inc[5];
        (
            cagr(b.revenue, a.revenue, 5.0),
            match (a.diluted_eps, b.diluted_eps) {
                (Some(e0), Some(e1)) if e1 > 0.0 && e0 > 0.0 => cagr(e1, e0, 5.0),
                _ => None,
            },
        )
    } else {
        (None, None)
    };

    let rev_yahoo = financials.revenue_growth * 100.0;
    let rev_disp = if rev_yoy.is_none() && financials.revenue_growth != 0.0 {
        Some(rev_yahoo)
    } else {
        rev_yoy
    };

    let earn_yahoo = financials.earnings_growth * 100.0;
    let ni_disp = if ni_yoy.is_none() && financials.earnings_growth != 0.0 {
        Some(earn_yahoo)
    } else {
        ni_yoy
    };

    let g_lines = vec![
        line("Revenue growth YoY", rev_disp, "pct"),
        line("Revenue CAGR (3Y)", rev_cagr_3, "pct"),
        line("Revenue CAGR (5Y)", rev_cagr_5, "pct"),
        line("EBITDA growth YoY", ebitda_yoy, "pct"),
        line("Net profit growth YoY", ni_disp, "pct"),
        line("EPS growth YoY", eps_yoy, "pct"),
        line("EPS CAGR (3Y)", eps_cagr_3, "pct"),
        line("EPS CAGR (5Y)", eps_cagr_5, "pct"),
    ];

    let g_interp = {
        let r_ok = rev_disp.map(|x| x > 2.0).unwrap_or(false);
        let p_ok = ni_disp.map(|x| x > 2.0).unwrap_or(false);
        let r_bad = rev_disp.map(|x| x < 0.0).unwrap_or(false);
        let p_bad = ni_disp.map(|x| x < 0.0).unwrap_or(false);
        if r_ok && p_ok {
            "Strong growth: revenue and profit both trending up (statement/Yahoo mix)."
        } else if r_bad && p_bad {
            "Weak growth: revenue and profit under pressure."
        } else if (r_ok && !p_ok) || (!r_ok && p_ok) {
            "Mixed growth: revenue and profit trends diverge — review drivers."
        } else {
            "Growth assessment limited by sparse or missing history."
        }
        .to_string()
    };
    let mut g_flags = Vec::new();
    if inc.len() < 2 {
        g_flags.push("Fewer than 2 annual income points; YoY may be N/A.".to_string());
    }

    let growth = FundamentalSection {
        interpretation: g_interp,
        flags: g_flags,
        confidence: conf.clone(),
        lines: g_lines,
    };

    // Profitability — prefer latest annual margins, fallback financials
    let latest = inc.first().copied();
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

    let roe = financials.return_on_equity * 100.0;
    let roce = financials
        .return_on_capital_employed
        .map(|x| x * 100.0)
        .unwrap_or(f64::NAN);
    let roa = financials
        .return_on_assets
        .map(|x| x * 100.0)
        .unwrap_or(f64::NAN);

    let mut p_lines = vec![
        line("Gross margin", Some(gross_m * 100.0), "pct"),
        line("EBITDA margin", Some(ebitda_m * 100.0), "pct"),
        line("Operating margin", Some(op_m * 100.0), "pct"),
        line("Net profit margin", Some(net_m * 100.0), "pct"),
        line("ROE", Some(roe), "pct"),
    ];
    if roce.is_finite() {
        p_lines.push(line("ROCE", Some(roce), "pct"));
    } else {
        p_lines.push(("ROCE".to_string(), "N/A".to_string()));
    }
    if roa.is_finite() {
        p_lines.push(line("ROA", Some(roa), "pct"));
    }

    let p_interp = if roe > 15.0 && gross_m > 0.25 {
        "High-quality profitability screen: ROE elevated with healthy gross margin."
    } else if net_m < 0.0 {
        "Poor profitability: negative net margin on latest data."
    } else if roe < 0.05 && roe > -100.0 {
        "Weak profitability: ROE on the low side."
    } else {
        "Profitability is mixed vs typical quality screens."
    }
    .to_string();

    let profitability = FundamentalSection {
        interpretation: p_interp,
        flags: vec![],
        confidence: conf.clone(),
        lines: p_lines,
    };

    // Balance sheet — latest annual balance
    let mut bal = bundle.balance_annual.clone();
    bal.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    let b0 = bal.first();

    let debt = b0.map(|b| b.total_debt).unwrap_or(financials.total_debt);
    let cash = b0.map(|b| b.cash).unwrap_or(financials.total_cash);
    let equity = b0.map(|b| b.total_equity).unwrap_or(0.0);
    let liab = b0.map(|b| b.total_liabilities).unwrap_or(0.0);
    let ca = b0.map(|b| b.current_assets).unwrap_or(0.0);
    let cl = b0.map(|b| b.current_liabilities).unwrap_or(0.0);
    let interest = b0.map(|b| b.interest_expense).unwrap_or(0.0);

    let net_debt = debt - cash;
    let d_eq = if equity.abs() > 1.0 {
        debt / equity
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

    let bs_lines = vec![
        ("Total debt".to_string(), format!("{:.0}", debt)),
        ("Cash & equivalents".to_string(), format!("{:.0}", cash)),
        ("Net debt".to_string(), format!("{:.0}", net_debt)),
        ("Debt / equity".to_string(), format!("{:.2}", d_eq)),
        line("Current ratio", current_ratio, "x"),
        line("Interest coverage", int_cov, "x"),
        ("Total liabilities".to_string(), format!("{:.0}", liab)),
        ("Total equity".to_string(), format!("{:.0}", equity)),
    ];

    let de_screen = financials.debt_to_equity.max(d_eq);
    let bs_interp = if net_debt < 0.0 && cash > debt {
        "Net cash company: cash exceeds total debt on latest balance snapshot."
    } else if de_screen > 1.50 || current_ratio.map(|c| c < 1.0).unwrap_or(false) {
        "Risky balance sheet screen: high leverage or weak liquidity."
    } else if int_cov.map(|c| c < 2.0).unwrap_or(false) && interest > 0.0 {
        "Interest coverage is tight — monitor refinancing and EBITDA."
    } else {
        "Balance sheet looks serviceable vs heuristic thresholds."
    }
    .to_string();

    let balance_sheet = FundamentalSection {
        interpretation: bs_interp,
        flags: if b0.is_none() {
            vec!["Balance sheet history sparse; using quote-level debt/cash where possible.".to_string()]
        } else {
            vec![]
        },
        confidence: conf.clone(),
        lines: bs_lines,
    };

    // Cash flow — latest annual
    let mut cf = bundle.cashflow_annual.clone();
    cf.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    let c0 = cf.first();
    let cfo = c0.map(|c| c.operating_cashflow).unwrap_or(financials.operating_cashflow);
    let capex = c0.map(|c| c.capital_expenditure).unwrap_or(0.0);
    let fcf = c0.map(|c| c.free_cashflow).unwrap_or(financials.free_cashflow);
    let pat = financials.net_income.max(latest.map(|r| r.net_income).unwrap_or(0.0));
    let cfo_pat = if pat.abs() > 1.0 { Some(cfo / pat) } else { None };
    let fcf_margin = if financials.revenue > 0.0 {
        Some((fcf / financials.revenue) * 100.0)
    } else {
        None
    };
    let fcf_yield = if financials.market_cap > 0.0 {
        Some((fcf / financials.market_cap) * 100.0)
    } else {
        None
    };
    let payout = financials.payout_ratio * 100.0;

    let cf_lines = vec![
        ("Operating cash flow".to_string(), format!("{:.0}", cfo)),
        ("Capital expenditure".to_string(), format!("{:.0}", capex)),
        ("Free cash flow".to_string(), format!("{:.0}", fcf)),
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
    ];

    let cf_interp = match cfo_pat {
        Some(r) if r >= 1.0 && pat > 0.0 => "Good earnings quality: CFO at or above PAT.",
        Some(r) if r < 0.5 && pat > 0.0 => "Weak earnings quality: PAT positive but CFO conversion weak.",
        _ => "Cash flow quality mixed or data incomplete — verify with filings.",
    }
    .to_string();

    let cash_flow = FundamentalSection {
        interpretation: cf_interp,
        flags: vec![],
        confidence: conf.clone(),
        lines: cf_lines,
    };

    // Efficiency
    let asset_turn = b0
        .zip(latest)
        .filter(|(bal, inc)| bal.total_assets > 0.0 && inc.revenue > 0.0)
        .map(|(bal, inc)| inc.revenue / bal.total_assets);

    let inv_days = b0.zip(latest).and_then(|(bal, inc)| {
        if inc.cost_of_revenue > 0.0 && bal.inventory > 0.0 {
            Some((bal.inventory / inc.cost_of_revenue) * 365.0)
        } else {
            None
        }
    });
    let rec_days = b0.zip(latest).and_then(|(bal, inc)| {
        if inc.revenue > 0.0 && bal.net_receivables > 0.0 {
            Some((bal.net_receivables / inc.revenue) * 365.0)
        } else {
            None
        }
    });

    let wc_trend = if inc.len() >= 2 {
        let b_new = bundle.balance_annual.iter().max_by_key(|x| x.end_ts.unwrap_or(0));
        let b_old = bundle.balance_annual.iter().filter(|x| {
            x.end_ts.unwrap_or(0) < b_new.and_then(|n| n.end_ts).unwrap_or(0)
        }).max_by_key(|x| x.end_ts.unwrap_or(0));
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

    let eff_lines = vec![
        line("Asset turnover", asset_turn, "x"),
        line("Inventory days", inv_days, ""),
        line("Receivable days", rec_days, ""),
        line("Working capital Δ (latest vs prior)", wc_trend, ""),
    ];

    let mut eff_flags = Vec::new();
    let mut bal_sorted = bundle.balance_annual.clone();
    bal_sorted.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    if let (Some(cur), Some(prev)) = (inc.get(0), inc.get(1)) {
        let ag = if bal_sorted.len() >= 2 {
            pct_change(bal_sorted[0].net_receivables, bal_sorted[1].net_receivables)
        } else {
            None
        };
        if let (Some(rg), Some(ag)) = (pct_change(cur.revenue, prev.revenue), ag) {
            if ag > rg + 10.0 && bal_sorted[0].net_receivables > 0.0 {
                eff_flags.push(
                    "Receivables growing faster than revenue — watch collection quality.".to_string(),
                );
            }
        }
    }

    let eff_interp = "Efficiency metrics from latest statements; days ratios need COGS/receivables present."
        .to_string();

    let efficiency = FundamentalSection {
        interpretation: eff_interp,
        flags: eff_flags,
        confidence: conf,
        lines: eff_lines,
    };

    // Suppress broken balance_sheet lines mapping - I mixed line() with tuples. Fix balance_sheet construction

    FundamentalAnalysis {
        growth,
        profitability,
        balance_sheet,
        cash_flow,
        efficiency,
    }
}
