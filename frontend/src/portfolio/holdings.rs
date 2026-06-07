use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{fmt_inr, fmt_pct, holdings, Holding};
use crate::routes::Route;

#[component]
pub fn PortfolioHoldings(id: i64) -> Element {
    let data = use_resource(move || async move { holdings(id).await });

    let body = match &*data.read_unchecked() {
        None => rsx! { p { "Loading" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) => render_holdings_table(id, list),
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 1rem;",
                Link { to: Route::PortfolioDashboard { id }, style: "color: #1a56db;", "← Dashboard" }
            }
            h1 { "Holdings" }
            {body}
        }
    }
}

fn render_holdings_table(id: i64, list: &[Holding]) -> Element {
    if list.is_empty() {
        return rsx! { p { "No current holdings" } };
    }
    rsx! {
        div { style: "overflow-x: auto;",
            table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                thead {
                    tr { style: "background: #f6f8fb; text-align: left;",
                        th { style: "padding: 0.5rem;", "Stock" }
                        th { style: "padding: 0.5rem;", "Qty" }
                        th { style: "padding: 0.5rem;", "Avg cost" }
                        th { style: "padding: 0.5rem;", "Invested" }
                        th { style: "padding: 0.5rem;", "Price" }
                        th { style: "padding: 0.5rem;", "Value" }
                        th { style: "padding: 0.5rem;", "Unrealized" }
                        th { style: "padding: 0.5rem;", "Realized" }
                        th { style: "padding: 0.5rem;", "Dividend" }
                        th { style: "padding: 0.5rem;", "Total return" }
                        th { style: "padding: 0.5rem;", "Weight" }
                        th { style: "padding: 0.5rem;", "Sector" }
                    }
                }
                tbody {
                    for h in list.iter() {
                        HoldingsRow { id, h: h.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn HoldingsRow(id: i64, h: Holding) -> Element {
    let name = h.short_name.clone().unwrap_or(h.symbol.clone());
    let ug = h.unrealized_gain.map(fmt_inr).unwrap_or_default();
    let ugp = h.unrealized_gain_pct.map(fmt_pct).unwrap_or_default();
    let unrealized = if ug.is_empty() { ug } else { format!("{ug} ({ugp})") };
    rsx! {
        tr { style: "border-top: 1px solid #eee;",
            td { style: "padding: 0.5rem;",
                Link {
                    to: Route::PortfolioStockDetail { id, symbol: h.symbol.clone() },
                    style: "color: #1a56db;",
                    "{name}"
                }
            }
            td { style: "padding: 0.5rem;", "{h.quantity:.2}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.average_cost)}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.total_cost)}" }
            td { style: "padding: 0.5rem;", "{h.current_price.map(fmt_inr).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{h.current_value.map(fmt_inr).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{unrealized}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.realized_gain)}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(h.dividend_received)}" }
            td { style: "padding: 0.5rem;", "{h.total_return.map(fmt_inr).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{h.portfolio_weight.map(fmt_pct).unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{h.sector.clone().unwrap_or_default()}" }
        }
    }
}
