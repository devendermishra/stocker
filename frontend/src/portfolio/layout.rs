//! Shared portfolio layout and navigation.

use dioxus::prelude::*;

use crate::routes::Route;

const NAV: &str = "display: flex; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; align-items: center;";
const LINK: &str = "color: #1a56db; text-decoration: none; font-weight: 600;";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PortfolioTab {
    Overview,
    Holdings,
    Transactions,
    Schedules,
    Dashboard,
}

#[component]
pub fn PortfolioNav(id: i64, active: PortfolioTab) -> Element {
    let tab_style = |tab: PortfolioTab| {
        if active == tab {
            "padding: 0.5rem 0.9rem; border-radius: 8px 8px 0 0; background: #1a56db; color: #fff; text-decoration: none; font-weight: 600; font-size: 0.9rem;"
        } else {
            "padding: 0.5rem 0.9rem; border-radius: 8px 8px 0 0; background: #f6f8fb; color: #374151; text-decoration: none; border: 1px solid #dfe3eb; border-bottom: none; font-size: 0.9rem;"
        }
    };
    rsx! {
        div { style: "display: flex; gap: 0.15rem; border-bottom: 2px solid #1a56db; margin-bottom: 1.25rem; flex-wrap: wrap;",
            Link { to: Route::PortfolioOverview { id }, style: "{tab_style(PortfolioTab::Overview)}", "Overview" }
            Link { to: Route::PortfolioHoldings { id }, style: "{tab_style(PortfolioTab::Holdings)}", "Holdings" }
            Link { to: Route::PortfolioTransactions { id }, style: "{tab_style(PortfolioTab::Transactions)}", "Transactions" }
            Link { to: Route::PortfolioSchedules { id }, style: "{tab_style(PortfolioTab::Schedules)}", "SIPs & SWPs" }
            Link { to: Route::PortfolioDashboard { id }, style: "{tab_style(PortfolioTab::Dashboard)}", "Dashboard" }
        }
    }
}

#[component]
pub fn PortfolioLayout(children: Element) -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1100px; margin: 1.5rem auto; padding: 0 1rem;",
            div { style: "{NAV}",
                Link { to: Route::Home { id: String::new(), exchange: String::new() }, style: "{LINK}", "Research" }
                Link { to: Route::Screener {}, style: "{LINK}", "Screener" }
                Link { to: Route::Stocks {}, style: "{LINK}", "Stocks" }
                Link { to: Route::PortfolioList {}, style: "{LINK}", "Portfolio" }
            }
            {children}
        }
    }
}

/// Wrapper kept for compatibility — portfolio pages no longer require login.
#[component]
pub fn AuthGuard(children: Element) -> Element {
    rsx! {
        PortfolioLayout { {children} }
    }
}
