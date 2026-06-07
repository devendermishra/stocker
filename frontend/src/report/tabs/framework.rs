use dioxus::prelude::*;

use crate::components::KeyValue;
use crate::format::fmt_opt_num;
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn framework_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Weighted Scorecard" }
            p { "Total Score: {r.score_breakdown.total_score:.1}/100 ({r.score_breakdown.interpretation})" }
            KeyValue { label: "Business Quality (20)", value: format!("{:.1}", r.score_breakdown.business_quality) }
            KeyValue { label: "Industry Tailwind (15)", value: format!("{:.1}", r.score_breakdown.industry_tailwind) }
            KeyValue { label: "Financial Strength (20)", value: format!("{:.1}", r.score_breakdown.financial_strength) }
            KeyValue { label: "Management Quality (15)", value: format!("{:.1}", r.score_breakdown.management_quality) }
            KeyValue { label: "Valuation Comfort (15)", value: format!("{:.1}", r.score_breakdown.valuation_comfort) }
            KeyValue { label: "Growth Triggers (10)", value: format!("{:.1}", r.score_breakdown.growth_triggers) }
            KeyValue { label: "Risk/Reward (5)", value: format!("{:.1}", r.score_breakdown.risk_reward) }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "16-Section Structured Review" }
            KeyValue { label: "1. Company Overview", value: r.structured_sections.company_overview.clone() }
            KeyValue { label: "2. Business Model", value: r.structured_sections.business_model.clone() }
            KeyValue { label: "3. Industry Opportunity", value: r.structured_sections.industry_opportunity.clone() }
            KeyValue { label: "4. Competitive Advantage", value: r.structured_sections.competitive_advantage.clone() }
            KeyValue { label: "5. Management Quality", value: r.structured_sections.management_quality.clone() }
            KeyValue { label: "6. Financial Performance", value: r.structured_sections.financial_performance.clone() }
            KeyValue { label: "7. Balance Sheet Strength", value: r.financial_strength_audit.interpretation.clone() }
            KeyValue { label: "8. Cash Flow Quality", value: r.structured_sections.cash_flow_quality.narrative.clone() }
            KeyValue { label: "8b. Earnings quality score", value: format!("{:.0}/100", r.financial_strength_audit.earnings_quality_score) }
            KeyValue { label: "8c. Balance sheet score", value: format!("{:.0}/100", r.financial_strength_audit.balance_sheet_score) }
            KeyValue { label: "9. Valuation", value: r.structured_sections.valuation.clone() }
            KeyValue { label: "11. Growth Triggers", value: r.structured_sections.growth_triggers.join(", ") }
            KeyValue { label: "14. Entry/Exit Strategy", value: r.structured_sections.entry_exit_strategy.clone() }
            KeyValue { label: "16. Final Recommendation", value: r.structured_sections.final_recommendation.clone() }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "10. Peer Comparison (Company vs 3 Peers)" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.9rem;",
                thead {
                    tr {
                        th { "Metric" }
                        th { "{r.structured_sections.peer_comparison.first().map(|x| x.company_label.clone()).unwrap_or_else(|| \"Company\".to_string())}" }
                        th { "{r.structured_sections.peer_comparison.first().map(|x| x.peer_1_label.clone()).unwrap_or_else(|| \"Peer 1\".to_string())}" }
                        th { "{r.structured_sections.peer_comparison.first().map(|x| x.peer_2_label.clone()).unwrap_or_else(|| \"Peer 2\".to_string())}" }
                        th { "{r.structured_sections.peer_comparison.first().map(|x| x.peer_3_label.clone()).unwrap_or_else(|| \"Peer 3\".to_string())}" }
                    }
                }
                tbody {
                    for row in &r.structured_sections.peer_comparison {
                        tr {
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{row.metric}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_opt_num(row.company)}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_opt_num(row.peer_1)}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_opt_num(row.peer_2)}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_opt_num(row.peer_3)}" }
                        }
                    }
                }
            }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "12. Categorized Risks" }
            h4 { "Business Risks" } ul { for item in &r.structured_sections.risks.business_risks { li { "{item.risk} ({item.severity}) - {item.note}" } } }
            h4 { "Financial Risks" } ul { for item in &r.structured_sections.risks.financial_risks { li { "{item.risk} ({item.severity}) - {item.note}" } } }
            h4 { "Management Risks" } ul { for item in &r.structured_sections.risks.management_risks { li { "{item.risk} ({item.severity}) - {item.note}" } } }
            h4 { "Valuation Risks" } ul { for item in &r.structured_sections.risks.valuation_risks { li { "{item.risk} ({item.severity}) - {item.note}" } } }
            h4 { "Regulatory Risks" } ul { for item in &r.structured_sections.risks.regulatory_risks { li { "{item.risk} ({item.severity}) - {item.note}" } } }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "13. Scenario Analysis" }
            p { "Base: {r.structured_sections.scenario_analysis.base_case}" }
            p { "Upside: {r.structured_sections.scenario_analysis.upside_case}" }
            p { "Downside: {r.structured_sections.scenario_analysis.downside_case}" }
            p { style: "font-weight:600;", "{r.structured_sections.scenario_analysis.capital_impairment_guardrail}" }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Financial Strength Checklist" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.88rem;",
                thead { tr { th { "Metric" } th { "Value" } th { "Benchmark" } th { "Status" } } }
                tbody {
                    for item in &r.financial_strength_audit.checklist {
                        tr {
                            td { style: "padding:0.3rem 0; border-top:1px solid #eceff5;", "{item.metric}" }
                            td { style: "padding:0.3rem 0; border-top:1px solid #eceff5;", "{item.value_display}" }
                            td { style: "padding:0.3rem 0; border-top:1px solid #eceff5;", "{item.benchmark}" }
                            td { style: "padding:0.3rem 0; border-top:1px solid #eceff5;", "{item.status}" }
                        }
                    }
                }
            }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "15. Key Monitorables" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.9rem;",
                thead { tr { th { "Area" } th { "What to track" } th { "Status" } } }
                tbody {
                    for m in &r.structured_sections.key_monitorables {
                        tr {
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{m.area}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{m.what_to_track}" }
                            td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{m.status}" }
                        }
                    }
                }
            }
        }
    }
}
