use dioxus::prelude::*;

use crate::portfolio::{AuthGuard, FILTER_BAR};
use crate::portfolio_api::{fmt_inr, txn_type_label, Transaction, TransactionFilter};
use crate::routes::Route;
use crate::sync_portfolio::layout::{SyncPortfolioBanner, SyncPortfolioNav, SyncPortfolioTab};
use crate::sync_portfolio_api::{remote_transactions, sync_remote_exported_at};

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

#[component]
pub fn SyncPortfolioTransactions(id: i64) -> Element {
    let exported_at = use_resource(|| async move { sync_remote_exported_at().await.ok().flatten() });
    let mut asset_filter = use_signal(AssetFilter::default);

    let txns = use_resource(move || {
        let asset = asset_filter();
        async move {
            remote_transactions(
                id,
                &TransactionFilter {
                    portfolio_id: Some(id),
                    asset_class: asset_class_for_filter(asset),
                    ..Default::default()
                },
            )
            .await
        }
    });

    let txn_body = match &*txns.read_unchecked() {
        None => rsx! { p { "Loading…" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) if list.is_empty() => rsx! { p { "No transactions" } },
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
                            th { style: "padding: 0.5rem;", "Amount" }
                        }
                    }
                    tbody {
                        for t in list.iter() {
                            ReadOnlyTxnRow { key: "{t.id}", txn: t.clone() }
                        }
                    }
                }
            }
        },
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::DriveSync {}, style: "color: #184ad8;", "← Google Drive Sync" }
            }
            if let Some(Some(ts)) = exported_at.read().as_ref() {
                SyncPortfolioBanner { exported_at: Some(ts.clone()) }
            }
            SyncPortfolioNav { id, active: SyncPortfolioTab::Transactions }
            h1 { style: "margin-top: 0;", "Transactions" }
            div { style: "{FILTER_BAR}",
                label { style: "font-size: 0.85rem;",
                    "Asset: "
                    select {
                        style: "margin-left: 0.35rem; padding: 0.35rem;",
                        onchange: move |ev| {
                            asset_filter.set(match ev.value().as_str() {
                                "equity" => AssetFilter::Equity,
                                "mutual_fund" => AssetFilter::MutualFund,
                                _ => AssetFilter::All,
                            });
                        },
                        option { value: "all", selected: asset_filter() == AssetFilter::All, "All" }
                        option { value: "equity", selected: asset_filter() == AssetFilter::Equity, "Equity" }
                        option { value: "mutual_fund", selected: asset_filter() == AssetFilter::MutualFund, "Mutual funds" }
                    }
                }
            }
            {txn_body}
        }
    }
}

#[component]
fn ReadOnlyTxnRow(txn: Transaction) -> Element {
    let qty = txn.quantity.map(|q| format!("{q:.4}")).unwrap_or_default();
    let price = txn.price.map(fmt_inr).unwrap_or_default();
    let amount = txn.net_amount.map(fmt_inr).unwrap_or_default();
    rsx! {
        tr { style: "border-top: 1px solid #eee;",
            td { style: "padding: 0.5rem;", "{txn.trade_date}" }
            td { style: "padding: 0.5rem;", "{txn_type_label(&txn.txn_type)}" }
            td { style: "padding: 0.5rem;", "{txn.symbol.clone().unwrap_or_default()}" }
            td { style: "padding: 0.5rem;", "{qty}" }
            td { style: "padding: 0.5rem;", "{price}" }
            td { style: "padding: 0.5rem;", "{amount}" }
        }
    }
}
