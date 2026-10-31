use dioxus::prelude::*;

use crate::portfolio::styles::{BTN_DANGER_SM, BTN_SECONDARY};
use crate::portfolio_api::{delete_transaction, fmt_inr, txn_type_label, Transaction, TransactionType};

fn corporate_action_qty_label(txn: &Transaction) -> String {
    match txn.txn_type {
        TransactionType::Split => {
            if let Some(num) = txn.split_ratio_num {
                let den = txn.split_ratio_den.unwrap_or(1.0);
                return format!("{num}:{den}");
            }
            txn.quantity.map(|q| format!("{q:.2}")).unwrap_or_default()
        }
        TransactionType::Bonus => {
            if let Some(num) = txn.bonus_ratio_num {
                let den = txn.bonus_ratio_den.unwrap_or(1.0);
                return format!("{num}:{den}");
            }
            txn.quantity.map(|q| format!("{q:.2}")).unwrap_or_default()
        }
        _ => txn.quantity.map(|q| format!("{q:.2}")).unwrap_or_default(),
    }
}

#[component]
pub fn TransactionRow(
    txn: Transaction,
    on_edit: EventHandler<Transaction>,
    on_delete: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let type_label = txn_type_label(&txn.txn_type);
    let qty_str = corporate_action_qty_label(&txn);
    let price_str = txn.price.map(fmt_inr).unwrap_or_default();
    let net_str = txn.net_amount.map(fmt_inr).unwrap_or_default();
    let sym = txn.symbol.clone().unwrap_or_default();
    rsx! {
        tr { style: "border-top: 1px solid #eee;",
            td { style: "padding: 0.5rem;", "{txn.trade_date}" }
            td { style: "padding: 0.5rem;", "{type_label}" }
            td { style: "padding: 0.5rem;", "{sym}" }
            td { style: "padding: 0.5rem;", "{qty_str}" }
            td { style: "padding: 0.5rem;", "{price_str}" }
            td { style: "padding: 0.5rem;", "{net_str}" }
            td { style: "padding: 0.5rem; white-space: nowrap;",
                button {
                    style: "{BTN_SECONDARY}",
                    onclick: {
                        let t = txn.clone();
                        move |_| on_edit.call(t.clone())
                    },
                    "Edit"
                }
                button {
                    style: "{BTN_DANGER_SM}",
                    onclick: move |_| {
                        let tid = txn.id;
                        spawn(async move {
                            match delete_transaction(tid).await {
                                Ok(()) => on_delete.call(()),
                                Err(e) => on_error.call(e),
                            }
                        });
                    },
                    "Delete"
                }
            }
        }
    }
}
