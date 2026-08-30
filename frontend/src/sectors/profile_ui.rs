//! Shared sector research profile rendering (catalog detail + report Sector tab).

use dioxus::prelude::*;

use crate::sectors_api::{PorterFiveForcesView, SectorResearchProfileView};

pub const CARD: &str =
    "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";

pub fn humanize_snake(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn badge_style() -> &'static str {
    "display: inline-block; padding: 0.15rem 0.55rem; border-radius: 6px; background: #eef2ff; color: #184ad8; font-size: 0.82rem; font-weight: 600; margin-right: 0.35rem;"
}

#[component]
fn EvidenceList(items: Vec<String>) -> Element {
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        ul { style: "margin: 0.4rem 0 0; padding-left: 1.1rem; color: #445; font-size: 0.9rem;",
            for e in items {
                li { "{e}" }
            }
        }
    }
}

#[component]
fn ForceCard(name: String, intensity: f64, label: String, narrative: String, evidence: Vec<String>) -> Element {
    let card = CARD;
    rsx! {
        div { style: "{card} margin-bottom: 0.65rem;",
            div { style: "display: flex; justify-content: space-between; gap: 0.5rem; flex-wrap: wrap;",
                strong { "{name}" }
                span { style: "color: #184ad8; font-weight: 600;",
                    "{label} ({intensity:.0})"
                }
            }
            p { style: "margin: 0.4rem 0 0; color: #333;", "{narrative}" }
            EvidenceList { items: evidence }
        }
    }
}

#[component]
pub fn PorterForcesBlock(porter: PorterFiveForcesView) -> Element {
    rsx! {
        div {
            ForceCard {
                name: porter.rivalry.name.clone(),
                intensity: porter.rivalry.intensity,
                label: porter.rivalry.label.clone(),
                narrative: porter.rivalry.narrative.clone(),
                evidence: porter.rivalry.evidence.clone(),
            }
            ForceCard {
                name: porter.new_entrants.name.clone(),
                intensity: porter.new_entrants.intensity,
                label: porter.new_entrants.label.clone(),
                narrative: porter.new_entrants.narrative.clone(),
                evidence: porter.new_entrants.evidence.clone(),
            }
            ForceCard {
                name: porter.supplier_power.name.clone(),
                intensity: porter.supplier_power.intensity,
                label: porter.supplier_power.label.clone(),
                narrative: porter.supplier_power.narrative.clone(),
                evidence: porter.supplier_power.evidence.clone(),
            }
            ForceCard {
                name: porter.buyer_power.name.clone(),
                intensity: porter.buyer_power.intensity,
                label: porter.buyer_power.label.clone(),
                narrative: porter.buyer_power.narrative.clone(),
                evidence: porter.buyer_power.evidence.clone(),
            }
            ForceCard {
                name: porter.substitutes.name.clone(),
                intensity: porter.substitutes.intensity,
                label: porter.substitutes.label.clone(),
                narrative: porter.substitutes.narrative.clone(),
                evidence: porter.substitutes.evidence.clone(),
            }
        }
    }
}

#[component]
pub fn SectorResearchFull(profile: SectorResearchProfileView) -> Element {
    let card = CARD;
    let b = badge_style();
    let lifecycle = humanize_snake(&profile.lifecycle.phase);
    let stype = humanize_snake(&profile.sector_type.sector_type);
    let gap = humanize_snake(&profile.demand_supply.gap_label);
    let comp = humanize_snake(&profile.competition.structure);
    let profit = humanize_snake(&profile.profitability.level);
    let growth = humanize_snake(&profile.growth_prospects.level);
    let supplier = humanize_snake(&profile.pricing_power.supplier.level);
    let customer = humanize_snake(&profile.pricing_power.customer.level);
    let growth_badge = format!("Growth: {growth}");
    let attract = profile.porter.attractiveness;
    let count = profile.company_count;
    let ds_intensity = profile.demand_supply.intensity;
    let profit_score = profile.profitability.score;
    let growth_score = profile.growth_prospects.score;

    rsx! {
        div {
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Overview" }
                p { style: "margin: 0; color: #333;",
                    "{count} listed names · attractiveness {attract:.0}/100"
                }
                p { style: "margin: 0.55rem 0 0;",
                    span { style: "{b}", "{lifecycle}" }
                    span { style: "{b}", "{stype}" }
                    span { style: "{b}", "{growth_badge}" }
                }
                p { style: "margin: 0.55rem 0 0; color: #445; font-size: 0.92rem;",
                    "{profile.porter.summary}"
                }
            }

            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Lifecycle" }
                span { style: "{b}", "{lifecycle}" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.lifecycle.narrative}" }
                EvidenceList { items: profile.lifecycle.evidence.clone() }
            }
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Sector type" }
                span { style: "{b}", "{stype}" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.sector_type.narrative}" }
                EvidenceList { items: profile.sector_type.evidence.clone() }
            }
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Demand–supply gap" }
                span { style: "{b}", "{gap}" }
                span { style: "color: #667; font-size: 0.85rem;", "intensity {ds_intensity:.0}" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.demand_supply.narrative}" }
                EvidenceList { items: profile.demand_supply.evidence.clone() }
            }
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Nature of competition" }
                span { style: "{b}", "{comp}" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.competition.narrative}" }
                EvidenceList { items: profile.competition.evidence.clone() }
            }
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Profitability" }
                span { style: "{b}", "{profit}" }
                span { style: "color: #667; font-size: 0.85rem;", "score {profit_score:.0}/100" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.profitability.narrative}" }
                EvidenceList { items: profile.profitability.evidence.clone() }
            }
            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Growth prospects" }
                span { style: "{b}", "{growth}" }
                span { style: "color: #667; font-size: 0.85rem;", "score {growth_score:.0}/100" }
                p { style: "margin: 0.35rem 0 0; color: #333;", "{profile.growth_prospects.narrative}" }
                EvidenceList { items: profile.growth_prospects.evidence.clone() }
            }

            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Supplier & customer pricing power" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem;",
                    div {
                        strong { "vs Suppliers" }
                        p { style: "margin: 0.25rem 0;",
                            span { style: "{b}", "{supplier}" }
                        }
                        p { style: "margin: 0; color: #333; font-size: 0.92rem;",
                            "{profile.pricing_power.supplier.narrative}"
                        }
                        EvidenceList { items: profile.pricing_power.supplier.evidence.clone() }
                    }
                    div {
                        strong { "vs Customers" }
                        p { style: "margin: 0.25rem 0;",
                            span { style: "{b}", "{customer}" }
                        }
                        p { style: "margin: 0; color: #333; font-size: 0.92rem;",
                            "{profile.pricing_power.customer.narrative}"
                        }
                        EvidenceList { items: profile.pricing_power.customer.evidence.clone() }
                    }
                }
            }

            section { style: "{card} margin-bottom: 0.85rem;",
                h3 { style: "margin-top: 0;", "Porter Five Forces" }
                p { style: "margin: 0 0 0.65rem; color: #445;",
                    "Attractiveness {attract:.0}/100 · higher force intensity = more hostile"
                }
                PorterForcesBlock { porter: profile.porter.clone() }
            }
        }
    }
}

#[component]
pub fn SectorResearchCompact(profile: SectorResearchProfileView) -> Element {
    let b = badge_style();
    let lifecycle = humanize_snake(&profile.lifecycle.phase);
    let stype = humanize_snake(&profile.sector_type.sector_type);
    let gap = humanize_snake(&profile.demand_supply.gap_label);
    let supplier = humanize_snake(&profile.pricing_power.supplier.level);
    let customer = humanize_snake(&profile.pricing_power.customer.level);
    let attract = profile.porter.attractiveness;
    let attract_badge = format!("Attractiveness {attract:.0}");
    let pricing_line = format!("Pricing — suppliers: {supplier}; customers: {customer}");

    rsx! {
        div { style: "margin-top: 1rem;",
            h4 { style: "margin: 0 0 0.5rem;", "Sector research (low-confidence heuristic)" }
            p { style: "margin: 0 0 0.5rem;",
                span { style: "{b}", "{lifecycle}" }
                span { style: "{b}", "{stype}" }
                span { style: "{b}", "{gap}" }
                span { style: "{b}", "{attract_badge}" }
            }
            p { style: "margin: 0 0 0.35rem; color: #333; font-size: 0.92rem;", "{profile.lifecycle.narrative}" }
            p { style: "margin: 0 0 0.35rem; color: #333; font-size: 0.92rem;", "{profile.sector_type.narrative}" }
            p { style: "margin: 0 0 0.35rem; color: #333; font-size: 0.92rem;", "{profile.demand_supply.narrative}" }
            p { style: "margin: 0 0 0.65rem; color: #333; font-size: 0.92rem;", "{pricing_line}" }
            h4 { style: "margin: 0.75rem 0 0.45rem;", "Porter Five Forces" }
            PorterForcesBlock { porter: profile.porter.clone() }
        }
    }
}
