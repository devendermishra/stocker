//! Stock list page: search SQLite universe by name and exchange.

use dioxus::prelude::*;

use crate::routes::Route;
use crate::screener_api::{search_symbols, status, SymbolListing};

const CARD: &str =
    "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";

async fn sleep_ms(ms: u32) {
    #[cfg(feature = "web")]
    {
        gloo_timers::future::TimeoutFuture::new(ms).await;
    }
    #[cfg(feature = "desktop")]
    {
        tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
    }
}

#[component]
pub fn Stocks() -> Element {
    let mut search = use_signal(String::new);
    let mut exchange_filter = use_signal(|| String::from("All"));
    let mut results = use_signal(Vec::<SymbolListing>::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut reload = use_signal(|| 0u32);

    let status_res = use_resource(|| async { status().await });

    use_effect(move || {
        let q = search();
        let ex = exchange_filter();
        let _ = reload();
        spawn(async move {
            loading.set(true);
            error.set(None);
            sleep_ms(300).await;
            let ex_arg = if ex == "All" { None } else { Some(ex.as_str()) };
            match search_symbols(&q, ex_arg, 100).await {
                Ok(rows) => results.set(rows),
                Err(e) => {
                    results.set(Vec::new());
                    error.set(Some(e));
                }
            }
            loading.set(false);
        });
    });

    let universe_empty = status_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .is_some_and(|s| s.universe_size == 0);

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1060px; margin: 1.5rem auto; padding: 0 1rem 2rem;",
            Link {
                to: Route::Home { id: String::new(), exchange: String::new() },
                style: "color: #184ad8;",
                "← Home"
            }
            h1 { style: "margin: 0.8rem 0 0.4rem;", "Stock List" }
            p { style: "color: #555; margin: 0 0 1rem;",
                "Search the screener universe by company name or symbol. Select a row to open the report."
            }

            if universe_empty {
                div {
                    style: "{CARD} margin-bottom: 1rem; background: #fff8e1; border-color: #f0c14b;",
                    p { style: "margin: 0; color: #5a4300;",
                        "No symbols in the database yet. Run universe sync with NSE and BSE CSV files to populate the stock list."
                    }
                }
            }

            div {
                style: "{CARD} margin-bottom: 1rem; display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: center;",
                input {
                    style: "flex: 1; min-width: 200px; padding: 0.55rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                    placeholder: "Search by name or symbol…",
                    value: "{search}",
                    oninput: move |e| {
                        search.set(e.value());
                        reload.set(reload() + 1);
                    },
                }
                select {
                    style: "padding: 0.55rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                    value: "{exchange_filter}",
                    onchange: move |e| {
                        exchange_filter.set(e.value());
                        reload.set(reload() + 1);
                    },
                    option { value: "All", "All exchanges" }
                    option { value: "NSE", "NSE" }
                    option { value: "BSE", "BSE" }
                }
            }

            if loading() {
                p { "Loading…" }
            } else if let Some(err) = error.read().as_ref() {
                p { style: "color: #b00020;", "{err}" }
            } else if results.read().is_empty() {
                p { style: "color: #555;", "No symbols found." }
            } else {
                div { style: "{CARD} overflow-x: auto;",
                    table {
                        style: "width: 100%; border-collapse: collapse; font-size: 0.92rem;",
                        thead {
                            tr {
                                style: "border-bottom: 2px solid #dfe3eb; text-align: left;",
                                th { style: "padding: 0.5rem 0.65rem;", "Name" }
                                th { style: "padding: 0.5rem 0.65rem;", "Id" }
                                th { style: "padding: 0.5rem 0.65rem;", "Exchange" }
                                th { style: "padding: 0.5rem 0.65rem;", "Sector" }
                                th { style: "padding: 0.5rem 0.65rem;", "" }
                            }
                        }
                        tbody {
                            for row in results.read().iter() {
                                {
                                    let stock_id = row.id.clone();
                                    let ex = row.exchange.clone().unwrap_or_else(|| "NSE".to_string());
                                    let name = row.short_name.clone().unwrap_or_else(|| row.symbol.clone());
                                    let sector = row.sector.clone().unwrap_or_else(|| "—".to_string());
                                    let sym = row.symbol.clone();
                                    rsx! {
                                        tr {
                                            key: "{row.symbol}",
                                            style: "border-bottom: 1px solid #eef1f6;",
                                            td { style: "padding: 0.5rem 0.65rem;", "{name}" }
                                            td { style: "padding: 0.5rem 0.65rem;", "{stock_id}" }
                                            td { style: "padding: 0.5rem 0.65rem;", "{ex}" }
                                            td { style: "padding: 0.5rem 0.65rem;", "{sector}" }
                                            td { style: "padding: 0.5rem 0.65rem;",
                                                Link {
                                                    to: Route::Report { symbol: sym },
                                                    style: "padding: 0.35rem 0.75rem; background: #184ad8; color: white; border-radius: 6px; text-decoration: none; font-weight: 600; font-size: 0.85rem; white-space: nowrap;",
                                                    "Select"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
