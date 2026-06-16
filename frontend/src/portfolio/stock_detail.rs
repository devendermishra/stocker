use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{
    attach_label, detach_label, fifo_lots, fmt_inr, holdings, list_labels, list_transactions,
    txn_type_label, FifoLot, Holding, Label, Transaction, TransactionFilter,
};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

const DELETE_BTN: &str = "padding: 0.15rem 0.45rem; border: 1px solid #f5c6cb; background: #fff; color: #b00020; border-radius: 6px; cursor: pointer; font-size: 0.75rem; margin-left: 0.35rem;";

#[component]
pub fn PortfolioStockDetail(id: i64, symbol: String) -> Element {
    let mut reload = use_signal(|| 0u32);
    let sym = symbol.clone();

    let holding: Resource<Result<Option<Holding>, String>> = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            let _ = reload();
            let _ = portfolio_data_revision();
            async move {
                let all = holdings(id).await?;
                Ok(all.into_iter().find(|h| h.symbol == s))
            }
        }
    });
    let lots = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            let _ = reload();
            let _ = portfolio_data_revision();
            async move { fifo_lots(id, &s).await }
        }
    });
    let txns = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            let _ = reload();
            let _ = portfolio_data_revision();
            async move {
                list_transactions(&TransactionFilter {
                    portfolio_id: Some(id),
                    symbol: Some(s),
                    ..Default::default()
                })
                .await
            }
        }
    });
    let labels = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { list_labels().await }
    });

    let attached_labels: Vec<Label> = holding
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|h| h.as_ref())
        .map(|h| h.labels.clone())
        .unwrap_or_default();
    let attached_ids: Vec<i64> = attached_labels.iter().map(|l| l.id).collect();

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 1rem;",
                Link { to: Route::PortfolioHoldings { id }, style: "color: #1a56db;", "← Holdings" }
            }
            h1 { "{symbol}" }
            match &*holding.read() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(None)) => rsx! { p { "No current holding for this symbol" } },
                Some(Ok(Some(h))) => render_holding_stats(h),
            }

            h2 { "Labels" }
            if attached_labels.is_empty() {
                p { style: "color: #666; font-size: 0.9rem; margin-bottom: 1rem;", "No labels attached." }
            } else {
                div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 1rem;",
                    for label in attached_labels.iter() {
                        AttachedLabelChip {
                            key: "{label.id}",
                            portfolio_id: id,
                            symbol: sym.clone(),
                            label: label.clone(),
                            on_detached: move || reload.set(reload() + 1),
                        }
                    }
                }
            }
            p { style: "color: #666; font-size: 0.85rem; margin-bottom: 0.75rem;",
                Link { to: Route::PortfolioLabels {}, style: "color: #1a56db;",
                    "Manage labels"
                }
                " — delete a label and all its linked transactions"
            }

            h2 { "Attach label" }
            match &*labels.read() {
                None => rsx! { p { "Loading labels…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(all_labels)) => {
                    let available: Vec<_> = all_labels
                        .iter()
                        .filter(|l| !attached_ids.contains(&l.id))
                        .cloned()
                        .collect();
                    if available.is_empty() {
                        rsx! {
                            p { style: "color: #666; font-size: 0.9rem;",
                                if all_labels.is_empty() {
                                    "No labels defined yet. "
                                } else {
                                    "All labels are already attached. "
                                }
                                Link { to: Route::PortfolioLabels {}, style: "color: #1a56db;", "Create a label" }
                            }
                        }
                    } else {
                        rsx! {
                            div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 1rem;",
                                for l in available.iter() {
                                    LabelAttachButton {
                                        key: "{l.id}",
                                        portfolio_id: id,
                                        symbol: sym.clone(),
                                        label: l.clone(),
                                        on_attached: move || reload.set(reload() + 1),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            h2 { "FIFO lots" }
            match &*lots.read() {
                None => rsx! { p { "Loading lots…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(ls)) => render_lots(ls),
            }

            h2 { "Transactions" }
            match &*txns.read() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(list)) => render_txn_list(list),
            }
        }
    }
}

#[component]
fn AttachedLabelChip(
    portfolio_id: i64,
    symbol: String,
    label: Label,
    on_detached: EventHandler<()>,
) -> Element {
    rsx! {
        span {
            style: "display: inline-flex; align-items: center; padding: 0.35rem 0.65rem; border: 1px solid #d5dbe3; border-radius: 999px; background: #f6f8fb; font-size: 0.85rem;",
            "{label.name}"
            button {
                style: "{DELETE_BTN}",
                title: "Remove label from this holding",
                onclick: move |_| {
                    let entity_id = format!("{portfolio_id}:{symbol}");
                    let lid = label.id;
                    spawn(async move {
                        let _ = detach_label(lid, "holding", &entity_id).await;
                    });
                    on_detached.call(());
                },
                "×"
            }
        }
    }
}

#[component]
fn LabelAttachButton(
    portfolio_id: i64,
    symbol: String,
    label: Label,
    on_attached: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            style: "padding: 0.35rem 0.65rem; border: 1px solid #d5dbe3; border-radius: 999px; background: #fff; cursor: pointer; font-size: 0.85rem;",
            onclick: move |_| {
                let entity_id = format!("{portfolio_id}:{symbol}");
                let lid = label.id;
                spawn(async move {
                    let _ = attach_label(lid, "holding", &entity_id).await;
                });
                on_attached.call(());
            },
            "{label.name}"
        }
    }
}

fn render_holding_stats(h: &Holding) -> Element {
    rsx! {
        div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.75rem; margin-bottom: 1rem;",
            Stat { label: "Quantity".to_string(), value: format!("{:.2}", h.quantity) }
            Stat { label: "Avg cost".to_string(), value: fmt_inr(h.average_cost) }
            Stat { label: "Invested".to_string(), value: fmt_inr(h.total_cost) }
            Stat { label: "Price".to_string(), value: h.current_price.map(fmt_inr).unwrap_or_default() }
            Stat { label: "Value".to_string(), value: h.current_value.map(fmt_inr).unwrap_or_default() }
            Stat { label: "Unrealized".to_string(), value: h.unrealized_gain.map(fmt_inr).unwrap_or_default() }
            Stat { label: "Realized".to_string(), value: fmt_inr(h.realized_gain) }
            Stat { label: "Dividends".to_string(), value: fmt_inr(h.dividend_received) }
        }
    }
}

fn render_lots(ls: &[FifoLot]) -> Element {
    if ls.is_empty() {
        return rsx! { p { "No open lots" } };
    }
    rsx! {
        for lot in ls.iter() {
            p { style: "font-size: 0.9rem; margin: 0.25rem 0;",
                "{lot.acquired_date}: {lot.remaining_quantity:.2} @ {fmt_inr(lot.cost_per_share)} (cost {fmt_inr(lot.total_cost)})"
            }
        }
    }
}

fn render_txn_list(list: &[Transaction]) -> Element {
    rsx! {
        for t in list.iter() {
            TxnLine { key: "{t.id}", txn: t.clone() }
        }
    }
}

#[component]
fn TxnLine(txn: Transaction) -> Element {
    let type_label = txn_type_label(&txn.txn_type);
    let qty = txn.quantity.map(|q| format!("{q:.2}")).unwrap_or_default();
    rsx! {
        p { style: "font-size: 0.9rem; margin: 0.25rem 0;",
            "{txn.trade_date} — {type_label} — qty {qty}"
        }
    }
}

#[component]
fn Stat(label: String, value: String) -> Element {
    rsx! {
        div { style: "background: #fff; border: 1px solid #dfe3eb; border-radius: 10px; padding: 0.65rem;",
            div { style: "font-size: 0.75rem; color: #666;", "{label}" }
            div { style: "font-weight: 700;", "{value}" }
        }
    }
}
