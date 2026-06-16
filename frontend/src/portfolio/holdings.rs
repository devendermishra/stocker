use dioxus::prelude::*;

use crate::portfolio::layout::{AuthGuard, PortfolioNav, PortfolioTab};
use crate::portfolio_api::{detach_label, fmt_inr, fmt_pct, fmt_return_pct, holdings, Holding, Label};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

const DETACH_BTN: &str = "padding: 0 0.35rem; border: 1px solid #f5c6cb; background: #fff; color: #b00020; border-radius: 4px; cursor: pointer; font-size: 0.7rem; margin-left: 0.25rem; line-height: 1.2;";

#[component]
pub fn PortfolioHoldings(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let data = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { holdings(id).await }
    });

    let body = match &*data.read() {
        None => rsx! { p { "Loading…" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) => render_holdings_sections(id, list, reload),
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem; display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: center;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
                Link {
                    to: Route::PortfolioLabels {},
                    style: "padding: 0.35rem 0.65rem; border: 1px solid #d5dbe3; border-radius: 6px; text-decoration: none; color: #333; font-size: 0.85rem;",
                    "Manage labels"
                }
            }
            PortfolioNav { id, active: PortfolioTab::Holdings }
            h1 { style: "margin-top: 0;", "Holdings" }
            {body}
        }
    }
}

fn is_mutual_fund_holding(h: &Holding) -> bool {
    h.asset_class.as_deref() == Some("mutual_fund") || h.symbol.starts_with("MF:")
}

fn render_holdings_sections(id: i64, list: &[Holding], reload: Signal<u32>) -> Element {
    if list.is_empty() {
        return rsx! { p { "No current holdings" } };
    }

    let equity: Vec<_> = list.iter().filter(|h| !is_mutual_fund_holding(h)).collect();
    let mutual_funds: Vec<_> = list.iter().filter(|h| is_mutual_fund_holding(h)).collect();

    rsx! {
        if !equity.is_empty() {
            h2 { style: "font-size: 1.1rem; margin: 0 0 0.75rem;", "Equity" }
            {render_holdings_table(id, &equity, reload, false)}
        }
        if !mutual_funds.is_empty() {
            h2 { style: "font-size: 1.1rem; margin: 1.5rem 0 0.75rem;", "Mutual funds" }
            {render_holdings_table(id, &mutual_funds, reload, true)}
        }
    }
}

fn render_holdings_table(id: i64, list: &[&Holding], reload: Signal<u32>, is_mf: bool) -> Element {
    let price_label = if is_mf { "NAV" } else { "Price" };
    rsx! {
        div { style: "overflow-x: auto; margin-bottom: 0.5rem;",
            table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                thead {
                    tr { style: "background: #f6f8fb; text-align: left;",
                        th { style: "padding: 0.5rem;", "Holding" }
                        th { style: "padding: 0.5rem;", "Qty" }
                        th { style: "padding: 0.5rem;", "Avg cost" }
                        th { style: "padding: 0.5rem;", "Invested" }
                        th { style: "padding: 0.5rem;", "{price_label}" }
                        if is_mf {
                            th { style: "padding: 0.5rem;", "NAV date" }
                        }
                        th { style: "padding: 0.5rem;", "Value" }
                        th { style: "padding: 0.5rem;", "Unrealized" }
                        th { style: "padding: 0.5rem;", "Realized" }
                        th { style: "padding: 0.5rem;", "Dividend" }
                        th { style: "padding: 0.5rem;", "Total return" }
                        th { style: "padding: 0.5rem;", "Return %" }
                        th { style: "padding: 0.5rem;", "Weight" }
                        if !is_mf {
                            th { style: "padding: 0.5rem;", "Sector" }
                        }
                        th { style: "padding: 0.5rem;", "Labels" }
                    }
                }
                tbody {
                    for h in list.iter() {
                        HoldingsRow { key: "{h.symbol}", id, h: (*h).clone(), reload, is_mf }
                    }
                }
            }
        }
    }
}

#[component]
fn HoldingsRow(id: i64, h: Holding, reload: Signal<u32>, is_mf: bool) -> Element {
    let name = h.short_name.clone().unwrap_or(h.symbol.clone());
    let ug = h.unrealized_gain.map(fmt_inr).unwrap_or_default();
    let ugp = h.unrealized_gain_pct.map(fmt_pct).unwrap_or_default();
    let unrealized = if ug.is_empty() { ug } else { format!("{ug} ({ugp})") };
    let nav_date = h.nav_date.clone().unwrap_or_default();
    rsx! {
        tr { style: "border-top: 1px solid #eee;",
            td { style: "padding: 0.5rem;",
                Link {
                    to: Route::PortfolioStockDetail { id, symbol: h.symbol.clone() },
                    style: "color: #1a56db;",
                    "{name}"
                }
            }
            td { style: "padding: 0.5rem;", "{h.quantity:.4}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.average_cost)}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.total_cost)}" }
            td { style: "padding: 0.5rem;", "{h.current_price.map(fmt_inr).unwrap_or_default()}" }
            if is_mf {
                td { style: "padding: 0.5rem;", "{nav_date}" }
            }
            td { style: "padding: 0.5rem;", "{h.current_value.map(fmt_inr).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{unrealized}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.realized_gain)}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.dividend_received)}" }
            td { style: "padding: 0.5rem;", "{h.total_return.map(fmt_inr).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{fmt_return_pct(h.total_return_pct, h.return_method.as_deref())}" }
            td { style: "padding: 0.5rem;", "{h.portfolio_weight.map(fmt_pct).unwrap_or_default()}" }
            if !is_mf {
                td { style: "padding: 0.5rem;", "{h.sector.clone().unwrap_or_default()}" }
            }
            td { style: "padding: 0.5rem; min-width: 140px;",
                HoldingLabelsCell {
                    portfolio_id: id,
                    symbol: h.symbol.clone(),
                    labels: h.labels.clone(),
                    on_change: move || reload.set(reload() + 1),
                }
            }
        }
    }
}

#[component]
fn HoldingLabelChip(
    portfolio_id: i64,
    symbol: String,
    label: Label,
    on_change: EventHandler<()>,
) -> Element {
    rsx! {
        span {
            style: "display: inline-flex; align-items: center; padding: 0.15rem 0.45rem; border: 1px solid #d5dbe3; border-radius: 999px; background: #f6f8fb; font-size: 0.75rem;",
            "{label.name}"
            button {
                style: "{DETACH_BTN}",
                title: "Remove label from this holding",
                onclick: move |_| {
                    let entity_id = format!("{portfolio_id}:{symbol}");
                    let lid = label.id;
                    spawn(async move {
                        let _ = detach_label(lid, "holding", &entity_id).await;
                    });
                    on_change.call(());
                },
                "×"
            }
        }
    }
}

#[component]
fn HoldingLabelsCell(
    portfolio_id: i64,
    symbol: String,
    labels: Vec<Label>,
    on_change: EventHandler<()>,
) -> Element {
    if labels.is_empty() {
        return rsx! {
            Link {
                to: Route::PortfolioStockDetail { id: portfolio_id, symbol: symbol.clone() },
                style: "color: #888; font-size: 0.8rem;",
                "Attach"
            }
        };
    }
    rsx! {
        div { style: "display: flex; flex-wrap: wrap; gap: 0.25rem;",
            for label in labels.iter() {
                HoldingLabelChip {
                    key: "{label.id}",
                    portfolio_id,
                    symbol: symbol.clone(),
                    label: label.clone(),
                    on_change,
                }
            }
        }
    }
}
