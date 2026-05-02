use dioxus::prelude::*;

#[component]
pub fn MetricCard(label: String, value: String) -> Element {
    rsx! {
        div {
            style: "background:#fff; border:1px solid #dfe3eb; border-radius:12px; padding:0.7rem 0.8rem;",
            p { style: "margin:0; font-size:0.8rem; color:#667085;", "{label}" }
            p { style: "margin:0.15rem 0 0; font-size:1rem; font-weight:600;", "{value}" }
        }
    }
}

#[component]
pub fn KeyValue(label: String, value: String) -> Element {
    rsx! {
        div { style: "display:flex; justify-content:space-between; gap:1rem; border-top:1px solid #edf1f7; padding:0.4rem 0;",
            span { style: "color:#556074;", "{label}" }
            span { style: "font-weight:600;", "{value}" }
        }
    }
}
