use dioxus::prelude::*;

use crate::portfolio::AuthGuard;
use crate::portfolio_api::{fmt_inr, fmt_pct, fmt_return_pct, Holding};
use crate::routes::Route;
use crate::sync_portfolio::layout::{SyncPortfolioBanner, SyncPortfolioNav, SyncPortfolioTab};
use crate::sync_portfolio_api::{remote_holdings, sync_remote_exported_at};

fn is_mutual_fund_holding(h: &Holding) -> bool {
    h.asset_class.as_deref() == Some("mutual_fund") || h.symbol.starts_with("MF:")
}

#[component]
pub fn SyncPortfolioHoldings(id: i64) -> Element {
    let exported_at = use_resource(|| async move { sync_remote_exported_at().await.ok().flatten() });
    let data = use_resource(move || async move { remote_holdings(id).await });

    let body = match &*data.read() {
        None => rsx! { p { "Loading…" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) => render_holdings_sections(id, list),
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::DriveSync {}, style: "color: #184ad8;", "← Google Drive Sync" }
            }
            if let Some(Some(ts)) = exported_at.read().as_ref() {
                SyncPortfolioBanner { exported_at: Some(ts.clone()) }
            }
            SyncPortfolioNav { id, active: SyncPortfolioTab::Holdings }
            h1 { style: "margin-top: 0;", "Holdings" }
            {body}
        }
    }
}

fn render_holdings_sections(id: i64, list: &[Holding]) -> Element {
    if list.is_empty() {
        return rsx! { p { "No current holdings" } };
    }
    let equity: Vec<_> = list.iter().filter(|h| !is_mutual_fund_holding(h)).collect();
    let mutual_funds: Vec<_> = list.iter().filter(|h| is_mutual_fund_holding(h)).collect();
    rsx! {
        if !equity.is_empty() {
            h2 { style: "font-size: 1.1rem; margin: 0 0 0.75rem;", "Equity" }
            {render_holdings_table(id, &equity, false)}
        }
        if !mutual_funds.is_empty() {
            h2 { style: "font-size: 1.1rem; margin: 1.5rem 0 0.75rem;", "Mutual funds" }
            {render_holdings_table(id, &mutual_funds, true)}
        }
    }
}

fn render_holdings_table(id: i64, list: &[&Holding], is_mf: bool) -> Element {
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
                        th { style: "padding: 0.5rem;", "Return %" }
                        th { style: "padding: 0.5rem;", "Weight" }
                        if !is_mf {
                            th { style: "padding: 0.5rem;", "Sector" }
                        }
                    }
                }
                tbody {
                    for h in list.iter() {
                        tr { style: "border-top: 1px solid #eee;",
                            td { style: "padding: 0.5rem;",
                                Link {
                                    to: Route::SyncPortfolioStockDetail { id, symbol: h.symbol.clone() },
                                    style: "color: #184ad8;",
                                    "{h.short_name.clone().unwrap_or(h.symbol.clone())}"
                                }
                            }
                            td { style: "padding: 0.5rem;", "{h.quantity:.4}" }
                            td { style: "padding: 0.5rem;", "{fmt_inr(h.average_cost)}" }
                            td { style: "padding: 0.5rem;", "{fmt_inr(h.total_cost)}" }
                            td { style: "padding: 0.5rem;", "{h.current_price.map(fmt_inr).unwrap_or_default()}" }
                            if is_mf {
                                td { style: "padding: 0.5rem;", "{h.nav_date.clone().unwrap_or_default()}" }
                            }
                            td { style: "padding: 0.5rem;", "{h.current_value.map(fmt_inr).unwrap_or_default()}" }
                            td { style: "padding: 0.5rem;",
                                "{h.unrealized_gain.map(fmt_inr).unwrap_or_default()}"
                            }
                            td { style: "padding: 0.5rem;", "{fmt_return_pct(h.total_return_pct, h.return_method.as_deref())}" }
                            td { style: "padding: 0.5rem;", "{h.portfolio_weight.map(fmt_pct).unwrap_or_default()}" }
                            if !is_mf {
                                td { style: "padding: 0.5rem;", "{h.sector.clone().unwrap_or_default()}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
