mod form;
mod helpers;
mod row;

use dioxus::prelude::*;

use crate::portfolio::confirm_dialog::ConfirmDialog;
use crate::portfolio::import::TransactionImport;
use crate::portfolio::layout::{AuthGuard, PortfolioNav, PortfolioTab};
use crate::portfolio::styles::{BTN_DANGER, BTN_PRIMARY, CANCEL_BTN, FILTER_BAR, INPUT};
use crate::portfolio_api::{
    clear_portfolio_transactions, list_transactions, refresh_sip_transactions,
    refresh_swp_transactions, Transaction, TransactionFilter,
};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

use form::TransactionForm;
use row::TransactionRow;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum AssetFilter {
    #[default]
    All,
    Equity,
    MutualFund,
}

fn asset_class_for_filter(filter: AssetFilter) -> Option<String> {
    match filter {
        AssetFilter::All => None,
        AssetFilter::Equity => Some("equity".into()),
        AssetFilter::MutualFund => Some("mutual_fund".into()),
    }
}

fn optional_date(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[component]
pub fn PortfolioTransactions(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut show_form = use_signal(|| false);
    let mut show_import = use_signal(|| false);
    let mut editing = use_signal(|| None::<Transaction>);
    let mut error = use_signal(|| None::<String>);
    let mut sip_refreshing = use_signal(|| false);
    let mut swp_refreshing = use_signal(|| false);
    let mut sip_refresh_msg = use_signal(|| None::<String>);
    let mut confirm_clear_all = use_signal(|| false);
    let mut clearing = use_signal(|| false);
    let mut from_date = use_signal(String::new);
    let mut to_date = use_signal(String::new);
    let mut asset_filter = use_signal(AssetFilter::default);

    let txns = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        let from = from_date();
        let to = to_date();
        let asset = asset_filter();
        async move {
            list_transactions(&TransactionFilter {
                portfolio_id: Some(id),
                from_date: optional_date(from),
                to_date: optional_date(to),
                asset_class: asset_class_for_filter(asset),
                ..Default::default()
            })
            .await
        }
    });

    let txn_body = match &*txns.read_unchecked() {
        None => rsx! { p { "Loading" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) if list.is_empty() => rsx! { p { "No transactions match the current filters" } },
        Some(Ok(list)) => rsx! {
            div { style: "overflow-x: auto;",
                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                    thead {
                        tr { style: "background: #f6f8fb; text-align: left;",
                            th { style: "padding: 0.5rem;", "Date" }
                            th { style: "padding: 0.5rem;", "Type" }
                            th { style: "padding: 0.5rem;", "Symbol" }
                            th { style: "padding: 0.5rem;", "Qty" }
                            th { style: "padding: 0.5rem;", "Price" }
                            th { style: "padding: 0.5rem;", "Net" }
                            th { style: "padding: 0.5rem;", "" }
                        }
                    }
                    tbody {
                        for t in list.iter() {
                            TransactionRow {
                                txn: t.clone(),
                                on_edit: move |txn: Transaction| {
                                    show_form.set(false);
                                    editing.set(Some(txn));
                                    error.set(None);
                                },
                                on_delete: move || reload.set(reload() + 1),
                                on_error: move |e: String| error.set(Some(e)),
                            }
                        }
                    }
                }
            }
        },
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            PortfolioNav { id, active: PortfolioTab::Transactions }
            TransactionToolbar {
                id,
                show_form,
                show_import,
                sip_refreshing,
                swp_refreshing,
                clearing,
                confirm_clear_all,
                reload,
                error,
                sip_refresh_msg,
            }
            if confirm_clear_all() {
                ClearAllConfirm {
                    id,
                    clearing,
                    confirm_clear_all,
                    reload,
                    error,
                    sip_refresh_msg,
                }
            }
            h1 { style: "margin-top: 0;", "Transactions" }
            TransactionFilters {
                from_date,
                to_date,
                asset_filter,
                reload,
            }
            if let Some(msg) = sip_refresh_msg() {
                p { style: "color: #0d6b2d;", {msg} }
            }
            if let Some(e) = error() {
                p { style: "color: #b00020;", {e} }
            }
            if show_import() {
                TransactionImport {
                    portfolio_id: id,
                    on_done: move || {
                        show_import.set(false);
                        reload.set(reload() + 1);
                        error.set(None);
                    },
                    on_error: move |e: String| error.set(Some(e)),
                }
            }
            if show_form() {
                TransactionForm {
                    portfolio_id: id,
                    on_saved: move || {
                        show_form.set(false);
                        reload.set(reload() + 1);
                        error.set(None);
                    },
                    on_cancel: move |_| {},
                    on_error: move |e: String| error.set(Some(e)),
                }
            }
            if let Some(ref txn) = editing() {
                TransactionForm {
                    key: "{txn.id}",
                    portfolio_id: id,
                    edit: Some(txn.clone()),
                    on_saved: move || {
                        editing.set(None);
                        reload.set(reload() + 1);
                        error.set(None);
                    },
                    on_cancel: move |_| editing.set(None),
                    on_error: move |e: String| error.set(Some(e)),
                }
            }
            {txn_body}
        }
    }
}

#[component]
fn TransactionToolbar(
    id: i64,
    mut show_form: Signal<bool>,
    mut show_import: Signal<bool>,
    mut sip_refreshing: Signal<bool>,
    mut swp_refreshing: Signal<bool>,
    clearing: Signal<bool>,
    mut confirm_clear_all: Signal<bool>,
    mut reload: Signal<u32>,
    mut error: Signal<Option<String>>,
    mut sip_refresh_msg: Signal<Option<String>>,
) -> Element {
    rsx! {
        div { style: "display: flex; gap: 0.75rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap;",
            button {
                style: "{BTN_PRIMARY}",
                onclick: move |_| {
                    show_form.set(!show_form());
                },
                if show_form() { "Hide form" } else { "Add transaction" }
            }
            button {
                style: "{BTN_PRIMARY}",
                onclick: move |_| show_import.set(!show_import()),
                if show_import() { "Hide import" } else { "Import" }
            }
            button {
                style: "{BTN_PRIMARY}",
                disabled: sip_refreshing(),
                onclick: move |_| {
                    spawn(async move {
                        sip_refreshing.set(true);
                        sip_refresh_msg.set(None);
                        error.set(None);
                        match refresh_sip_transactions(id).await {
                            Ok(result) => {
                                sip_refresh_msg.set(Some(format!(
                                    "SIP refresh: {} buy(s) created, {} skipped, {} failed",
                                    result.created.len(),
                                    result.skipped.len(),
                                    result.failed.len()
                                )));
                                if !result.created.is_empty() {
                                    reload.set(reload() + 1);
                                }
                                if !result.failed.is_empty() {
                                    let detail: Vec<String> = result
                                        .failed
                                        .iter()
                                        .map(|f| format!("SIP #{} ({}): {}", f.sip_id, f.trade_date, f.reason))
                                        .collect();
                                    error.set(Some(detail.join("; ")));
                                }
                            }
                            Err(e) => error.set(Some(e)),
                        }
                        sip_refreshing.set(false);
                    });
                },
                if sip_refreshing() { "Refreshing SIP…" } else { "Refresh SIP transactions" }
            }
            Link {
                to: Route::PortfolioSchedules { id },
                style: "color: #1a56db; font-weight: 600; text-decoration: none;",
                "SIPs & SWPs"
            }
            button {
                style: "{BTN_PRIMARY}",
                disabled: swp_refreshing(),
                onclick: move |_| {
                    spawn(async move {
                        swp_refreshing.set(true);
                        sip_refresh_msg.set(None);
                        error.set(None);
                        match refresh_swp_transactions(id).await {
                            Ok(result) => {
                                sip_refresh_msg.set(Some(format!(
                                    "SWP refresh: {} sell(s) created, {} skipped, {} failed",
                                    result.created.len(),
                                    result.skipped.len(),
                                    result.failed.len()
                                )));
                                if !result.created.is_empty() {
                                    reload.set(reload() + 1);
                                }
                                if !result.failed.is_empty() {
                                    let detail: Vec<String> = result
                                        .failed
                                        .iter()
                                        .map(|f| format!("SWP #{} ({}): {}", f.swp_id, f.trade_date, f.reason))
                                        .collect();
                                    error.set(Some(detail.join("; ")));
                                }
                            }
                            Err(e) => error.set(Some(e)),
                        }
                        swp_refreshing.set(false);
                    });
                },
                if swp_refreshing() { "Refreshing SWP…" } else { "Refresh SWP transactions" }
            }
            button {
                style: "{BTN_DANGER}",
                disabled: clearing(),
                onclick: move |_| confirm_clear_all.set(true),
                if clearing() { "Deleting…" } else { "Delete all transactions" }
            }
        }
    }
}

#[component]
fn ClearAllConfirm(
    id: i64,
    mut clearing: Signal<bool>,
    mut confirm_clear_all: Signal<bool>,
    mut reload: Signal<u32>,
    mut error: Signal<Option<String>>,
    mut sip_refresh_msg: Signal<Option<String>>,
) -> Element {
    rsx! {
        ConfirmDialog {
            title: String::from("Delete every transaction in this portfolio?"),
            message: rsx! {
                p { style: "margin: 0; color: #b00020;",
                    "This cannot be undone. Holdings will be cleared after recalculation."
                }
            },
            confirm_label: "Confirm delete all",
            confirming: clearing(),
            on_confirm: move |_| {
                spawn(async move {
                    clearing.set(true);
                    error.set(None);
                    match clear_portfolio_transactions(id).await {
                        Ok(result) => {
                            confirm_clear_all.set(false);
                            reload.set(reload() + 1);
                            sip_refresh_msg.set(Some(format!(
                                "Deleted {} transaction(s).",
                                result.transactions_deleted
                            )));
                        }
                        Err(e) => error.set(Some(e)),
                    }
                    clearing.set(false);
                });
            },
            on_cancel: move |_| confirm_clear_all.set(false),
        }
    }
}

#[component]
fn TransactionFilters(
    mut from_date: Signal<String>,
    mut to_date: Signal<String>,
    mut asset_filter: Signal<AssetFilter>,
    mut reload: Signal<u32>,
) -> Element {
    rsx! {
        div { style: "{FILTER_BAR}",
            label { style: "display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.85rem;",
                "From date"
                input {
                    r#type: "date",
                    style: "{INPUT}",
                    value: "{from_date}",
                    oninput: move |ev| {
                        from_date.set(ev.value());
                        reload.set(reload() + 1);
                    },
                }
            }
            label { style: "display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.85rem;",
                "To date"
                input {
                    r#type: "date",
                    style: "{INPUT}",
                    value: "{to_date}",
                    oninput: move |ev| {
                        to_date.set(ev.value());
                        reload.set(reload() + 1);
                    },
                }
            }
            label { style: "display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.85rem;",
                "Asset type"
                select {
                    style: "{INPUT}",
                    onchange: move |ev| {
                        asset_filter.set(match ev.value().as_str() {
                            "equity" => AssetFilter::Equity,
                            "mutual_fund" => AssetFilter::MutualFund,
                            _ => AssetFilter::All,
                        });
                        reload.set(reload() + 1);
                    },
                    option { value: "all", selected: asset_filter() == AssetFilter::All, "All" }
                    option { value: "equity", selected: asset_filter() == AssetFilter::Equity, "Stocks" }
                    option { value: "mutual_fund", selected: asset_filter() == AssetFilter::MutualFund, "Mutual funds" }
                }
            }
            button {
                style: "{CANCEL_BTN}",
                onclick: move |_| {
                    from_date.set(String::new());
                    to_date.set(String::new());
                    asset_filter.set(AssetFilter::All);
                    reload.set(reload() + 1);
                },
                "Clear filters"
            }
        }
    }
}
