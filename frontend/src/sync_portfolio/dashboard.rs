use dioxus::prelude::*;

use crate::portfolio::{AllocationTable, AuthGuard};
use crate::routes::Route;
use crate::sync_portfolio::layout::{SyncPortfolioBanner, SyncPortfolioNav, SyncPortfolioTab};
use crate::sync_portfolio_api::{
    remote_allocation_label, remote_allocation_stock, remote_dashboard, sync_remote_exported_at,
};

#[component]
pub fn SyncPortfolioDashboard(id: i64) -> Element {
    let exported_at = use_resource(|| async move { sync_remote_exported_at().await.ok().flatten() });
    let dash = use_resource(move || async move { remote_dashboard(id).await });
    let alloc_stock = use_resource(move || async move { remote_allocation_stock(id).await });
    let alloc_label = use_resource(move || async move { remote_allocation_label(id).await });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::DriveSync {}, style: "color: #184ad8;", "← Google Drive Sync" }
            }
            if let Some(Some(ts)) = exported_at.read().as_ref() {
                SyncPortfolioBanner { exported_at: Some(ts.clone()) }
            }
            SyncPortfolioNav { id, active: SyncPortfolioTab::Dashboard }
            match &*dash.read_unchecked() {
                None => rsx! { p { "Loading…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
                Some(Ok(d)) => rsx! {
                    h1 { style: "margin-top: 0;", "{d.portfolio.name}" }
                    p { style: "color: #666; margin-bottom: 1.25rem; font-size: 0.9rem;",
                        "Allocation breakdown from the Drive backup (read-only)."
                    }
                },
            }
            h2 { style: "margin-top: 0.5rem;", "Allocation by stock" }
            AllocationTable { data: alloc_stock.read_unchecked().clone() }
            h2 { style: "margin-top: 1.5rem;", "Allocation by label" }
            AllocationTable { data: alloc_label.read_unchecked().clone() }
        }
    }
}
