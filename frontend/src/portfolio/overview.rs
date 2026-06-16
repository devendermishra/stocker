use dioxus::prelude::*;

use crate::portfolio::dashboard::{return_pct_label, SummaryCard};
use crate::portfolio::layout::{AuthGuard, PortfolioNav, PortfolioTab};
use crate::portfolio_api::{dashboard, fmt_inr, fmt_pct, fmt_return_pct, txn_type_label};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

#[component]
pub fn PortfolioOverview(id: i64) -> Element {
    let dash = use_resource(move || {
        let _ = portfolio_data_revision();
        async move { dashboard(id).await }
    });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            PortfolioNav { id, active: PortfolioTab::Overview }
            match &*dash.read_unchecked() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(d)) => rsx! {
                    h1 { style: "margin-top: 0;", "{d.portfolio.name}" }
                    div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 0.75rem; margin-bottom: 1.25rem;",
                        SummaryCard { label: "Invested", value: fmt_inr(d.summary.invested_amount) }
                        SummaryCard { label: "Market value", value: fmt_inr(d.summary.current_market_value) }
                        SummaryCard { label: "Unrealized", value: fmt_inr(d.summary.unrealized_gain) }
                        SummaryCard { label: "Realized", value: fmt_inr(d.summary.realized_gain) }
                        SummaryCard { label: "Dividends", value: fmt_inr(d.summary.dividend_received) }
                        SummaryCard { label: "Total return", value: fmt_inr(d.summary.total_return) }
                        SummaryCard {
                            label: return_pct_label(d.summary.return_method.as_deref()),
                            value: fmt_pct(d.summary.total_return_pct)
                        }
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
                                    th { style: "padding: 0.5rem;", "Return %" }
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
                                        td { style: "padding: 0.5rem;", "{fmt_return_pct(h.total_return_pct, h.return_method.as_deref())}" }
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
                    Link {
                        to: Route::PortfolioTransactions { id },
                        style: "color: #1a56db; font-size: 0.9rem;",
                        "View all transactions →"
                    }
                },
            }
        }
    }
}
