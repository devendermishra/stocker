//! Sector research catalog pages.

mod profile_ui;

pub use profile_ui::{SectorResearchCompact, SectorResearchFull, humanize_snake};

use dioxus::prelude::*;

use crate::format::fmt_price_in_currency;
use crate::routes::Route;
use crate::sectors_api::{
    encode_sector_path, get_sector, list_sectors, SectorDetail, SectorListItem,
};

use profile_ui::CARD as PAGE_CARD;

fn fmt_mcap(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() && x > 0.0 => {
            if x >= 1.0e7 {
                format!("₹{:.1} Cr", x / 1.0e7)
            } else {
                fmt_price_in_currency(x, Some("INR"))
            }
        }
        _ => "—".to_string(),
    }
}

#[component]
pub fn SectorsList() -> Element {
    let card = PAGE_CARD;
    let resource = use_resource(|| async { list_sectors().await });

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1060px; margin: 1.5rem auto; padding: 0 1rem 2rem;",
            Link {
                to: Route::Home { id: String::new(), exchange: String::new() },
                style: "color: #184ad8;",
                "← Home"
            }
            h1 { style: "margin: 0.8rem 0 0.4rem;", "Sector Research" }
            p { style: "color: #555; margin: 0 0 1rem;",
                "All Yahoo sectors in the screener universe. Profiles are heuristic from listed-company metrics and refresh with screener snapshots."
            }

            match &*resource.read_unchecked() {
                None => rsx! { p { "Loading sectors…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", "Error: {e}" } },
                Some(Ok(items)) => rsx! {
                    if items.is_empty() {
                        div { style: "{card}",
                            p { style: "margin: 0; color: #5a4300;",
                                "No sectors yet. Sync the screener universe and refresh snapshots."
                            }
                        }
                    } else {
                        div { style: "{card} overflow-x: auto;",
                            table { style: "width: 100%; border-collapse: collapse; font-size: 0.92rem;",
                                thead {
                                    tr { style: "text-align: left; border-bottom: 1px solid #dfe3eb; color: #556;",
                                        th { style: "padding: 0.45rem;", "Sector" }
                                        th { style: "padding: 0.45rem;", "Companies" }
                                        th { style: "padding: 0.45rem;", "Snapshots" }
                                        th { style: "padding: 0.45rem;", "Mcap" }
                                        th { style: "padding: 0.45rem;", "Lifecycle" }
                                        th { style: "padding: 0.45rem;", "Type" }
                                        th { style: "padding: 0.45rem;", "Attract." }
                                        th { style: "padding: 0.45rem;", "Growth" }
                                    }
                                }
                                tbody {
                                    for item in items.clone() {
                                        SectorRow { item }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn SectorRow(item: SectorListItem) -> Element {
    let enc = encode_sector_path(&item.sector);
    let mcap = fmt_mcap(item.total_market_cap);
    let life = humanize_snake(&item.lifecycle);
    let ty = humanize_snake(&item.sector_type);
    let growth = humanize_snake(&item.growth_prospects);
    rsx! {
        tr { style: "border-bottom: 1px solid #eef1f5;",
            td { style: "padding: 0.5rem;",
                Link {
                    to: Route::SectorDetailPage { sector: enc },
                    style: "color: #184ad8; font-weight: 600; text-decoration: none;",
                    "{item.sector}"
                }
            }
            td { style: "padding: 0.5rem;", "{item.company_count}" }
            td { style: "padding: 0.5rem;", "{item.with_snapshot_count}" }
            td { style: "padding: 0.5rem;", "{mcap}" }
            td { style: "padding: 0.5rem;", "{life}" }
            td { style: "padding: 0.5rem;", "{ty}" }
            td { style: "padding: 0.5rem;", "{item.attractiveness:.0}" }
            td { style: "padding: 0.5rem;", "{growth}" }
        }
    }
}

#[component]
pub fn SectorDetailPage(sector: String) -> Element {
    let sector_key = sector.clone();
    let resource = use_resource(move || {
        let s = sector_key.clone();
        async move { get_sector(&s).await }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1060px; margin: 1.5rem auto; padding: 0 1rem 2rem;",
            Link {
                to: Route::SectorsList {},
                style: "color: #184ad8;",
                "← All sectors"
            }

            match &*resource.read_unchecked() {
                None => rsx! {
                    h1 { style: "margin: 0.8rem 0 0.4rem;", "Sector" }
                    p { "Loading…" }
                },
                Some(Err(e)) => rsx! {
                    h1 { style: "margin: 0.8rem 0 0.4rem;", "Sector" }
                    p { style: "color: #b00020;", "{e}" }
                },
                Some(Ok(detail)) => rsx! { SectorDetailBody { detail: detail.clone() } },
            }
        }
    }
}

#[component]
fn SectorDetailBody(detail: SectorDetail) -> Element {
    let card = PAGE_CARD;
    let mcap = fmt_mcap(detail.total_market_cap);
    rsx! {
        h1 { style: "margin: 0.8rem 0 0.25rem;", "{detail.sector}" }
        p { style: "color: #555; margin: 0 0 1rem;",
            "{detail.company_count} companies · {detail.with_snapshot_count} with snapshots · {mcap} total mcap"
        }
        SectorResearchFull { profile: detail.research.clone() }

        section { style: "{card} margin-top: 0.5rem;",
            h3 { style: "margin-top: 0;", "Constituents (by market cap)" }
            if detail.members.is_empty() {
                p { style: "margin: 0; color: #667;", "No snapshot-backed members yet." }
            } else {
                table { style: "width: 100%; border-collapse: collapse; font-size: 0.92rem;",
                    thead {
                        tr { style: "text-align: left; border-bottom: 1px solid #dfe3eb; color: #556;",
                            th { style: "padding: 0.4rem;", "Symbol" }
                            th { style: "padding: 0.4rem;", "Name" }
                            th { style: "padding: 0.4rem;", "Mcap" }
                        }
                    }
                    tbody {
                        for m in detail.members.clone() {
                            tr { style: "border-bottom: 1px solid #eef1f5;",
                                td { style: "padding: 0.45rem;",
                                    Link {
                                        to: Route::Report { symbol: m.symbol.clone() },
                                        style: "color: #184ad8; text-decoration: none; font-weight: 600;",
                                        "{m.symbol}"
                                    }
                                }
                                td { style: "padding: 0.45rem;", "{m.short_name.clone().unwrap_or_default()}" }
                                td { style: "padding: 0.45rem;", "{fmt_mcap(m.market_cap)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
