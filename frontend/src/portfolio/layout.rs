//! Shared portfolio layout and navigation.

use dioxus::prelude::*;

use crate::routes::Route;

const NAV: &str = "display: flex; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; align-items: center;";
const LINK: &str = "color: #1a56db; text-decoration: none; font-weight: 600;";

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
