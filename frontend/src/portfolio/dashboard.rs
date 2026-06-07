use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{allocation_label, allocation_stock, dashboard, fmt_inr, fmt_pct, rebuild_portfolio, export_holdings_url, export_transactions_url, txn_type_label};
use crate::routes::Route;

const CARD: &str = "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";
const BTN: &str = "padding: 0.45rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; background: #fff; cursor: pointer; font-size: 0.85rem;";

#[component]
pub fn PortfolioDashboard(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut msg = use_signal(|| None::<String>);

    let dash = use_resource(move || {
        let _ = reload();
        async move { dashboard(id).await }
    });
    let alloc_stock = use_resource(move || async move { allocation_stock(id).await });
    let alloc_label = use_resource(move || async move { allocation_label(id).await });

    rsx! {
        AuthGuard {
            div { style: "display: flex; gap: 0.5rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
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
                None => rsx! { p { "Loading" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(d)) => rsx! {
                    h1 { "{d.portfolio.name}" }
                    div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 0.75rem; margin-bottom: 1.25rem;",
                        SummaryCard { label: "Invested", value: fmt_inr(d.summary.invested_amount) }
                        SummaryCard { label: "Market value", value: fmt_inr(d.summary.current_market_value) }
                        SummaryCard { label: "Unrealized", value: fmt_inr(d.summary.unrealized_gain) }
                        SummaryCard { label: "Realized", value: fmt_inr(d.summary.realized_gain) }
                        SummaryCard { label: "Dividends", value: fmt_inr(d.summary.dividend_received) }
                        SummaryCard { label: "Total return", value: fmt_inr(d.summary.total_return) }
                        SummaryCard { label: "Return %", value: fmt_pct(d.summary.total_return_pct) }
                        SummaryCard { label: "Holdings", value: d.summary.holdings_count.to_string() }
                    }
                    h2 { "Top holdings" }
                    div { style: "overflow-x: auto; margin-bottom: 1.25rem;",
                        table { style: "width: 100%; border-collapse: collapse; font-size: 0.9rem;",
                            thead {
                                tr { style: "background: #f6f8fb; text-align: left;",
                                    th { style: "padding: 0.5rem;", "Symbol" }
                                    th { style: "padding: 0.5rem;", "Qty" }
                                    th { style: "padding: 0.5rem;", "Value" }
                                    th { style: "padding: 0.5rem;", "Return" }
                                }
                            }
                            tbody {
                                for h in d.top_holdings.iter() {
                                    tr { style: "border-top: 1px solid #eee;",
                                        td { style: "padding: 0.5rem;",
                                            Link {
                                                to: Route::PortfolioStockDetail { id, symbol: h.symbol.clone() },
                                                style: "color: #1a56db;",
                                                "{h.short_name.clone().unwrap_or(h.symbol.clone())}"
                                            }
                                        }
                                        td { style: "padding: 0.5rem;", "{h.quantity:.0}" }
                                        td { style: "padding: 0.5rem;", "{h.current_value.map(fmt_inr).unwrap_or_default()}" }
                                        td { style: "padding: 0.5rem;", "{h.total_return.map(fmt_inr).unwrap_or_default()}" }
                                    }
                                }
                            }
                        }
                    }
                    h2 { "Recent transactions" }
                    for t in d.recent_transactions.iter().take(5) {
                        p { style: "margin: 0.25rem 0; font-size: 0.9rem; color: #444;",
                            "{t.trade_date} — {txn_type_label(&t.txn_type)} — {t.symbol.clone().unwrap_or_default()}"
                        }
                    }
                },
            }
            h2 { style: "margin-top: 1.5rem;", "Allocation by stock" }
            AllocationTable { data: alloc_stock.read_unchecked().clone() }
            h2 { style: "margin-top: 1.5rem;", "Allocation by label" }
            AllocationTable { data: alloc_label.read_unchecked().clone() }
        }
    }
}

#[component]
fn SummaryCard(label: String, value: String) -> Element {
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
        None => rsx! { p { "Loading" } },
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
