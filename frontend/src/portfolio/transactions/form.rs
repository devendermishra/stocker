use dioxus::prelude::*;

use crate::portfolio::styles::{BTN_OUTLINE, BTN_PRIMARY, FORM_PANEL, INPUT};
use crate::portfolio_api::{
    create_transaction, search_mutual_funds, update_transaction, MfSearchHit, Transaction,
    TransactionType,
};

use super::helpers::{
    build_txn, fill_net_amount_string, form_initial_state, maybe_fill_net_amount, resolve_mf_for_edit,
    AssetKind,
};

const TXN_TYPE_OPTIONS: &[(TransactionType, &str, &str)] = &[
    (TransactionType::Buy, "buy", "Buy"),
    (TransactionType::Sell, "sell", "Sell"),
    (TransactionType::Sip, "sip", "SIP"),
    (TransactionType::Swp, "swp", "SWP"),
    (TransactionType::MergerInvestment, "merger_investment", "Merger Investment"),
    (TransactionType::DemergerInvestment, "demerger_investment", "Demerger Investment"),
    (TransactionType::MergerRedemption, "merger_redemption", "Merger Redemption"),
    (TransactionType::DemergerRedemption, "demerger_redemption", "Demerger Redemption"),
    (TransactionType::OpeningBalance, "opening_balance", "Opening balance"),
    (TransactionType::Dividend, "dividend", "Dividend"),
    (TransactionType::Split, "split", "Splits"),
    (TransactionType::Bonus, "bonus", "Bonus"),
    (TransactionType::Rights, "rights", "Rights"),
];

#[component]
pub fn TransactionForm(
    portfolio_id: i64,
    #[props(default)]
    edit: Option<Transaction>,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let edit_id = edit.as_ref().map(|t| t.id);
    let initial = edit.as_ref().map(form_initial_state);

    let mut txn_type = use_signal(|| initial.as_ref().map(|s| s.txn_type.clone()).unwrap_or(TransactionType::Buy));
    let mut trade_date = use_signal(|| {
        initial
            .as_ref()
            .map(|s| s.trade_date.clone())
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string())
    });
    let mut asset_kind = use_signal(|| initial.as_ref().map(|s| s.asset_kind).unwrap_or(AssetKind::Stock));
    let mut symbol = use_signal(|| initial.as_ref().map(|s| s.symbol.clone()).unwrap_or_default());
    let mut mf_query = use_signal(|| initial.as_ref().map(|s| s.mf_query.clone()).unwrap_or_default());
    let mut mf_hits = use_signal(Vec::<MfSearchHit>::new);
    let mut selected_mf = use_signal(|| initial.as_ref().and_then(|s| s.selected_mf.clone()));
    let mut quantity = use_signal(|| initial.as_ref().map(|s| s.quantity.clone()).unwrap_or_default());
    let mut price = use_signal(|| initial.as_ref().map(|s| s.price.clone()).unwrap_or_default());
    let mut net_amount = use_signal(|| initial.as_ref().map(|s| s.net_amount.clone()).unwrap_or_default());
    let mut split_num = use_signal(|| initial.as_ref().map(|s| s.split_num.clone()).unwrap_or_else(|| "5".to_string()));
    let mut split_den = use_signal(|| initial.as_ref().map(|s| s.split_den.clone()).unwrap_or_else(|| "1".to_string()));
    let mut bonus_num = use_signal(|| initial.as_ref().map(|s| s.bonus_num.clone()).unwrap_or_else(|| "1".to_string()));
    let mut bonus_den = use_signal(|| initial.as_ref().map(|s| s.bonus_den.clone()).unwrap_or_else(|| "1".to_string()));
    let mut dividend_per_share = use_signal(|| initial.as_ref().map(|s| s.dividend_per_share.clone()).unwrap_or_default());

    if let Some(ref txn) = edit {
        if txn.symbol.as_deref().is_some_and(|s| s.starts_with("MF:")) {
            let sym = txn.symbol.clone().unwrap_or_default();
            use_effect(move || {
                let sym = sym.clone();
                spawn(async move {
                    if let Some(hit) = resolve_mf_for_edit(sym).await {
                        selected_mf.set(Some(hit.clone()));
                        mf_query.set(hit.scheme_name);
                    }
                });
            });
        }
    }

    let is_edit = edit_id.is_some();
    let heading = if is_edit { "Edit transaction" } else { "New transaction" };
    let save_label = if is_edit { "Save changes" } else { "Save transaction" };

    rsx! {
        div { style: "{FORM_PANEL}",
            h3 { style: "margin-top: 0;", "{heading}" }
            div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.75rem;",
                AssetKindSelect { asset_kind, selected_mf, mf_hits }
                TxnTypeSelect { txn_type }
                label { "Date"
                    input { style: "{INPUT}", value: "{trade_date}", oninput: move |ev| trade_date.set(ev.value()) }
                }
                if asset_kind() == AssetKind::Stock {
                    label { "Symbol"
                        input { style: "{INPUT}", placeholder: "ITC or ITC.NS", value: "{symbol}", oninput: move |ev| symbol.set(ev.value()) }
                    }
                } else {
                    MfSearchField {
                        mf_query,
                        mf_hits,
                        selected_mf,
                    }
                }
                if txn_type().requires_qty_price() {
                    TradeAmountFields {
                        asset_kind,
                        quantity,
                        price,
                        net_amount,
                    }
                }
                if txn_type() == TransactionType::Split {
                    RatioFields { label_num: "Split num", label_den: "Split den", num: split_num, den: split_den }
                }
                if txn_type() == TransactionType::Bonus {
                    RatioFields { label_num: "Bonus num", label_den: "Bonus den", num: bonus_num, den: bonus_den }
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
            div { style: "display: flex; gap: 0.75rem; margin-top: 0.75rem; flex-wrap: wrap;",
                button {
                    style: "{BTN_PRIMARY}",
                    onclick: move |_| save_transaction(
                        portfolio_id,
                        edit_id,
                        txn_type(),
                        trade_date(),
                        asset_kind(),
                        symbol(),
                        mf_query(),
                        selected_mf(),
                        quantity(),
                        price(),
                        net_amount(),
                        split_num(),
                        split_den(),
                        bonus_num(),
                        bonus_den(),
                        dividend_per_share(),
                        on_saved,
                        on_error,
                    ),
                    "{save_label}"
                }
                if is_edit {
                    button {
                        style: "{BTN_OUTLINE}",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                }
            }
        }
    }
}

#[component]
fn AssetKindSelect(
    mut asset_kind: Signal<AssetKind>,
    mut selected_mf: Signal<Option<MfSearchHit>>,
    mut mf_hits: Signal<Vec<MfSearchHit>>,
) -> Element {
    rsx! {
        label {
            "Asset"
            select {
                style: "{INPUT}",
                onchange: move |ev| {
                    asset_kind.set(if ev.value() == "mutual_fund" {
                        AssetKind::MutualFund
                    } else {
                        AssetKind::Stock
                    });
                    selected_mf.set(None);
                    mf_hits.set(Vec::new());
                },
                option { value: "stock", selected: asset_kind() == AssetKind::Stock, "Stock" }
                option { value: "mutual_fund", selected: asset_kind() == AssetKind::MutualFund, "Mutual Fund" }
            }
        }
    }
}

#[component]
fn TxnTypeSelect(mut txn_type: Signal<TransactionType>) -> Element {
    rsx! {
        label {
            "Type"
            select {
                style: "{INPUT}",
                onchange: move |ev| txn_type.set(TransactionType::from_form_value(&ev.value())),
                for (ty, value, label) in TXN_TYPE_OPTIONS {
                    option {
                        value: "{value}",
                        selected: txn_type() == *ty,
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
fn MfSearchField(
    mut mf_query: Signal<String>,
    mut mf_hits: Signal<Vec<MfSearchHit>>,
    mut selected_mf: Signal<Option<MfSearchHit>>,
) -> Element {
    rsx! {
        label { style: "grid-column: 1 / -1;", "Fund name"
            input {
                style: "{INPUT}; width: 100%;",
                placeholder: "Parag Parikh Flexi Cap Direct Growth",
                value: "{mf_query}",
                oninput: move |ev| {
                    let q = ev.value();
                    mf_query.set(q.clone());
                    selected_mf.set(None);
                    if q.trim().len() < 3 {
                        mf_hits.set(Vec::new());
                        return;
                    }
                    spawn(async move {
                        debounce_search().await;
                        match search_mutual_funds(&q).await {
                            Ok(hits) => mf_hits.set(hits),
                            Err(_) => mf_hits.set(Vec::new()),
                        }
                    });
                },
            }
        }
        if let Some(sel) = selected_mf() {
            p { style: "grid-column: 1 / -1; margin: 0; font-size: 0.85rem; color: #166534;",
                "Selected: {sel.scheme_name} ({sel.scheme_code})"
            }
        } else if !mf_hits().is_empty() {
            div { style: "grid-column: 1 / -1; border: 1px solid #dfe3eb; border-radius: 8px; max-height: 160px; overflow-y: auto; background: #fff;",
                for hit in mf_hits().iter().cloned() {
                    {
                        let scheme_code = hit.scheme_code;
                        let scheme_name_label = hit.scheme_name.clone();
                        let scheme_name_pick = hit.scheme_name.clone();
                        let pick = hit;
                        rsx! {
                            button {
                                key: "{scheme_code}",
                                style: "display: block; width: 100%; text-align: left; padding: 0.5rem 0.75rem; border: none; border-bottom: 1px solid #eee; background: #fff; cursor: pointer; font-size: 0.85rem;",
                                onclick: move |_| {
                                    selected_mf.set(Some(pick.clone()));
                                    mf_query.set(scheme_name_pick.clone());
                                    mf_hits.set(Vec::new());
                                },
                                "{scheme_name_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TradeAmountFields(
    asset_kind: Signal<AssetKind>,
    mut quantity: Signal<String>,
    mut price: Signal<String>,
    mut net_amount: Signal<String>,
) -> Element {
    rsx! {
        label { "Quantity"
            input {
                style: "{INPUT}",
                value: "{quantity}",
                oninput: move |ev| {
                    let q = ev.value();
                    quantity.set(q.clone());
                    maybe_fill_net_amount(&q, &price(), &mut net_amount);
                },
            }
        }
        label {
            if asset_kind() == AssetKind::MutualFund { "NAV" } else { "Price" }
            input {
                style: "{INPUT}",
                value: "{price}",
                oninput: move |ev| {
                    let p = ev.value();
                    price.set(p.clone());
                    maybe_fill_net_amount(&quantity(), &p, &mut net_amount);
                },
            }
        }
        label { "Net amount"
            input {
                style: "{INPUT}",
                value: "{net_amount}",
                oninput: move |ev| {
                    let v = ev.value();
                    net_amount.set(v.clone());
                    if v.trim().is_empty() {
                        maybe_fill_net_amount(&quantity(), &price(), &mut net_amount);
                    }
                },
            }
        }
    }
}

#[component]
fn RatioFields(
    label_num: &'static str,
    label_den: &'static str,
    mut num: Signal<String>,
    mut den: Signal<String>,
) -> Element {
    rsx! {
        label { "{label_num}"
            input { style: "{INPUT}", value: "{num}", oninput: move |ev| num.set(ev.value()) }
        }
        label { "{label_den}"
            input { style: "{INPUT}", value: "{den}", oninput: move |ev| den.set(ev.value()) }
        }
    }
}

async fn debounce_search() {
    #[cfg(feature = "web")]
    {
        gloo_timers::future::TimeoutFuture::new(300).await;
    }
    #[cfg(all(feature = "desktop", not(feature = "web")))]
    {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
}

#[allow(clippy::too_many_arguments)]
fn save_transaction(
    portfolio_id: i64,
    edit_id: Option<i64>,
    txn_type: TransactionType,
    trade_date: String,
    asset_kind: AssetKind,
    symbol: String,
    mf_query: String,
    selected_mf: Option<MfSearchHit>,
    quantity: String,
    price: String,
    net_amount: String,
    split_num: String,
    split_den: String,
    bonus_num: String,
    bonus_den: String,
    dividend_per_share: String,
    on_saved: EventHandler<()>,
    on_error: EventHandler<String>,
) {
    let sym = if asset_kind == AssetKind::MutualFund {
        selected_mf
            .map(|h| format!("MF:{}", h.scheme_code))
            .unwrap_or_else(|| mf_query)
    } else {
        symbol
    };
    if asset_kind == AssetKind::MutualFund && sym.trim().is_empty() {
        on_error.call("Select a mutual fund from search results".into());
        return;
    }
    let mut net = net_amount;
    if txn_type.requires_qty_price() {
        fill_net_amount_string(&quantity, &price, &mut net);
    }
    let input = build_txn(
        portfolio_id,
        txn_type,
        trade_date,
        sym,
        quantity,
        price,
        net,
        split_num,
        split_den,
        bonus_num,
        bonus_den,
        dividend_per_share,
    );
    spawn(async move {
        let result = if let Some(id) = edit_id {
            update_transaction(id, &input).await.map(|_| ())
        } else {
            create_transaction(&input).await.map(|_| ())
        };
        match result {
            Ok(()) => on_saved.call(()),
            Err(e) => on_error.call(e),
        }
    });
}
