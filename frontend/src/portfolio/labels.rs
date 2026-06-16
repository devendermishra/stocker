use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{create_label, delete_label, list_labels, Label, NewLabel};
use crate::routes::Route;

const DELETE_BTN: &str = "padding: 0.25rem 0.5rem; border: 1px solid #f5c6cb; background: #fff; color: #b00020; border-radius: 6px; cursor: pointer; font-size: 0.8rem; flex-shrink: 0;";

#[derive(Clone, PartialEq)]
struct PendingLabelDelete {
    id: i64,
    name: String,
    transaction_count: i64,
}

async fn fetch_labels_into(
    mut labels_list: Signal<Option<Result<Vec<Label>, String>>>,
    mut error: Signal<Option<String>>,
) {
    match list_labels().await {
        Ok(list) => {
            error.set(None);
            labels_list.set(Some(Ok(list)));
        }
        Err(e) => {
            error.set(Some(e.clone()));
            labels_list.set(Some(Err(e)));
        }
    }
}

pub fn delete_success_message(name: &str, transactions_deleted: usize, portfolios_deleted: usize) -> String {
    let mut parts = vec![format!("Deleted \"{name}\"")];
    if portfolios_deleted > 0 {
        parts.push(format!(
            "{} portfolio{} removed",
            portfolios_deleted,
            if portfolios_deleted == 1 { "" } else { "s" }
        ));
    }
    if transactions_deleted > 0 {
        parts.push(format!("{transactions_deleted} transaction(s) removed"));
    }
    parts.join(", ") + "."
}

#[component]
fn LabelDeleteConfirm(
    pending: PendingLabelDelete,
    mut pending_delete: Signal<Option<PendingLabelDelete>>,
    mut reload: Signal<u32>,
    mut error: Signal<Option<String>>,
    mut success_msg: Signal<Option<String>>,
    mut labels_list: Signal<Option<Result<Vec<Label>, String>>>,
) -> Element {
    rsx! {
        div {
            style: "margin-bottom: 1rem; padding: 1rem; border: 1px solid #f5c6cb; border-radius: 8px; background: #fff8f8;",
            p { style: "margin: 0 0 0.75rem 0;",
                strong { "Delete \"{pending.name}\"?" }
            }
            p { style: "margin: 0 0 0.75rem 0; color: #b00020; font-size: 0.9rem;",
                "This removes the portfolio and its label. "
                if pending.transaction_count > 0 {
                    "All {pending.transaction_count} transaction(s) will be permanently deleted."
                } else {
                    "No transactions are linked to this portfolio."
                }
            }
            div { style: "display: flex; gap: 0.5rem;",
                button {
                    style: "padding: 0.45rem 0.75rem; background: #b00020; color: #fff; border: none; border-radius: 6px; cursor: pointer;",
                    onclick: move |_| {
                        let lid = pending.id;
                        let label_name = pending.name.clone();
                        let mut error = error;
                        spawn(async move {
                            match delete_label(lid).await {
                                Ok(result) => {
                                    pending_delete.set(None);
                                    reload.set(reload() + 1);
                                    error.set(None);
                                    fetch_labels_into(labels_list, error).await;
                                    success_msg.set(Some(delete_success_message(
                                        &label_name,
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
                    style: "padding: 0.45rem 0.75rem; border: 1px solid #d5dbe3; background: #fff; border-radius: 6px; cursor: pointer;",
                    onclick: move |_| pending_delete.set(None),
                    "Cancel"
                }
            }
        }
    }
}

fn format_attachments(label: &Label) -> String {
    let mut parts = Vec::new();
    if label.portfolio_count > 0 {
        parts.push(format!("{} portfolio(s)", label.portfolio_count));
    }
    if label.holding_count > 0 {
        parts.push(format!("{} holding(s)", label.holding_count));
    }
    if parts.is_empty() {
        "Portfolio only".to_string()
    } else {
        parts.join(", ")
    }
}

#[component]
fn LabelRow(
    label: Label,
    mut pending_delete: Signal<Option<PendingLabelDelete>>,
    mut error: Signal<Option<String>>,
    mut success_msg: Signal<Option<String>>,
) -> Element {
    rsx! {
        tr { style: "border-top: 1px solid #eee;",
            td { style: "padding: 0.5rem;", "{label.name}" }
            td { style: "padding: 0.5rem; color: #666;", "{format_attachments(&label)}" }
            td { style: "padding: 0.5rem; color: #666;",
                if label.transaction_count > 0 {
                    "{label.transaction_count}"
                } else if label.holding_count > 0 {
                    "0 (no txns for tagged symbols)"
                } else {
                    "0"
                }
            }
            td { style: "padding: 0.5rem; text-align: right; white-space: nowrap;",
                button {
                    style: "{DELETE_BTN}",
                    onclick: move |_| {
                        error.set(None);
                        success_msg.set(None);
                        pending_delete.set(Some(PendingLabelDelete {
                            id: label.id,
                            name: label.name.clone(),
                            transaction_count: label.transaction_count,
                        }));
                    },
                    "Delete"
                }
            }
        }
    }
}

#[component]
pub fn PortfolioLabels() -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut pending_delete = use_signal(|| None::<PendingLabelDelete>);
    let mut success_msg = use_signal(|| None::<String>);
    let mut labels_list = use_signal(|| None::<Result<Vec<Label>, String>>);

    let _loader = use_resource(move || {
        let _ = reload();
        let labels_list = labels_list;
        let error = error;
        async move {
            fetch_labels_into(labels_list, error).await;
        }
    });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 1rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            h1 { "Labels" }
            p { style: "color: #666; font-size: 0.9rem; margin: 0 0 1rem 0;",
                "Each label is a portfolio with the same name. Creating one here also creates the portfolio; deleting one removes the portfolio and all its transactions. "
                "You can still tag individual holdings from "
                strong { "Holdings → holding name → Attach label" }
                "."
            }
            div { style: "display: flex; gap: 0.5rem; margin-bottom: 1rem;",
                input {
                    placeholder: "Portfolio / label name",
                    style: "padding: 0.5rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; flex: 1;",
                    value: "{name}",
                    oninput: move |ev| name.set(ev.value()),
                }
                button {
                    style: "padding: 0.5rem 0.85rem; background: #1a56db; color: #fff; border: none; border-radius: 8px; cursor: pointer;",
                    onclick: move |_| {
                        let n = name();
                        let labels_list = labels_list;
                        let mut error = error;
                        spawn(async move {
                            if n.trim().is_empty() {
                                error.set(Some("Name required".into()));
                                return;
                            }
                            match create_label(&NewLabel { name: n.trim().to_string(), color: None }).await {
                                Ok(_) => {
                                    name.set(String::new());
                                    reload.set(reload() + 1);
                                    fetch_labels_into(labels_list, error).await;
                                }
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    "Add"
                }
            }
            if let Some(e) = error() {
                p { style: "color: #b00020;", {e} }
            }
            if let Some(msg) = success_msg() {
                p { style: "color: #0d6b2d;", {msg} }
            }
            if let Some(pending) = pending_delete() {
                LabelDeleteConfirm {
                    pending: pending.clone(),
                    pending_delete,
                    reload,
                    error,
                    success_msg,
                    labels_list,
                }
            }
            match labels_list() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e} } },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { style: "color: #666;", "No portfolios yet. Add one above — it appears here and on the portfolio list." }
                },
                Some(Ok(list)) => rsx! {
                    div { style: "overflow-x: auto; margin-top: 0.5rem;",
                        table { style: "width: 100%; border-collapse: collapse; font-size: 0.9rem;",
                            thead {
                                tr { style: "background: #f6f8fb; text-align: left;",
                                    th { style: "padding: 0.5rem;", "Name" }
                                    th { style: "padding: 0.5rem;", "Also tagged on" }
                                    th { style: "padding: 0.5rem;", "Transactions on delete" }
                                    th { style: "padding: 0.5rem; text-align: right;", "Actions" }
                                }
                            }
                            tbody {
                                for l in list.iter() {
                                    LabelRow {
                                        key: "{l.id}",
                                        label: l.clone(),
                                        pending_delete,
                                        error,
                                        success_msg,
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
