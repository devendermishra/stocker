use dioxus::prelude::*;

use crate::components::KeyValue;
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn management_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Management & Qualitative Read" }
            KeyValue { label: "Pay Efficiency Score", value: format!("{:.0}/100", r.management_analysis.pay_vs_revenue_score) }
            KeyValue { label: "Summary Tone", value: format!("{} ({:.0}/100)", r.management_analysis.tone_label, r.management_analysis.tone_score) }
            p { style: "line-height:1.55; color:#273447;", "{r.management_analysis.narrative}" }
            p { style: "line-height:1.55; color:#273447;", "{r.stock_analysis.margin_trend}" }
            if let Some(ref s) = r.company_summary {
                h4 { "Company Summary (Yahoo)" }
                p { style: "font-size: 0.92rem; line-height: 1.55; color: #3a4352;", "{s}" }
            }
        }
    }
}
