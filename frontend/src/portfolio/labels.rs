use dioxus::prelude::*;

use crate::portfolio::layout::AuthGuard;
use crate::portfolio_api::{create_label, delete_label, list_labels, NewLabel};
use crate::routes::Route;

#[component]
pub fn PortfolioLabels() -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let labels = use_resource(move || {
        let _ = reload();
        async move { list_labels().await }
    });

    let labels_body = match &*labels.read_unchecked() {
        None => rsx! { p { "Loading" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", {e.clone()} } },
        Some(Ok(list)) => {
            if list.is_empty() {
                rsx! { p { "No labels yet." } }
            } else {
                rsx! {
                    for l in list.iter() {
                        div { style: "display: flex; justify-content: space-between; align-items: center; padding: 0.5rem 0; border-bottom: 1px solid #eee;",
                            span { "{l.name}" }
                            button {
                                style: "padding: 0.25rem 0.5rem; border: 1px solid #f5c6cb; background: #fff; color: #b00020; border-radius: 6px; cursor: pointer; font-size: 0.8rem;",
                                onclick: {
                                    let lid = l.id;
                                    move |_| {
                                        spawn(async move {
                                            let _ = delete_label(lid).await;
                                            reload.set(reload() + 1);
                                        });
                                    }
                                },
                                "Delete"
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 1rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            h1 { "Labels" }
            div { style: "display: flex; gap: 0.5rem; margin-bottom: 1rem;",
                input {
                    placeholder: "Label name",
                    style: "padding: 0.5rem 0.75rem; border: 1px solid #d5dbe3; border-radius: 8px; flex: 1;",
                    value: "{name}",
                    oninput: move |ev| name.set(ev.value()),
                }
                button {
                    style: "padding: 0.5rem 0.85rem; background: #1a56db; color: #fff; border: none; border-radius: 8px; cursor: pointer;",
                    onclick: move |_| {
                        let n = name();
                        spawn(async move {
                            if n.trim().is_empty() {
                                error.set(Some("Name required".into()));
                                return;
                            }
                            match create_label(&NewLabel { name: n.trim().to_string(), color: None }).await {
                                Ok(_) => {
                                    name.set(String::new());
                                    reload.set(reload() + 1);
                                    error.set(None);
                                }
                                Err(e) => error.set(Some(e)),
                            }
                        });
                    },
                    "Add"
                }
            }
            if let Some(e) = error() {
                p { style: "color: #b00020;", {e} }
            }
            {labels_body}
        }
    }
}
