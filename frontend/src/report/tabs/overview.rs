use dioxus::prelude::*;

use crate::components::MetricCard;
use crate::format::{fmt_money, fmt_opt_money, fmt_opt_pct, fmt_pct, fmt_price_in_currency};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn overview_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let cur = r.asset_profile.currency.as_deref();
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Executive Summary" }
            p { style: "line-height:1.55;", "{r.report_insights.executive_summary}" }
            p { style: "line-height:1.55; color:#243043;", "{r.stock_analysis.narrative}" }
            if !r.retrieved_at.is_empty() {
                p { style: "line-height:1.45; color:#5a6578; font-size:0.85rem;", "Live Yahoo snapshot as of {r.retrieved_at}" }
            }
            if let Some(enr) = &r.screener_enrichment {
                if let Some(ts) = enr.updated_at {
                    p { style: "line-height:1.45; color:#5a6578; font-size:0.85rem;", "Screener enrichment updated_at (unix): {ts} — statement-derived metrics, not live Yahoo." }
                }
            }
        }
        div { style: "display:grid; grid-template-columns: repeat(auto-fit,minmax(280px,1fr)); gap: 0.6rem; margin-top: 0.65rem;",
            div { style: "{card}",
                h3 { style: "margin-top:0;", "Strengths" }
                if r.report_insights.strengths.is_empty() {
                    p { style: "color:#5a6578; font-size:0.88rem;", "No company strengths established from Yahoo." }
                } else {
                    ul { for s in &r.report_insights.strengths { li { "{s}" } } }
                }
            }
            div { style: "{card}",
                h3 { style: "margin-top:0;", "Watch Items" }
                ul { for w in &r.report_insights.watch_items { li { "{w}" } } }
            }
        }
        if !r.report_insights.data_notes.is_empty() || !r.report_insights.data_strengths.is_empty() {
            div { style: "display:grid; grid-template-columns: repeat(auto-fit,minmax(280px,1fr)); gap: 0.6rem; margin-top: 0.65rem;",
                if !r.report_insights.data_strengths.is_empty() {
                    div { style: "{card}",
                        h3 { style: "margin-top:0;", "Data coverage" }
                        ul { for s in &r.report_insights.data_strengths { li { "{s}" } } }
                    }
                }
                if !r.report_insights.data_notes.is_empty() {
                    div { style: "{card}",
                        h3 { style: "margin-top:0;", "Data notes" }
                        ul { for s in &r.report_insights.data_notes { li { "{s}" } } }
                    }
                }
            }
        }
        div { style: "display:grid; grid-template-columns: repeat(auto-fit,minmax(180px,1fr)); gap: 0.55rem; margin-top: 0.65rem;",
            MetricCard { label: "Data quality (Yahoo snapshot)", value: format!("{:.0}/100", r.stock_analysis.data_quality_score) }
            MetricCard { label: "P/E", value: if r.financials.pe_ratio > 0.0 { format!("{:.2}", r.financials.pe_ratio) } else { "N/A".to_string() } }
            MetricCard { label: "Forward P/E (Yahoo API; page may show —)", value: crate::format::fmt_opt_multiple(r.financials.forward_pe) }
            MetricCard { label: "Market Cap", value: fmt_money(r.financials.market_cap) }
            MetricCard { label: "ROE", value: crate::format::fmt_opt_ratio(r.financials.return_on_equity) }
            MetricCard { label: "Net Margin", value: fmt_pct(r.financials.profit_margins) }
            MetricCard { label: "Debt / Equity", value: r.financials.debt_to_equity.map(|d| format!("{:.2}", d)).unwrap_or_else(|| "N/A".to_string()) }
            MetricCard { label: "Total Debt", value: fmt_money(r.financials.total_debt) }
            MetricCard { label: "FCF Yield (CFO − capex)", value: if r.canonical.industrial_metrics_suppressed { "N/A — not used for lenders".to_string() } else { fmt_opt_pct(r.stock_analysis.fcf_yield_pct) } }
            MetricCard { label: "P/B", value: if r.financials.price_to_book > 0.0 { format!("{:.2}", r.financials.price_to_book) } else { "N/A".to_string() } }
            MetricCard { label: "Earnings Yield (EPS/price)", value: fmt_opt_pct(r.stock_analysis.earnings_yield_pct) }
            MetricCard { label: "PAT CAGR (3Y)", value: fmt_opt_pct(r.stock_analysis.net_income_cagr_trailing_3y_pct) }
            if !r.canonical.industrial_metrics_suppressed {
                MetricCard { label: "Revenue CAGR (3Y)", value: fmt_opt_pct(r.stock_analysis.revenue_cagr_trailing_3y_pct) }
            }
            MetricCard { label: "Beta", value: if r.financials.beta > 0.0 { format!("{:.2}", r.financials.beta) } else { "N/A".to_string() } }
            MetricCard { label: "52W Low / High", value: format!("{} / {}", fmt_price_in_currency(r.financials.fifty_two_week_low, cur), fmt_price_in_currency(r.financials.fifty_two_week_high, cur)) }
            MetricCard { label: "Day Change", value: fmt_pct(r.financials.regular_market_change_percent) }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", if r.canonical.industrial_metrics_suppressed { "Annual Yahoo totalRevenue / net income (statement series)" } else { "Annual Revenue / Net Income (statement series)" } }
            p { style: "font-size:0.84rem; color:#5a6578; line-height:1.45;",
                if r.canonical.industrial_metrics_suppressed {
                    "Yahoo totalRevenue is not industrial sales. PAT scope is unknown (Yahoo does not label standalone vs consolidated)."
                } else {
                    "From Yahoo fundamentals time series after scope checks — not quoteSummary incomeStatementHistory (which can mix standalone and consolidated years)."
                }
            }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.92rem;",
                thead { tr {
                    th { "Fiscal Year" }
                    th { if r.annual_reports.first().map(|a| a.revenue_represents_sales).unwrap_or(true) { "Revenue" } else { "Yahoo totalRevenue (raw)" } }
                    th { "Net Income" }
                    th { "Note" }
                } }
                tbody {
                    for a in &r.annual_reports {
                        tr {
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;", "{a.date}" }
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;",
                                {if a.revenue_represents_sales {
                                    fmt_opt_money(a.revenue)
                                } else {
                                    fmt_money(a.yahoo_total_revenue_raw)
                                }}
                            }
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;", "{fmt_money(a.net_income)}" }
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5; color:#8a4a00; font-size:0.8rem;",
                                {a.series_warning.clone().unwrap_or_default()}
                            }
                        }
                    }
                }
            }
        }
    }
}
