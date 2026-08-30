use dioxus::prelude::*;

use crate::routes::Route;
use crate::screener_api::resolve_report_ticker;

fn simple_ticker(id: &str, exchange: &str) -> Option<String> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    let ex = exchange.trim().to_uppercase();
    if ex == "BSE" {
        Some(format!("{}.BO", id.to_uppercase()))
    } else {
        Some(format!("{}.NS", id.to_uppercase()))
    }
}

#[component]
pub fn Home(id: String, exchange: String) -> Element {
    let init_id = if id.trim().is_empty() {
        String::from("RELIANCE")
    } else {
        id.clone()
    };
    let init_exchange = if exchange.trim().is_empty() {
        String::from("NSE")
    } else {
        exchange.to_uppercase()
    };

    let mut stock_id = use_signal(|| init_id);
    let mut selected_exchange = use_signal(|| init_exchange);
    let mut resolved_ticker = use_signal(|| None::<String>);

    use_effect(move || {
        if !id.trim().is_empty() {
            stock_id.set(id.clone());
        }
        if !exchange.trim().is_empty() {
            selected_exchange.set(exchange.to_uppercase());
        }
    });

    use_effect(move || {
        let sid = stock_id();
        let ex = selected_exchange();
        spawn(async move {
            let ticker = resolve_report_ticker(&sid, &ex).await;
            resolved_ticker.set(if ticker.is_empty() { None } else { Some(ticker) });
        });
    });

    let fallback = simple_ticker(&stock_id(), &selected_exchange());
    let report_symbol = resolved_ticker().or(fallback);
    let can_report = report_symbol.is_some();

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 760px; margin: 2rem auto; padding: 0 1rem;",
            h1 { "NSE Stock Researcher" }
            p { style: "color: #555;", "Professional summary from Yahoo-derived data with heuristics. Not investment advice." }

            div {
                style: "display: flex; gap: 0.75rem; margin-top: 1.25rem; flex-wrap: wrap; align-items: flex-end;",
                label {
                    style: "display: flex; flex-direction: column; gap: 0.35rem; font-size: 0.9rem; font-weight: 600; color: #333; flex: 1; min-width: 200px;",
                    "Id"
                    input {
                        style: "padding: 0.55rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; font-weight: 400;",
                        placeholder: "RELIANCE",
                        value: "{stock_id}",
                        oninput: move |e| stock_id.set(e.value()),
                    }
                }
                label {
                    style: "display: flex; flex-direction: column; gap: 0.35rem; font-size: 0.9rem; font-weight: 600; color: #333;",
                    "Exchange"
                    select {
                        style: "padding: 0.55rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; font-weight: 400; min-width: 100px;",
                        value: "{selected_exchange}",
                        onchange: move |e| selected_exchange.set(e.value()),
                        option { value: "NSE", "NSE" }
                        option { value: "BSE", "BSE" }
                    }
                }
            }

            div { style: "display: flex; gap: 0.5rem; margin-top: 1rem; flex-wrap: wrap; align-items: center;",
                if let Some(sym) = report_symbol.clone() {
                    Link {
                        to: Route::Report { symbol: sym },
                        style: "padding: 0.55rem 1rem; background: #184ad8; color: white; border-radius: 8px; text-decoration: none; font-weight: 600;",
                        "Generate Report"
                    }
                } else {
                    span {
                        style: "padding: 0.55rem 1rem; background: #aab4c4; color: white; border-radius: 8px; font-weight: 600;",
                        "Generate Report"
                    }
                }
                Link {
                    to: Route::Stocks {},
                    style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Browse stocks"
                }
                Link {
                    to: Route::Screener {},
                    style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Open Screener"
                }
                Link {
                    to: Route::SectorsList {},
                    style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Sector Research"
                }
                Link {
                    to: Route::PortfolioList {},
                    style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Portfolio"
                }
                Link {
                    to: Route::DriveSync {},
                    style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Sync"
                }
            }
            if !can_report {
                p { style: "color: #b00020; font-size: 0.88rem; margin-top: 0.5rem;",
                    "Enter a symbol id to generate a report."
                }
            }
        }
    }
}
