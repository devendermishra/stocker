use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{create_portfolio, list_portfolios, NewPortfolio, Portfolio};
use crate::routes::Route;

const CARD: &str = "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 1rem; margin-bottom: 0.75rem;";
const BTN: &str = "padding: 0.5rem 0.85rem; background: #1a56db; color: #fff; border: none; border-radius: 8px; cursor: pointer;";

#[component]
pub fn PortfolioList() -> Element {
    let _nav = use_navigator();
    let mut reload = use_signal(|| 0u32);
    let mut new_name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let portfolios = use_resource(move || {
        let _ = reload();
        async move { list_portfolios(false).await }
    });

    let list_body = match &*portfolios.read_unchecked() {
        None => rsx! { p { "Loading" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) => {
            if list.is_empty() {
                rsx! { p { "No portfolios yet. Create one above." } }
            } else {
                rsx! {
                    for p in list.iter() {
                        PortfolioCard { portfolio: p.clone() }
                    }
                }
            }
        }
    };

    rsx! {
        AuthGuard {
            h1 { "Portfolios" }
            div { style: "display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap;",
                input {
                    placeholder: "New portfolio name",
                    style: "padding: 0.5rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; flex: 1; min-width: 200px;",
                    value: "{new_name}",
                    oninput: move |ev| new_name.set(ev.value()),
                }
                button {
                    style: "{BTN}",
                    onclick: move |_| {
                        let name = new_name();
                        spawn(async move {
                            if name.trim().is_empty() {
                                error.set(Some("Name required".into()));
                                return;
                            }
                            match create_portfolio(&NewPortfolio {
                                name: name.trim().to_string(),
                                description: None,
                                base_currency: Some("INR".into()),
                                portfolio_type: Some("mixed".into()),
                            }).await {
                                Ok(p) => {
                                    new_name.set(String::new());
                                    reload.set(reload() + 1);
                                    error.set(None);
                                }
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    "Create"
                }
                Link {
                    to: Route::PortfolioLabels {},
                    style: "padding: 0.5rem 0.85rem; border: 1px solid #d5dbe3; border-radius: 8px; text-decoration: none; color: #333;",
                    "Manage labels"
                }
            }
            if let Some(e) = error() {
                p { style: "color: #b00020;", {e} }
            }
            {list_body}
        }
    }
}

#[component]
fn PortfolioCard(portfolio: Portfolio) -> Element {
    rsx! {
        div { style: "{CARD}",
            h3 { style: "margin: 0 0 0.35rem;", "{portfolio.name}" }
            if let Some(d) = &portfolio.description {
                p { style: "margin: 0 0 0.75rem; color: #666; font-size: 0.9rem;", "{d}" }
            }
            div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap;",
                Link {
                    to: Route::PortfolioDashboard { id: portfolio.id },
                    style: "{BTN}; text-decoration: none; display: inline-block;",
                    "Dashboard"
                }
                Link {
                    to: Route::PortfolioHoldings { id: portfolio.id },
                    style: "padding: 0.5rem 0.85rem; border: 1px solid #d5dbe3; border-radius: 8px; text-decoration: none; color: #333;",
                    "Holdings"
                }
                Link {
                    to: Route::PortfolioTransactions { id: portfolio.id },
                    style: "padding: 0.5rem 0.85rem; border: 1px solid #d5dbe3; border-radius: 8px; text-decoration: none; color: #333;",
                    "Transactions"
                }
            }
        }
    }
}
