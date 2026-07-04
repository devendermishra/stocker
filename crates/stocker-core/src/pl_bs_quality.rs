//! P&L and balance-sheet quality analysis when CFO/PAT diverges from reported earnings.

use crate::math::pct_change;
use crate::models::{PlBsQualityAnalysis, PlBsQualityLine, StatementBundle};

fn push_line(lines: &mut Vec<PlBsQualityLine>, area: &str, finding: String, severity: &str) {
    lines.push(PlBsQualityLine {
        area: area.to_string(),
        finding,
        severity: severity.to_string(),
    });
}

/// Analyze P&L and balance sheet when cumulative CFO/PAT (3Y) is unusually high or low.
pub fn build_pl_bs_quality_analysis(
    bundle: &StatementBundle,
    cfo_pat_3y: Option<f64>,
) -> PlBsQualityAnalysis {
    let mut lines = Vec::new();
    let ratio = match cfo_pat_3y {
        Some(r) if r.is_finite() => r,
        _ => {
            return PlBsQualityAnalysis {
                trigger: "Insufficient CFO/PAT data".to_string(),
                summary: "Cannot run P&L / balance sheet quality deep-dive without 3-year CFO and PAT.".to_string(),
                lines,
            };
        }
    };

    let trigger = if ratio > 1.25 {
        format!(
            "Cumulative CFO/PAT (3Y) is {:.2}x — cash generation materially exceeds reported profit.",
            ratio
        )
    } else if ratio < 0.85 {
        format!(
            "Cumulative CFO/PAT (3Y) is {:.2}x — cash trails reported profit.",
            ratio
        )
    } else {
        return PlBsQualityAnalysis {
            trigger: format!("CFO/PAT (3Y) {:.2}x is within a normal band.", ratio),
            summary: "No deep-dive triggered; cash conversion is broadly aligned with earnings.".to_string(),
            lines,
        };
    };

    let mut inc = bundle.income_annual.clone();
    let mut cf = bundle.cashflow_annual.clone();
    let mut bal = bundle.balance_annual.clone();
    inc.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    cf.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    bal.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));

    if ratio > 1.25 {
        push_line(
            &mut lines,
            "P&L",
            "High CFO vs PAT may reflect heavy non-cash charges (depreciation/amortization) \
             inflating operating cash above accounting profit — verify if charges are recurring or one-off."
                .to_string(),
            if ratio > 1.75 { "High" } else { "Medium" },
        );
        if let Some(cur) = inc.first() {
            if cur.depreciation > 0.0 && cur.net_income > 0.0 {
                let dep_ratio = cur.depreciation / cur.net_income;
                if dep_ratio > 0.5 {
                    push_line(
                        &mut lines,
                        "P&L",
                        format!(
                            "Depreciation {:.0} is {:.0}% of PAT — large add-back drives CFO above earnings.",
                            cur.depreciation,
                            dep_ratio * 100.0
                        ),
                        "High",
                    );
                }
            }
            if cur.net_income > 0.0 && cur.operating_income > 0.0 {
                let margin = cur.net_income / cur.revenue.max(1.0);
                if margin < 0.06 {
                    push_line(
                        &mut lines,
                        "P&L",
                        format!(
                            "Low net margin {:.1}% despite high CFO/PAT — profits may be understated or cash includes working-capital release.",
                            margin * 100.0
                        ),
                        "Medium",
                    );
                }
            }
        }
    } else {
        push_line(
            &mut lines,
            "P&L",
            "CFO trailing PAT suggests accrual-heavy earnings — scrutinize revenue recognition and one-off gains."
                .to_string(),
            "High",
        );
    }

    if let (Some(cur_i), Some(prev_i)) = (inc.get(0), inc.get(1)) {
        if let (Some(rg), Some(pg)) = (
            pct_change(cur_i.revenue, prev_i.revenue),
            pct_change(cur_i.net_income, prev_i.net_income),
        ) {
            if pg > rg + 15.0 {
                push_line(
                    &mut lines,
                    "P&L",
                    format!(
                        "PAT grew {:.1}% vs revenue {:.1}% — margin expansion may be accounting-driven; reconcile with cash.",
                        pg, rg
                    ),
                    "Medium",
                );
            }
        }
    }

    if let (Some(cur_b), Some(prev_b)) = (bal.get(0), bal.get(1)) {
        if let (Some(cur_i), Some(prev_i)) = (inc.get(0), inc.get(1)) {
            let rg = pct_change(cur_i.revenue, prev_i.revenue);
            if let (Some(rg), Some(rec_g)) = (
                rg,
                pct_change(cur_b.net_receivables, prev_b.net_receivables),
            ) {
                if rec_g > rg + 8.0 {
                    push_line(
                        &mut lines,
                        "Balance sheet",
                        format!(
                            "Receivables grew {:.1}% vs revenue {:.1}% — collections may lag; high CFO/PAT could reverse if debtors normalize.",
                            rec_g, rg
                        ),
                        "High",
                    );
                }
            }
            if let (Some(rg), Some(inv_g)) = (rg, pct_change(cur_b.inventory, prev_b.inventory)) {
                if inv_g > rg + 8.0 {
                    push_line(
                        &mut lines,
                        "Balance sheet",
                        format!(
                            "Inventory grew {:.1}% vs revenue {:.1}% — working capital may have released cash temporarily.",
                            inv_g, rg
                        ),
                        "Medium",
                    );
                }
            }
        }
        if cur_b.goodwill > 0.0 && cur_b.total_assets > 0.0 {
            let gw_pct = cur_b.goodwill / cur_b.total_assets * 100.0;
            if gw_pct > 15.0 {
                push_line(
                    &mut lines,
                    "Balance sheet",
                    format!(
                        "Goodwill is {:.1}% of assets — impairment risk can distort future PAT vs CFO.",
                        gw_pct
                    ),
                    "Medium",
                );
            }
        }
        if cur_b.total_debt > prev_b.total_debt * 1.15 && ratio > 1.25 {
            push_line(
                &mut lines,
                "Balance sheet",
                "Debt increased while CFO exceeds PAT — check if financing cash flows explain the gap.".to_string(),
                "Medium",
            );
        }
    }

    if let (Some(c0), Some(c1), Some(c2)) = (cf.get(0), cf.get(1), cf.get(2)) {
        let cfo_sum = c0.operating_cashflow + c1.operating_cashflow + c2.operating_cashflow;
        let pat_sum = inc
            .iter()
            .take(3)
            .map(|r| r.net_income)
            .sum::<f64>();
        let fcf_sum = c0.free_cashflow + c1.free_cashflow + c2.free_cashflow;
        if cfo_sum > pat_sum * 1.25 && fcf_sum < pat_sum * 0.7 {
            push_line(
                &mut lines,
                "Cash flow",
                "CFO exceeds PAT but cumulative FCF is weak — capex or working capital may explain the divergence; owner earnings may be lower than CFO suggests.".to_string(),
                "High",
            );
        }
    }

    let summary = if ratio > 1.25 {
        format!(
            "CFO materially exceeds PAT ({:.2}x). Treat reported earnings as conservative or investigate \
             depreciation, working-capital release, and non-recurring items in filings.",
            ratio
        )
    } else {
        format!(
            "CFO trails PAT ({:.2}x). Earnings quality is weak — prioritize receivables, inventory, and accrual policies.",
            ratio
        )
    };

    PlBsQualityAnalysis {
        trigger,
        summary,
        lines,
    }
}
