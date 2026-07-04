use dioxus::prelude::*;

use crate::portfolio::AuthGuard;
use crate::portfolio_api::{fmt_inr, txn_type_label, FifoLot, Holding, Transaction, TransactionFilter};
use crate::routes::Route;
use crate::sync_portfolio::layout::SyncPortfolioBanner;
use crate::sync_portfolio_api::{remote_fifo_lots, remote_holdings, remote_transactions, sync_remote_exported_at};

#[component]
pub fn SyncPortfolioStockDetail(id: i64, symbol: String) -> Element {
    let sym = symbol.clone();
    let exported_at = use_resource(|| async move { sync_remote_exported_at().await.ok().flatten() });

    let holding: Resource<Result<Option<Holding>, String>> = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            async move {
                let all = remote_holdings(id).await?;
                Ok(all.into_iter().find(|h| h.symbol == s))
            }
        }
    });
    let lots = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            async move { remote_fifo_lots(id, &s).await }
        }
    });
    let txns = use_resource({
        let sym = sym.clone();
        move || {
            let s = sym.clone();
            async move {
                remote_transactions(
                    id,
                    &TransactionFilter {
                        portfolio_id: Some(id),
                        symbol: Some(s),
                        ..Default::default()
                    },
                )
                .await
            }
        }
    });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 1rem;",
                Link { to: Route::SyncPortfolioHoldings { id }, style: "color: #184ad8;", "← Holdings" }
            }
            if let Some(Some(ts)) = exported_at.read().as_ref() {
                SyncPortfolioBanner { exported_at: Some(ts.clone()) }
            }
            h1 { "{symbol}" }
            match &*holding.read() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(None)) => rsx! { p { "No current holding for this symbol" } },
                Some(Ok(Some(h))) => render_holding_stats(h),
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
    let qty = txn
        .quantity
        .map(|q| format!("{q:.2}"))
        .unwrap_or_default();
    rsx! {
        p { style: "font-size: 0.9rem; margin: 0.25rem 0;",
            "{txn.trade_date} — {txn_type_label(&txn.txn_type)} — qty {qty}"
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
