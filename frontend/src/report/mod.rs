mod tabs;

use std::sync::Arc;

use dioxus::prelude::*;

use crate::api::load_research_report;
use crate::format::fmt_price_in_currency;
use crate::routes::Route;
use crate::screener_api::{list_fields, load_snapshot, refresh_symbol, CatalogEntry, ScreenRow};

use tabs::{
    company_news_tab, financials_tab, framework_tab, management_tab, overview_tab, peers_tab,
    research_tab, sector_tab, StockDetailedInformation,
};

pub const CARD: &str =
    "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";

#[component]
pub fn Report(symbol: String) -> Element {
    let mut tab = use_signal(|| 0i32);
    let sym_arc = Arc::new(symbol);
    let heading = sym_arc.as_ref().clone();

    let catalog_res = use_resource(|| async { list_fields().await });
    let mut metrics_reload = use_signal(|| 0u32);
    let mut metrics_refreshing = use_signal(|| false);
    let mut metrics_refresh_err = use_signal(|| None::<String>);

    let sym_metrics = sym_arc.clone();
    let sym_report = sym_arc.clone();

    let metrics_res = use_resource(move || {
        let sym = sym_metrics.as_ref().clone();
        let _n = metrics_reload();
        async move { load_snapshot(&sym).await }
    });

    let resource = use_resource(move || {
        let sym = sym_report.as_ref().clone();
        async move { load_research_report(sym).await }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1060px; margin: 1.5rem auto; padding: 0 1rem 2rem;",
            Link { to: Route::Home {}, style: "color: #184ad8;", "← Home" }
            h1 { style: "margin: 0.8rem 0 0.4rem;", "Research Report: {heading}" }
            match &*resource.read() {
                None => rsx! { p { "Loading..." } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", "{e}" } },
                Some(Ok(r)) => {
                    let title = r.long_name.clone().unwrap_or_else(|| r.symbol.clone());
                    let card = CARD;
                    let catalog: Vec<CatalogEntry> = catalog_res
                        .read()
                        .as_ref()
                        .and_then(|x| x.as_ref().ok())
                        .cloned()
                        .unwrap_or_default();
                    let snapshot: Option<ScreenRow> = metrics_res
                        .read()
                        .as_ref()
                        .and_then(|x| x.as_ref().ok())
                        .cloned()
                        .flatten();
                    let metrics_load_err = metrics_res
                        .read()
                        .as_ref()
                        .and_then(|x| x.as_ref().err().cloned());
                    let sym_for_refresh = sym_arc.as_ref().clone();
                    rsx! {
                        div { style: "display: grid; grid-template-columns: repeat(auto-fit,minmax(180px,1fr)); gap: 0.6rem; margin: 0.75rem 0 1rem;",
                            div { style: "{card}", h3 { style: "margin: 0 0 0.25rem; font-size: 0.92rem; color:#555;", "Company" } p { style: "margin:0; font-weight: 600;", "{title}" } }
                            div { style: "{card}", h3 { style: "margin: 0 0 0.25rem; font-size: 0.92rem; color:#555;", "Last Price" } p { style: "margin:0; font-weight: 700;", "{fmt_price_in_currency(r.price, r.asset_profile.currency.as_deref())}" } }
                            div { style: "{card}", h3 { style: "margin: 0 0 0.25rem; font-size: 0.92rem; color:#555;", "Sector / Industry" } p { style: "margin:0;", "{r.asset_profile.sector.clone().unwrap_or_else(|| \"N/A\".to_string())} / {r.asset_profile.industry.clone().unwrap_or_else(|| \"N/A\".to_string())}" } }
                            div { style: "{card}", h3 { style: "margin: 0 0 0.25rem; font-size: 0.92rem; color:#555;", "Valuation Label" } p { style: "margin:0;", "{r.stock_analysis.valuation_label}" } }
                        }

                        div { style: "display: flex; gap: 0.35rem; margin: 0.7rem 0 1rem; flex-wrap: wrap; border-bottom: 1px solid #ddd; padding-bottom: 0.6rem;",
                            for (i, label) in ["Overview", "Research", "Financials", "Detailed Data", "Sector", "Peers", "News", "Management", "Framework"].iter().enumerate() {
                                button {
                                    style: if tab() == i as i32 {
                                        "padding: 0.38rem 0.8rem; border: none; background: #184ad8; color: white; border-radius: 8px; cursor: pointer;"
                                    } else {
                                        "padding: 0.38rem 0.8rem; border: 1px solid #cad1de; background: #fff; border-radius: 8px; cursor: pointer;"
                                    },
                                    onclick: move |_| tab.set(i as i32),
                                    "{label}"
                                }
                            }
                        }

                        match tab() {
                            0 => rsx! { {overview_tab(r)} },
                            1 => rsx! { {research_tab(r)} },
                            2 => rsx! { {financials_tab(r)} },
                            3 => rsx! {
                                StockDetailedInformation {
                                    symbol: sym_for_refresh.clone(),
                                    catalog: catalog.clone(),
                                    snapshot: snapshot.clone(),
                                    load_error: metrics_load_err.clone(),
                                    refreshing: metrics_refreshing(),
                                    refresh_error: metrics_refresh_err().clone(),
                                    on_refresh: move |_| {
                                        let sym = sym_for_refresh.clone();
                                        spawn(async move {
                                            metrics_refreshing.set(true);
                                            metrics_refresh_err.set(None);
                                            match refresh_symbol(&sym).await {
                                                Ok(()) => metrics_reload.set(metrics_reload() + 1),
                                                Err(e) => metrics_refresh_err.set(Some(e)),
                                            }
                                            metrics_refreshing.set(false);
                                        });
                                    },
                                }
                            },
                            4 => rsx! { {sector_tab(r)} },
                            5 => rsx! { {peers_tab(r)} },
                            6 => rsx! { {company_news_tab(r)} },
                            7 => rsx! { {management_tab(r)} },
                            _ => rsx! { {framework_tab(r)} },
                        }
                    }
                }
            }
        }
    }
}
