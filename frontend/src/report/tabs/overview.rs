use dioxus::prelude::*;

use crate::components::MetricCard;
use crate::format::{fmt_money, fmt_opt_pct, fmt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn overview_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Executive Summary" }
            p { style: "line-height:1.55;", "{r.report_insights.executive_summary}" }
            p { style: "line-height:1.55; color:#243043;", "{r.stock_analysis.narrative}" }
        }
        div { style: "display:grid; grid-template-columns: repeat(auto-fit,minmax(280px,1fr)); gap: 0.6rem; margin-top: 0.65rem;",
            div { style: "{card}",
                h3 { style: "margin-top:0;", "Strengths" }
                ul { for s in &r.report_insights.strengths { li { "{s}" } } }
            }
            div { style: "{card}",
                h3 { style: "margin-top:0;", "Watch Items" }
                ul { for w in &r.report_insights.watch_items { li { "{w}" } } }
            }
        }
        div { style: "display:grid; grid-template-columns: repeat(auto-fit,minmax(180px,1fr)); gap: 0.55rem; margin-top: 0.65rem;",
            MetricCard { label: "Quality Score", value: format!("{:.0}/100", r.stock_analysis.quality_score) }
            MetricCard { label: "P/E", value: format!("{:.2}", r.financials.pe_ratio) }
            MetricCard { label: "Forward P/E", value: format!("{:.2}", r.financials.forward_pe) }
            MetricCard { label: "Market Cap", value: fmt_money(r.financials.market_cap) }
            MetricCard { label: "ROE", value: fmt_pct(r.financials.return_on_equity) }
            MetricCard { label: "Net Margin", value: fmt_pct(r.financials.profit_margins) }
            MetricCard { label: "Debt / Equity", value: format!("{:.2}", r.financials.debt_to_equity) }
            MetricCard { label: "Total Debt", value: fmt_money(r.financials.total_debt) }
            MetricCard { label: "FCF Yield", value: fmt_opt_pct(r.stock_analysis.fcf_yield_pct) }
            MetricCard { label: "Earnings Yield", value: fmt_opt_pct(r.stock_analysis.earnings_yield_pct) }
            MetricCard { label: "Revenue CAGR (3Y)", value: fmt_opt_pct(r.stock_analysis.revenue_cagr_trailing_3y_pct) }
            MetricCard { label: "Net Income CAGR (3Y)", value: fmt_opt_pct(r.stock_analysis.net_income_cagr_trailing_3y_pct) }
            MetricCard { label: "Beta", value: if r.financials.beta > 0.0 { format!("{:.2}", r.financials.beta) } else { "N/A".to_string() } }
            MetricCard { label: "52W Low / High", value: format!("₹{:.2} / ₹{:.2}", r.financials.fifty_two_week_low, r.financials.fifty_two_week_high) }
            MetricCard { label: "Day Change", value: fmt_pct(r.financials.regular_market_change_percent) }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Annual Revenue / Net Income (Yahoo)" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.92rem;",
                thead { tr { th { "Fiscal Year" } th { "Revenue" } th { "Net Income" } } }
                tbody {
                    for a in &r.annual_reports {
                        tr {
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;", "{a.date}" }
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;", "{fmt_money(a.revenue)}" }
                            td { style: "padding: 0.35rem 0; border-top: 1px solid #eceff5;", "{fmt_money(a.net_income)}" }
                        }
                    }
                }
            }
        }
    }
}
