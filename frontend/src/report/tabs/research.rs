use dioxus::prelude::*;

use crate::format::{fmt_money, fmt_opt_num, fmt_opt_pct, fmt_price_in_currency};
use crate::report::CARD;
use crate::types::ResearchReport;

#[component]
fn FundamentalsBlock(
    title: String,
    interpretation: String,
    confidence: String,
    lines: Vec<(String, String)>,
    flags: Vec<String>,
) -> Element {
    rsx! {
        div { style: "border-top: 1px solid #eceff5; padding-top: 0.5rem;",
            h4 { style: "margin: 0 0 0.35rem; font-size: 0.95rem;", "{title}" }
            p { style: "margin: 0 0 0.35rem; color:#333; font-size: 0.9rem;", "{interpretation}" }
            p { style: "margin: 0; font-size: 0.82rem; color:#666;", "Confidence: {confidence}" }
            ul { style: "margin: 0.35rem 0 0; padding-left: 1.1rem; font-size: 0.88rem;",
                for (k, val) in lines {
                    li { "{k}: {val}" }
                }
            }
            if !flags.is_empty() {
                ul { style: "margin: 0.35rem 0 0; padding-left: 1.1rem; color:#8a4a00; font-size: 0.85rem;",
                    for fl in flags {
                        li { "{fl}" }
                    }
                }
            }
        }
    }
}

pub fn research_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let cur = r.company_overview.currency.as_deref();
    let o = &r.company_overview;
    let f = &r.fundamental_analysis;
    let v = &r.valuation_analysis;
    let t = &r.technical_analysis;
    let volume_interp_line = format!(
        "{}{}",
        t.volume.interpretation,
        if t.volume.volume_breakout {
            " (breakout flag)"
        } else {
            ""
        }
    );
    let te = &r.technical_entry;
    let rr = &r.research_rating;
    let s = &r.research_summary;
    let ag = &s.action_guidance;
    let audit = &r.financial_strength_audit;
    let ms = &r.market_signals;
    let company_heading = format!("{} ({})", o.company_name, o.ticker);
    let exchange_line = format!(
        "Exchange: {} · Sector: {} · Industry: {}",
        o.exchange.clone().unwrap_or_else(|| "N/A".to_string()),
        o.sector.clone().unwrap_or_else(|| "N/A".to_string()),
        o.industry.clone().unwrap_or_else(|| "N/A".to_string()),
    );
    let price_mc = format!(
        "Price: {} · Market cap: {}",
        fmt_price_in_currency(o.current_price, cur),
        fmt_money(o.market_cap),
    );
    let curr_country = format!(
        "Currency: {} · Country: {}",
        o.currency.clone().unwrap_or_else(|| "—".to_string()),
        o.country.clone().unwrap_or_else(|| "—".to_string()),
    );
    let fiscal_q = format!(
        "Latest fiscal year: {} · Latest quarter: {}",
        o.latest_fiscal_year_end.clone().unwrap_or_else(|| "—".to_string()),
        o.latest_quarter_end.clone().unwrap_or_else(|| "—".to_string()),
    );
    let val_label_line = format!(
        "Label: {} · Data confidence: {}",
        v.valuation_label, v.confidence
    );
    let pe_line = format!("P/E: {:.2} · Forward P/E: {:.2}", v.pe, v.forward_pe);
    let pb_line = format!("P/B: {:.2} · P/S: {:.2}", v.price_to_book, v.price_to_sales);
    let ev_line = format!(
        "EV/EBITDA: {} · EV/Sales: {}",
        fmt_opt_num(v.ev_to_ebitda),
        fmt_opt_num(v.ev_to_sales)
    );
    let peg_line = format!(
        "PEG: {} · Div yield: {:.2}%",
        fmt_opt_num(v.peg_ratio),
        v.dividend_yield * 100.0
    );
    let yield_line = format!(
        "Earnings yield: {} · FCF yield: {}",
        fmt_opt_pct(v.earnings_yield_pct),
        fmt_opt_pct(v.fcf_yield_pct)
    );
    let hist_med = format!(
        "Historical medians — P/E 3Y: {}, 5Y: {} · P/B 3Y: {}, 5Y: {}",
        fmt_opt_num(v.historical.median_pe_3y),
        fmt_opt_num(v.historical.median_pe_5y),
        fmt_opt_num(v.historical.median_pb_3y),
        fmt_opt_num(v.historical.median_pb_5y),
    );
    let earn_based = format!(
        "Earnings-based (heuristic): base {}, fair {} · Upside/downside: {}",
        fmt_price_in_currency(v.earnings_based.base_value, cur),
        fmt_price_in_currency(v.earnings_based.fair_value, cur),
        fmt_opt_pct(v.earnings_based.upside_downside_pct),
    );
    let sma_line = format!(
        "SMA20: {} · SMA50: {} · SMA100: {} · SMA200: {}",
        fmt_opt_num(t.trend.sma_20),
        fmt_opt_num(t.trend.sma_50),
        fmt_opt_num(t.trend.sma_100),
        fmt_opt_num(t.trend.sma_200),
    );
    let rsi_line = format!(
        "RSI(14): {} — {}",
        fmt_opt_num(t.momentum.rsi_14),
        t.momentum.rsi_label
    );
    let macd_line = format!(
        "MACD / signal / hist: {} / {} / {}",
        fmt_opt_num(t.momentum.macd),
        fmt_opt_num(t.momentum.macd_signal),
        fmt_opt_num(t.momentum.macd_histogram)
    );
    let roc_line = format!(
        "ROC 1M/3M/6M/1Y (%): {} / {} / {} / {}",
        fmt_opt_num(t.momentum.roc_1m_pct),
        fmt_opt_num(t.momentum.roc_3m_pct),
        fmt_opt_num(t.momentum.roc_6m_pct),
        fmt_opt_num(t.momentum.roc_1y_pct),
    );
    let vol_stats_line = format!(
        "Volatility: ann % {}, max DD % {}, ATR14 {}",
        fmt_opt_num(t.volatility.vol_1y_ann_pct),
        fmt_opt_num(t.volatility.max_drawdown_1y_pct),
        fmt_opt_num(t.volatility.atr_14),
    );
    let dist_hi_line = format!(
        "52W distance from high: {}%",
        fmt_opt_num(t.volatility.dist_from_high_pct)
    );
    let scores_line = format!(
        "Growth: {:.0} · Quality: {:.0} · Valuation: {:.0} · Technical: {:.0} · Risk: {:.0}",
        rr.growth_score,
        rr.quality_score,
        rr.valuation_score,
        rr.technical_score,
        rr.risk_score,
    );
    let ratings_line = format!(
        "Fundamental view: {} · Valuation: {} · Technical: {} · Risk: {}",
        rr.fundamental_rating,
        rr.valuation_rating,
        rr.technical_rating,
        rr.risk_rating,
    );
    let overall_hdr = format!("{:.0}/100 — {}", rr.overall_score, rr.rating_label);
    let explain_lines: Vec<String> = rr
        .explain
        .iter()
        .take(4)
        .map(|e| format!("{}: {} ({:.1})", e.factor, e.impact, e.points))
        .collect();

    rsx! {
        p { style: "color: #5a6578; font-size: 0.92rem; line-height: 1.5; border-left: 3px solid #184ad8; padding-left: 0.75rem; margin-bottom: 0.75rem;",
            "{s.disclaimer}"
        }

        div { style: "{card}",
            h3 { style: "margin-top:0;", "Executive action (not investment advice)" }
            p { style: "margin: 0 0 0.5rem; font-size: 0.92rem; line-height: 1.45;", "{ag.headline}" }
            div { style: "display: grid; grid-template-columns: repeat(auto-fit,minmax(200px,1fr)); gap: 0.5rem;",
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.55rem;",
                    p { style: "margin:0; font-size: 0.82rem; color:#666;", "If you hold" }
                    p { style: "margin: 0.2rem 0 0; font-size: 1.05rem; font-weight: 700; color:#184ad8;", "{ag.if_holding}" }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.55rem;",
                    p { style: "margin:0; font-size: 0.82rem; color:#666;", "If considering entry" }
                    p { style: "margin: 0.2rem 0 0; font-size: 1.05rem; font-weight: 700; color:#184ad8;", "{ag.if_considering_entry}" }
                }
            }
            if !ag.wait_for_events.is_empty() {
                h4 { style: "margin: 0.65rem 0 0.3rem; font-size: 0.88rem;", "Wait for these events" }
                ul { style: "margin:0; padding-left: 1.1rem; font-size: 0.86rem;",
                    for ev in &ag.wait_for_events {
                        li { "{ev}" }
                    }
                }
            }
        }

        div { style: "{card}; margin-top: 0.55rem;",
            h3 { style: "margin-top:0;", "Financial strength audit" }
            p { style: "margin: 0 0 0.45rem; font-size: 0.9rem;",
                "Overall: " strong { "{audit.overall_strength_score:.0}/100" }
                " · Earnings quality: {audit.earnings_quality_score:.0}/100 · Balance sheet: {audit.balance_sheet_score:.0}/100 · Confidence: {audit.confidence}"
            }
            p { style: "margin: 0 0 0.5rem; font-size: 0.88rem; color:#333; line-height: 1.45;", "{audit.interpretation}" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.85rem;",
                thead {
                    tr {
                        th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Metric" }
                        th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Value" }
                        th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Benchmark" }
                        th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Status" }
                    }
                }
                tbody {
                    for item in &audit.checklist {
                        tr {
                            td { style: "padding:0.35rem 0.25rem 0.35rem 0; border-top:1px solid #f0f2f6; vertical-align:top;", "{item.metric}" }
                            td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6;", "{item.value_display}" }
                            td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6;", "{item.benchmark}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #f0f2f6; font-weight:600;",
                                match item.status.as_str() {
                                    "pass" => rsx! { span { style: "color:#1a7f4b;", "Pass" } },
                                    "watch" => rsx! { span { style: "color:#9a6b00;", "Watch" } },
                                    "fail" => rsx! { span { style: "color:#b42318;", "Fail" } },
                                    _ => rsx! { span { style: "color:#666;", "N/A" } },
                                }
                            }
                        }
                    }
                }
            }
        }

        div { style: "{card}; margin-top: 0.55rem;",
            h3 { style: "margin-top:0;", "Market signals" }
            p { style: "margin: 0 0 0.45rem; font-size: 0.88rem;", "{ms.narrative}" }
            p { style: "margin: 0 0 0.35rem; font-size: 0.86rem; font-weight:600;", "{ms.analyst.consensus_label}" }
            if !ms.analyst.trend.is_empty() {
                table { style: "width:100%; border-collapse: collapse; font-size: 0.82rem; margin-bottom: 0.5rem;",
                    thead { tr { th { "Period" } th { "Strong Buy" } th { "Buy" } th { "Hold" } th { "Sell" } th { "Strong Sell" } } }
                    tbody {
                        for p in ms.analyst.trend.iter().take(4) {
                            tr {
                                td { "{p.period}" }
                                td { "{p.strong_buy}" }
                                td { "{p.buy}" }
                                td { "{p.hold}" }
                                td { "{p.sell}" }
                                td { "{p.strong_sell}" }
                            }
                        }
                    }
                }
            }
            if !ms.institutional_holders.is_empty() {
                h4 { style: "margin: 0.4rem 0 0.25rem; font-size: 0.86rem;", "Top institutional holders" }
                ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.84rem;",
                    for h in ms.institutional_holders.iter().take(5) {
                        li {
                            {format!("{}: {:.1}%", h.organization, h.pct_held * 100.0)}
                        }
                    }
                }
            }
            if !ms.insider_transactions.is_empty() {
                h4 { style: "margin: 0.4rem 0 0.25rem; font-size: 0.86rem;", "Recent insider transactions" }
                ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.84rem;",
                    for t in ms.insider_transactions.iter().take(5) {
                        li { "{t.filer_name} — {t.transaction_text} ({t.shares:.0} shares, {t.start_date})" }
                    }
                }
            }
        }

        div { style: "{card}",
            h3 { style: "margin-top:0;", "Company overview" }
            p { style: "margin: 0.25rem 0;", strong { "{company_heading}" } }
            p { style: "margin: 0.2rem 0; color:#444;", "{exchange_line}" }
            p { style: "margin: 0.2rem 0;", "{price_mc}" }
            p { style: "margin: 0.2rem 0; font-size: 0.9rem;", "{curr_country}" }
            if let Some(ref w) = o.website {
                p { style: "margin: 0.35rem 0 0; font-size: 0.9rem;",
                    a { href: "{w}", target: "_blank", rel: "noopener", "{w}" }
                }
            }
            p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; color:#333; line-height: 1.5;",
                "{o.business_summary_short}"
            }
            p { style: "margin: 0.35rem 0 0; font-size: 0.85rem; color:#666;", "{fiscal_q}" }
        }

        details { style: "{card}; margin-top: 0.55rem;",
            summary { style: "cursor: pointer; font-weight: 600;", "Fundamental analysis" }
            div { style: "margin-top: 0.65rem; display: grid; gap: 0.75rem;",
                FundamentalsBlock { title: "Growth".to_string(), interpretation: f.growth.interpretation.clone(), confidence: f.growth.confidence.clone(), lines: f.growth.lines.clone(), flags: f.growth.flags.clone() }
                FundamentalsBlock { title: "Profitability".to_string(), interpretation: f.profitability.interpretation.clone(), confidence: f.profitability.confidence.clone(), lines: f.profitability.lines.clone(), flags: f.profitability.flags.clone() }
                FundamentalsBlock { title: "Balance sheet".to_string(), interpretation: f.balance_sheet.interpretation.clone(), confidence: f.balance_sheet.confidence.clone(), lines: f.balance_sheet.lines.clone(), flags: f.balance_sheet.flags.clone() }
                FundamentalsBlock { title: "Cash flow quality".to_string(), interpretation: f.cash_flow.interpretation.clone(), confidence: f.cash_flow.confidence.clone(), lines: f.cash_flow.lines.clone(), flags: f.cash_flow.flags.clone() }
                FundamentalsBlock { title: "Efficiency".to_string(), interpretation: f.efficiency.interpretation.clone(), confidence: f.efficiency.confidence.clone(), lines: f.efficiency.lines.clone(), flags: f.efficiency.flags.clone() }
            }
        }

        details { style: "{card}; margin-top: 0.55rem;",
            summary { style: "cursor: pointer; font-weight: 600;", "Valuation" }
            div { style: "margin-top: 0.65rem;",
                p { style: "font-size: 0.9rem;", "{val_label_line}" }
                p { style: "font-size: 0.88rem; color:#333;", "{v.historical_classification}" }
                p { style: "font-size: 0.88rem;", "Peer read: {v.peer_value_read}" }
                ul { style: "font-size: 0.88rem; padding-left: 1.1rem;",
                    li { "{pe_line}" }
                    li { "{pb_line}" }
                    li { "{ev_line}" }
                    li { "{peg_line}" }
                    li { "{yield_line}" }
                }
                p { style: "font-size: 0.85rem; color:#555; margin-top: 0.5rem;", "{hist_med}" }
                p { style: "font-size: 0.85rem; margin-top: 0.35rem;", "{earn_based}" }
            }
        }

        details { style: "{card}; margin-top: 0.55rem;",
            summary { style: "cursor: pointer; font-weight: 600;", "Technical analysis" }
            div { style: "margin-top: 0.65rem;",
                p { style: "font-size: 0.88rem;", "Confidence: {t.confidence}" }
                p { style: "font-size: 0.9rem;", strong { "Trend: " } "{t.trend.trend_label}" }
                ul { style: "font-size: 0.88rem; padding-left: 1.1rem;",
                    li { "{sma_line}" }
                    li { "{rsi_line}" }
                    li { "{macd_line}" }
                    li { "{roc_line}" }
                    li { "{vol_stats_line}" }
                    li { "{dist_hi_line}" }
                    li { "{t.volatility.note}" }
                    li { "{volume_interp_line}" }
                }
            }
        }

        div { style: "display: grid; grid-template-columns: repeat(auto-fit,minmax(260px,1fr)); gap: 0.6rem; margin-top: 0.55rem;",
            div { style: "{card}",
                h3 { style: "margin-top:0; font-size: 0.98rem;", "Fundamental: cheap / fair / expensive" }
                p { style: "margin:0; font-weight: 600; color: #184ad8;", "{rr.cheap_fair_expensive_fundamental}" }
                p { style: "margin: 0.35rem 0 0; font-size: 0.88rem; color:#444;", "Rating: {rr.valuation_rating}" }
            }
            div { style: "{card}",
                h3 { style: "margin-top:0; font-size: 0.98rem;", "Technical entry zone" }
                p { style: "margin:0; font-weight: 600;", "{te.zone}" }
                p { style: "margin: 0.25rem 0 0; font-size: 0.9rem;", "{te.detail_label}" }
                ul { style: "margin: 0.35rem 0 0; padding-left: 1.1rem; font-size: 0.85rem;",
                    for line in &te.rationale { li { "{line}" } }
                }
            }
        }

        div { style: "{card}; margin-top: 0.55rem;",
            p { style: "margin:0; font-size: 0.92rem; line-height: 1.5;", "{te.fundamental_vs_technical}" }
        }

        div { style: "{card}; margin-top: 0.55rem;",
            h3 { style: "margin-top:0;", "Scores & action" }
            p { style: "margin: 0 0 0.5rem; font-size: 0.92rem;",
                "Overall: " strong { "{overall_hdr}" }
            }
            ul { style: "font-size: 0.88rem; padding-left: 1.1rem; margin: 0 0 0.5rem;",
                li { "{scores_line}" }
                li { "{ratings_line}" }
            }
            p { style: "font-size: 0.9rem; margin: 0 0 0.35rem;",
                "Suggested entry action: " strong { "{s.suggested_action}" }
                " · If holding: " strong { "{ag.if_holding}" }
            }
            for line in explain_lines {
                p { style: "font-size: 0.82rem; color:#444; margin: 0.2rem 0;", "{line}" }
            }
        }

        div { style: "{card}; margin-top: 0.55rem;",
            h3 { style: "margin-top:0;", "Research summary" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Business: " } "{s.business_quality}" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Growth: " } "{s.growth}" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Valuation: " } "{s.valuation}" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Technicals: " } "{s.technical_position}" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Risks: " } "{s.key_risks}" }
            p { style: "font-size: 0.9rem; margin: 0.35rem 0;", strong { "Final view: " } "{s.final_view}" }
            div { style: "display: grid; grid-template-columns: repeat(auto-fit,minmax(200px,1fr)); gap: 0.5rem; margin-top: 0.5rem;",
                div {
                    h4 { style: "margin:0 0 0.25rem; font-size: 0.88rem;", "Positives" }
                    ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.85rem;",
                        for x in &s.key_positives { li { "{x}" } }
                    }
                }
                div {
                    h4 { style: "margin:0 0 0.25rem; font-size: 0.88rem;", "Negatives" }
                    ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.85rem;",
                        for x in &s.key_negatives { li { "{x}" } }
                    }
                }
                div {
                    h4 { style: "margin:0 0 0.25rem; font-size: 0.88rem;", "Monitorables" }
                    ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.85rem;",
                        for x in &s.key_monitorables { li { "{x}" } }
                    }
                }
            }
        }
    }
}
