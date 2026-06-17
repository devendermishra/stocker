use dioxus::prelude::*;

use crate::portfolio::layout::{AuthGuard, PortfolioNav, PortfolioTab};
use crate::portfolio_api::{allocation_label, allocation_stock, dashboard, fmt_inr, fmt_pct, rebuild_portfolio, refresh_prices, export_holdings_url, export_transactions_url};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

const CARD: &str = "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";
const BTN: &str = "padding: 0.45rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; background: #fff; cursor: pointer; font-size: 0.85rem;";

#[component]
pub fn PortfolioDashboard(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut msg = use_signal(|| None::<String>);

    let dash = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { dashboard(id).await }
    });
    let alloc_stock = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { allocation_stock(id).await }
    });
    let alloc_label = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { allocation_label(id).await }
    });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            PortfolioNav { id, active: PortfolioTab::Dashboard }
            div { style: "display: flex; gap: 0.5rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap;",
                button {
                    style: "{BTN}",
                    onclick: move |_| {
                        spawn(async move {
                            match rebuild_portfolio(id).await {
                                Ok(()) => {
                                    msg.set(Some("Portfolio recalculated.".into()));
                                    reload.set(reload() + 1);
                                }
                                Err(e) => msg.set(Some(e)),
                            }
                        });
                    },
                    "Recalculate"
                }
                button {
                    style: "{BTN}",
                    onclick: move |_| {
                        spawn(async move {
                            match refresh_prices(id).await {
                                Ok(()) => {
                                    msg.set(Some("Prices refreshed.".into()));
                                    reload.set(reload() + 1);
                                }
                                Err(e) => msg.set(Some(e)),
                            }
                        });
                    },
                    "Refresh prices"
                }
                a {
                    href: "{export_holdings_url(id)}",
                    style: "{BTN}; text-decoration: none; color: #333;",
                    "Export holdings CSV"
                }
                a {
                    href: "{export_transactions_url(id)}",
                    style: "{BTN}; text-decoration: none; color: #333;",
                    "Export transactions CSV"
                }
            }
            if let Some(m) = msg() {
                p { style: "color: #1b5e20; margin-bottom: 0.75rem;", "{m}" }
            }
            match &*dash.read_unchecked() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(d)) => rsx! {
                    h1 { style: "margin-top: 0;", "{d.portfolio.name}" }
                    p { style: "color: #666; margin-bottom: 1.25rem; font-size: 0.9rem;",
                        "Allocation breakdown and portfolio tools."
                    }
                },
            }
            h2 { style: "margin-top: 0.5rem;", "Allocation by stock" }
            AllocationTable { data: alloc_stock.read_unchecked().clone() }
            h2 { style: "margin-top: 1.5rem;", "Allocation by label" }
            AllocationTable { data: alloc_label.read_unchecked().clone() }
        }
    }
}

pub fn return_pct_label(method: Option<&str>) -> String {
    match method {
        Some("xirr") => "XIRR".to_string(),
        Some("cagr") => "CAGR".to_string(),
        _ => "Return %".to_string(),
    }
}

#[component]
pub fn SummaryCard(label: String, value: String) -> Element {
    rsx! {
        div { style: "{CARD}",
            div { style: "font-size: 0.8rem; color: #666;", "{label}" }
            div { style: "font-size: 1.15rem; font-weight: 700; margin-top: 0.25rem;", "{value}" }
        }
    }
}

#[component]
fn AllocationTable(data: Option<Result<Vec<crate::portfolio_api::AllocationRow>, String>>) -> Element {
    match data {
        None => rsx! { p { "Loading…" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(rows)) if rows.is_empty() => rsx! { p { "No data." } },
        Some(Ok(rows)) => rsx! {
            div { style: "overflow-x: auto;",
                table { style: "width: 100%; border-collapse: collapse; font-size: 0.9rem;",
                    thead {
                        tr { style: "background: #f6f8fb; text-align: left;",
                            th { style: "padding: 0.5rem;", "Name" }
                            th { style: "padding: 0.5rem;", "Weight" }
                            th { style: "padding: 0.5rem;", "Value" }
                            th { style: "padding: 0.5rem;", "Return" }
                        }
                    }
                    tbody {
                        for r in rows.iter() {
                            tr { style: "border-top: 1px solid #eee;",
                                td { style: "padding: 0.5rem;", "{r.label}" }
                                td { style: "padding: 0.5rem;", "{fmt_pct(r.weight_pct)}" }
                                td { style: "padding: 0.5rem;", "{fmt_inr(r.current_value)}" }
                                td { style: "padding: 0.5rem;", "{fmt_inr(r.total_return)}" }
                            }
                        }
                    }
                }
            }
        },
    }
}
