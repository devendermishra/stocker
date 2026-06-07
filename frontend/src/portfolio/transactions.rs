use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{
    create_transaction, delete_transaction, fmt_inr, list_transactions, txn_type_label,
    NewTransaction, Transaction, TransactionFilter, TransactionType,
};
use crate::routes::Route;

const INPUT: &str = "padding: 0.45rem 0.6rem; border: 1px solid #d5dbe3; border-radius: 6px;";
const BTN: &str = "padding: 0.45rem 0.75rem; background: #1a56db; color: #fff; border: none; border-radius: 8px; cursor: pointer;";

#[component]
pub fn PortfolioTransactions(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut show_form = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let txns = use_resource(move || {
        let _ = reload();
        async move {
            list_transactions(&TransactionFilter {
                portfolio_id: Some(id),
                ..Default::default()
            })
            .await
        }
    });

    let txn_body = match &*txns.read_unchecked() {
        None => rsx! { p { "Loading" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) if list.is_empty() => rsx! { p { "No transactions yet" } },
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
            div { style: "display: flex; gap: 0.75rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap;",
                Link { to: Route::PortfolioDashboard { id }, style: "color: #1a56db;", "← Dashboard" }
                button {
                    style: "{BTN}",
                    onclick: move |_| show_form.set(!show_form()),
                    if show_form() { "Hide form" } else { "Add transaction" }
                }
            }
            h1 { "Transactions" }
            if let Some(e) = error() {
                p { style: "color: #b00020;", {e} }
            }
            if show_form() {
                TransactionForm {
                    portfolio_id: id,
                    on_saved: move || {
                        show_form.set(false);
                        reload.set(reload() + 1);
                        error.set(None);
                    },
                    on_error: move |e: String| error.set(Some(e)),
                }
            }
            {txn_body}
        }
    }
}

#[component]
fn TransactionRow(
    txn: Transaction,
    on_delete: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let type_label = txn_type_label(&txn.txn_type);
    let qty_str = txn.quantity.map(|q| format!("{q:.2}")).unwrap_or_default();
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
            td { style: "padding: 0.5rem;",
                button {
                    style: "padding: 0.25rem 0.5rem; border: 1px solid #f5c6cb; background: #fff; color: #b00020; border-radius: 6px; cursor: pointer; font-size: 0.8rem;",
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

#[component]
fn TransactionForm(
    portfolio_id: i64,
    on_saved: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let mut txn_type = use_signal(|| TransactionType::Buy);
    let mut trade_date = use_signal(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut symbol = use_signal(String::new);
    let mut quantity = use_signal(String::new);
    let mut price = use_signal(String::new);
    let mut net_amount = use_signal(String::new);
    let mut split_num = use_signal(|| "5".to_string());
    let mut split_den = use_signal(|| "1".to_string());
    let mut bonus_num = use_signal(|| "1".to_string());
    let mut bonus_den = use_signal(|| "1".to_string());
    let mut dividend_per_share = use_signal(String::new);

    rsx! {
        div { style: "background: #f6f8fb; border: 1px solid #dfe3eb; border-radius: 12px; padding: 1rem; margin-bottom: 1rem;",
            h3 { style: "margin-top: 0;", "New transaction" }
            div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.75rem;",
                label {
                    "Type"
                    select {
                        style: "{INPUT}",
                        onchange: move |ev| {
                            txn_type.set(match ev.value().as_str() {
                                "opening_balance" => TransactionType::OpeningBalance,
                                "sell" => TransactionType::Sell,
                                "dividend" => TransactionType::Dividend,
                                "split" => TransactionType::Split,
                                "bonus" => TransactionType::Bonus,
                                _ => TransactionType::Buy,
                            });
                        },
                        option { value: "buy", selected: txn_type() == TransactionType::Buy, "Buy" }
                        option { value: "sell", selected: txn_type() == TransactionType::Sell, "Sell" }
                        option { value: "opening_balance", selected: txn_type() == TransactionType::OpeningBalance, "Opening balance" }
                        option { value: "dividend", selected: txn_type() == TransactionType::Dividend, "Dividend" }
                        option { value: "split", selected: txn_type() == TransactionType::Split, "Split" }
                        option { value: "bonus", selected: txn_type() == TransactionType::Bonus, "Bonus" }
                    }
                }
                label { "Date"
                    input { style: "{INPUT}", value: "{trade_date}", oninput: move |ev| trade_date.set(ev.value()) }
                }
                label { "Symbol"
                    input { style: "{INPUT}", placeholder: "ITC or ITC.NS", value: "{symbol}", oninput: move |ev| symbol.set(ev.value()) }
                }
                if matches!(txn_type(), TransactionType::Buy | TransactionType::Sell | TransactionType::OpeningBalance) {
                    label { "Quantity"
                        input { style: "{INPUT}", value: "{quantity}", oninput: move |ev| quantity.set(ev.value()) }
                    }
                    label { "Price"
                        input { style: "{INPUT}", value: "{price}", oninput: move |ev| price.set(ev.value()) }
                    }
                    label { "Net amount"
                        input { style: "{INPUT}", value: "{net_amount}", oninput: move |ev| net_amount.set(ev.value()) }
                    }
                }
                if txn_type() == TransactionType::Split {
                    label { "Split num"
                        input { style: "{INPUT}", value: "{split_num}", oninput: move |ev| split_num.set(ev.value()) }
                    }
                    label { "Split den"
                        input { style: "{INPUT}", value: "{split_den}", oninput: move |ev| split_den.set(ev.value()) }
                    }
                }
                if txn_type() == TransactionType::Bonus {
                    label { "Bonus num"
                        input { style: "{INPUT}", value: "{bonus_num}", oninput: move |ev| bonus_num.set(ev.value()) }
                    }
                    label { "Bonus den"
                        input { style: "{INPUT}", value: "{bonus_den}", oninput: move |ev| bonus_den.set(ev.value()) }
                    }
                }
                if txn_type() == TransactionType::Dividend {
                    label { "Dividend/share"
                        input { style: "{INPUT}", value: "{dividend_per_share}", oninput: move |ev| dividend_per_share.set(ev.value()) }
                    }
                    label { "Net amount"
                        input { style: "{INPUT}", value: "{net_amount}", oninput: move |ev| net_amount.set(ev.value()) }
                    }
                }
            }
            button {
                style: "{BTN}; margin-top: 0.75rem;",
                onclick: move |_| {
                    let input = build_txn(
                        portfolio_id,
                        txn_type(),
                        trade_date(),
                        symbol(),
                        quantity(),
                        price(),
                        net_amount(),
                        split_num(),
                        split_den(),
                        bonus_num(),
                        bonus_den(),
                        dividend_per_share(),
                    );
                    spawn(async move {
                        match create_transaction(&input).await {
                            Ok(_) => on_saved.call(()),
                            Err(e) => on_error.call(e),
                        }
                    });
                },
                "Save transaction"
            }
        }
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    if s.trim().is_empty() {
        None
    } else {
        s.trim().parse().ok()
    }
}

fn build_txn(
    portfolio_id: i64,
    txn_type: TransactionType,
    trade_date: String,
    symbol: String,
    quantity: String,
    price: String,
    net_amount: String,
    split_num: String,
    split_den: String,
    bonus_num: String,
    bonus_den: String,
    dividend_per_share: String,
) -> NewTransaction {
    let sym = if symbol.trim().is_empty() {
        None
    } else {
        Some(symbol.trim().to_string())
    };
    let qty = parse_f64(&quantity);
    let pr = parse_f64(&price);
    let net = parse_f64(&net_amount);
    NewTransaction {
        portfolio_id,
        txn_type,
        trade_date,
        symbol: sym,
        quantity: qty,
        price: pr,
        gross_amount: net.or_else(|| qty.zip(pr).map(|(q, p)| q * p)),
        brokerage: None,
        taxes: None,
        net_amount: net,
        split_ratio_num: parse_f64(&split_num),
        split_ratio_den: parse_f64(&split_den),
        bonus_ratio_num: parse_f64(&bonus_num),
        bonus_ratio_den: parse_f64(&bonus_den),
        dividend_per_share: parse_f64(&dividend_per_share),
        tds: None,
        eligible_quantity: qty,
        notes: None,
    }
}

trait ZipOpt {
    fn zip<U>(self, other: Option<U>) -> Option<(f64, U)>;
}
impl ZipOpt for Option<f64> {
    fn zip<U>(self, other: Option<U>) -> Option<(f64, U)> {
        match (self, other) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }
}
