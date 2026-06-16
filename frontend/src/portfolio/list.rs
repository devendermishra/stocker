use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio::styles::{BTN_DANGER, BTN_PRIMARY, CARD, CONFIRM_BTN, CONFIRM_BOX, CANCEL_BTN};
use super::labels::delete_success_message;
use crate::portfolio_api::{create_portfolio, delete_portfolio, list_portfolios, NewPortfolio, Portfolio};
use crate::routes::Route;


const DELETE_BTN: &str = BTN_DANGER;

#[derive(Clone, PartialEq)]
struct PendingPortfolioDelete {
    id: i64,
    name: String,
}

#[component]
pub fn PortfolioList() -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut new_name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut success_msg = use_signal(|| None::<String>);
    let mut pending_delete = use_signal(|| None::<PendingPortfolioDelete>);

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
                        PortfolioCard {
                            key: "{p.id}",
                            portfolio: p.clone(),
                            pending_delete,
                            error,
                            success_msg,
                        }
                    }
                }
            }
        }
    };

    rsx! {
        AuthGuard {
            h1 { "Portfolios" }
            p { style: "color: #666; font-size: 0.9rem; margin: 0 0 1rem 0;",
                "Each portfolio has a matching label with the same name. Deleting either one removes the portfolio, its label, and all transactions."
            }
            div { style: "display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap;",
                input {
                    placeholder: "New portfolio name",
                    style: "padding: 0.5rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; flex: 1; min-width: 200px;",
                    value: "{new_name}",
                    oninput: move |ev| new_name.set(ev.value()),
                }
                button {
                    style: "{BTN_PRIMARY}",
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
                                Ok(_) => {
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
            if let Some(msg) = success_msg() {
                p { style: "color: #0d6b2d;", {msg} }
            }
            if let Some(pending) = pending_delete() {
                PortfolioDeleteConfirm {
                    pending: pending.clone(),
                    pending_delete,
                    reload,
                    error,
                    success_msg,
                }
            }
            {list_body}
        }
    }
}

#[component]
fn PortfolioDeleteConfirm(
    pending: PendingPortfolioDelete,
    mut pending_delete: Signal<Option<PendingPortfolioDelete>>,
    mut reload: Signal<u32>,
    mut error: Signal<Option<String>>,
    mut success_msg: Signal<Option<String>>,
) -> Element {
    rsx! {
        div {
            style: "{CONFIRM_BOX}",
            p { style: "margin: 0 0 0.75rem 0;",
                strong { "Delete portfolio \"{pending.name}\"?" }
            }
            p { style: "margin: 0 0 0.75rem 0; color: #b00020; font-size: 0.9rem;",
                "This permanently removes the portfolio, its matching label, and all transactions."
            }
            div { style: "display: flex; gap: 0.5rem;",
                button {
                    style: "{CONFIRM_BTN}",
                    onclick: move |_| {
                        let pid = pending.id;
                        let portfolio_name = pending.name.clone();
                        let mut error = error;
                        spawn(async move {
                            match delete_portfolio(pid).await {
                                Ok(result) => {
                                    pending_delete.set(None);
                                    reload.set(reload() + 1);
                                    error.set(None);
                                    success_msg.set(Some(delete_success_message(
                                        &portfolio_name,
                                        result.transactions_deleted,
                                        result.portfolios_deleted,
                                    )));
                                }
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    "Confirm delete"
                }
                button {
                    style: "{CANCEL_BTN}",
                    onclick: move |_| pending_delete.set(None),
                    "Cancel"
                }
            }
        }
    }
}

#[component]
fn PortfolioCard(
    portfolio: Portfolio,
    mut pending_delete: Signal<Option<PendingPortfolioDelete>>,
    mut error: Signal<Option<String>>,
    mut success_msg: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "{CARD}",
            h3 { style: "margin: 0 0 0.35rem;", "{portfolio.name}" }
            if let Some(d) = &portfolio.description {
                p { style: "margin: 0 0 0.75rem; color: #666; font-size: 0.9rem;", "{d}" }
            }
            div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap;",
                Link {
                    to: Route::PortfolioOverview { id: portfolio.id },
                    style: "{BTN_PRIMARY}; text-decoration: none; display: inline-block;",
                    "Open"
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
                button {
                    style: "{DELETE_BTN}",
                    onclick: move |_| {
                        error.set(None);
                        success_msg.set(None);
                        pending_delete.set(Some(PendingPortfolioDelete {
                            id: portfolio.id,
                            name: portfolio.name.clone(),
                        }));
                    },
                    "Delete"
                }
            }
        }
    }
}
