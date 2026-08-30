use dioxus::prelude::*;

use crate::format::{fmt_money, fmt_opt_ratio, fmt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn peers_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let peer_percentiles = format!(
        "Percentiles - P/E: {}, ROE: {} (coverage {}/{}), Screening quality: {}",
        r.peer_analysis
            .subject_percentile_pe
            .map(|x| format!("{:.0}", x))
            .unwrap_or_else(|| "N/A".to_string()),
        r.peer_analysis
            .subject_percentile_roe
            .map(|x| format!("{:.0}", x))
            .unwrap_or_else(|| "N/A".to_string()),
        r.peer_analysis.roe_coverage_known,
        r.peer_analysis.roe_coverage_total,
        r.peer_analysis
            .subject_percentile_quality
            .map(|x| format!("{:.0}", x))
            .unwrap_or_else(|| "N/A".to_string())
    );
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Peer Positioning" }
            p { "Comparability: {r.peer_analysis.peer_comparability}" }
            if !r.peer_analysis.direct_peer_comparability.is_empty() {
                p { "{r.peer_analysis.direct_peer_comparability}" }
            }
            if !r.peer_analysis.bank_comparability.is_empty() {
                p { "{r.peer_analysis.bank_comparability}" }
            }
            p { "{r.peer_analysis.narrative}" }
            p { "{peer_percentiles}" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.9rem; margin-top: 0.5rem;",
                thead { tr { th { "Peer" } th { "Symbol" } th { "Price" } th { "MCap" } th { "P/E" } th { "ROE" } th { "Margin" } } }
                tbody {
                    for p in &r.peer_analysis.peers {
                        tr {
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{p.short_name.clone().unwrap_or_else(|| \"N/A\".to_string())}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{p.symbol}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "₹{p.price:.2}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_money(p.market_cap)}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{p.pe_ratio:.2}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_opt_ratio(p.return_on_equity)}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_pct(p.profit_margins)}" }
                        }
                    }
                }
            }
        }
    }
}
