use dioxus::prelude::*;

use crate::routes::Route;

#[component]
pub fn Home() -> Element {
    let mut input = use_signal(|| String::from("RELIANCE"));
    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 760px; margin: 2rem auto; padding: 0 1rem;",
            h1 { "NSE Stock Researcher" }
            p { style: "color: #555;", "Professional summary from Yahoo-derived data with heuristics. Not investment advice." }
            div { style: "display: flex; gap: 0.5rem; margin-top: 1rem; flex-wrap: wrap;",
                input {
                    style: "flex: 1; min-width: 200px; padding: 0.55rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                    placeholder: "INFY",
                    value: "{input}",
                    oninput: move |e| input.set(e.value()),
                }
                Link {
                    to: Route::Report { symbol: input.cloned().trim().to_string() },
                    style: "padding: 0.55rem 1rem; background: #184ad8; color: white; border-radius: 8px; text-decoration: none; font-weight: 600;",
                    "Generate Report"
                }
            }
        }
    }
}
