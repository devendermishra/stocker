use dioxus::prelude::*;

use crate::routes::Route;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SyncPortfolioTab {
    Overview,
    Holdings,
    Transactions,
    Dashboard,
}

#[component]
pub fn SyncPortfolioNav(id: i64, active: SyncPortfolioTab) -> Element {
    let tab_style = |tab: SyncPortfolioTab| {
        if active == tab {
            "padding: 0.5rem 0.9rem; border-radius: 8px 8px 0 0; background: #184ad8; color: #fff; text-decoration: none; font-weight: 600; font-size: 0.9rem;"
        } else {
            "padding: 0.5rem 0.9rem; border-radius: 8px 8px 0 0; background: #f6f8fb; color: #374151; text-decoration: none; border: 1px solid #dfe3eb; border-bottom: none; font-size: 0.9rem;"
        }
    };
    rsx! {
        div { style: "display: flex; gap: 0.15rem; border-bottom: 2px solid #184ad8; margin-bottom: 1.25rem; flex-wrap: wrap;",
            Link { to: Route::SyncPortfolioOverview { id }, style: "{tab_style(SyncPortfolioTab::Overview)}", "Overview" }
            Link { to: Route::SyncPortfolioHoldings { id }, style: "{tab_style(SyncPortfolioTab::Holdings)}", "Holdings" }
            Link { to: Route::SyncPortfolioTransactions { id }, style: "{tab_style(SyncPortfolioTab::Transactions)}", "Transactions" }
            Link { to: Route::SyncPortfolioDashboard { id }, style: "{tab_style(SyncPortfolioTab::Dashboard)}", "Dashboard" }
        }
    }
}

#[component]
pub fn SyncPortfolioBanner(exported_at: Option<String>) -> Element {
    let ts = exported_at.unwrap_or_else(|| "unknown time".into());
    rsx! {
        div {
            style: "margin: 0 0 1rem 0; padding: 0.85rem 1rem; background: #e8f0fe; border: 1px solid #b6cef7; border-radius: 8px; font-size: 0.9rem; color: #1a3d7c;",
            strong { "Read-only: " }
            "Viewing Google Drive backup from {ts}. Use "
            Link { to: Route::DriveSync {}, style: "color: #184ad8; font-weight: 600;", "Sync" }
            " → Force pull to use this data locally."
        }
    }
}
