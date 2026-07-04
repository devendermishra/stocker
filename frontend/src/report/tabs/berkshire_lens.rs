use dioxus::prelude::*;

use crate::format::{fmt_opt_num, fmt_opt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

#[cfg(all(feature = "desktop", not(feature = "web")))]
fn business_tier_display(r: &ResearchReport) -> String {
    stocker_core::tier_label_str(r.buffett_lens.price_vs_value.business_tier_label).to_string()
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn business_tier_display(r: &ResearchReport) -> String {
    r.buffett_lens.price_vs_value.business_tier_label.clone()
}

pub fn berkshire_lens_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let bl = &r.buffett_lens;
    let tier_label = business_tier_display(r);
    rsx! {
        div { style: "{card}",
            h3 { style: "margin-top:0;", "Berkshire Lens" }
            p { style: "margin: 0 0 0.5rem; font-size: 0.92rem; line-height: 1.45; font-weight: 600; color: #243043;",
                "{bl.headline_verdict}"
            }
            div { style: "display: grid; grid-template-columns: repeat(auto-fit,minmax(140px,1fr)); gap: 0.45rem; margin-bottom: 0.55rem;",
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Moat" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;", "{bl.moat_assessment.score:.0}/100" }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Earnings durability" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;",
                        {format!("{}/100", bl.scores.earnings_durability_score.map(|s| format!("{s:.0}")).unwrap_or_else(|| "—".to_string()))}
                    }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Capital intensity" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;",
                        {format!("{}/100", bl.scores.capital_intensity_score.map(|s| format!("{s:.0}")).unwrap_or_else(|| "—".to_string()))}
                    }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Trust" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;", "{bl.management_trust.score:.0}/100" }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "MOS" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;",
                        {fmt_opt_pct(bl.price_vs_value.margin_of_safety_pct)}
                    }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Owner yield" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;",
                        {fmt_opt_pct(bl.scores.owner_earnings_yield_pct)}
                    }
                }
                div { style: "border: 1px solid #dce3ef; border-radius: 6px; padding: 0.45rem;",
                    p { style: "margin:0; font-size: 0.78rem; color:#666;", "Business tier" }
                    p { style: "margin: 0.15rem 0 0; font-weight: 700;", "{tier_label}" }
                }
            }

            if !bl.score_reasons.is_empty() {
                h4 { style: "margin: 0.65rem 0 0.35rem; font-size: 0.92rem;", "Score breakdown" }
                table { style: "width:100%; border-collapse: collapse; font-size: 0.85rem;",
                    thead {
                        tr {
                            th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Dimension" }
                            th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Score" }
                            th { style: "text-align:left; padding:0.35rem 0; border-bottom:1px solid #eceff5;", "Why" }
                        }
                    }
                    tbody {
                        for sr in &bl.score_reasons {
                            tr {
                                td { style: "padding:0.35rem 0.25rem 0.35rem 0; border-top:1px solid #f0f2f6; vertical-align:top;", "{sr.dimension}" }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; white-space:nowrap;",
                                    if sr.dimension == "Margin of safety" || sr.dimension == "Owner earnings yield" {
                                        {fmt_opt_pct(Some(sr.score))}
                                    } else if sr.dimension == "Business tier" {
                                        {format!("{:.0}", sr.score)}
                                    } else {
                                        {format!("{:.0}/100", sr.score)}
                                    }
                                }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; color:#444; line-height:1.4;", "{sr.reason}" }
                            }
                        }
                    }
                }
            }

            h4 { style: "margin: 0.65rem 0 0.35rem; font-size: 0.92rem;", "Five core questions" }
            ol { style: "margin: 0; padding-left: 1.2rem; font-size: 0.88rem; line-height: 1.5;",
                for ans in &bl.five_answers {
                    li { style: "margin-bottom: 0.35rem;", "{ans}" }
                }
            }

            div { style: "margin-top: 0.75rem; display: grid; gap: 0.55rem;",
                details { open: true,
                    summary { style: "cursor: pointer; font-weight: 600; font-size: 0.9rem;", "Earnings picture" }
                    p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; line-height: 1.45;", "{bl.earnings_picture.narrative}" }
                    ul { style: "margin: 0.35rem 0 0; padding-left: 1.1rem; font-size: 0.85rem;",
                        li { "Owner earnings (TTM): {fmt_opt_num(bl.earnings_picture.owner_earnings_ttm)}" }
                        li { "Owner earnings yield: {fmt_opt_pct(bl.earnings_picture.owner_earnings_yield_pct)}" }
                        li { "ROE: {fmt_opt_pct(Some(bl.earnings_picture.roe_pct))}" }
                        li { "ROCE: {fmt_opt_pct(bl.earnings_picture.roce_pct)}" }
                    }
                }
                details {
                    summary { style: "cursor: pointer; font-weight: 600; font-size: 0.9rem;", "Moat assessment" }
                    p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; line-height: 1.45;", "{bl.moat_assessment.narrative}" }
                    if !bl.moat_assessment.moat_types.is_empty() {
                        ul { style: "margin: 0.35rem 0 0; padding-left: 1.1rem; font-size: 0.85rem;",
                            for m in &bl.moat_assessment.moat_types {
                                li { "{m.label}: {m.evidence}" }
                            }
                        }
                    }
                }
                details {
                    summary { style: "cursor: pointer; font-weight: 600; font-size: 0.9rem;", "Capital intensity" }
                    p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; line-height: 1.45;", "{bl.capital_intensity.narrative}" }
                }
                details {
                    summary { style: "cursor: pointer; font-weight: 600; font-size: 0.9rem;", "Management trust" }
                    p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; line-height: 1.45;", "{bl.management_trust.narrative}" }
                }
                details {
                    summary { style: "cursor: pointer; font-weight: 600; font-size: 0.9rem;", "Price vs value" }
                    p { style: "margin: 0.45rem 0 0; font-size: 0.88rem; line-height: 1.45;", "{bl.price_vs_value.narrative}" }
                    p { style: "margin: 0.35rem 0 0; font-size: 0.86rem; color: #444;", "{bl.price_vs_value.graham_buffett_read}" }
                }
            }

            if !bl.accounting_skepticism_flags.is_empty() {
                h4 { style: "margin: 0.75rem 0 0.35rem; font-size: 0.9rem; color: #8a4a00;", "Accounting skepticism" }
                ul { style: "margin:0; padding-left: 1.05rem; font-size: 0.86rem; color: #8a4a00;",
                    for fl in &bl.accounting_skepticism_flags {
                        li { "{fl}" }
                    }
                }
            }

            p { style: "margin: 0.75rem 0 0; font-size: 0.8rem; color: #888; line-height: 1.45;", "{bl.philosophy_note}" }
        }
    }
}
