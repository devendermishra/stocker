use dioxus::prelude::*;

use super::styles::{CANCEL_BTN, CONFIRM_BOX, CONFIRM_BTN};

#[component]
pub fn ConfirmDialog(
    title: String,
    message: Element,
    confirm_label: &'static str,
    confirming: bool,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { style: "{CONFIRM_BOX}",
            p { style: "margin: 0 0 0.75rem 0;",
                strong { "{title}" }
            }
            div { style: "margin: 0 0 0.75rem 0; color: #555; font-size: 0.9rem;",
                {message}
            }
            div { style: "display: flex; gap: 0.5rem;",
                button {
                    style: "{CONFIRM_BTN}",
                    disabled: confirming,
                    onclick: move |_| on_confirm.call(()),
                    "{confirm_label}"
                }
                button {
                    style: "{CANCEL_BTN}",
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
            }
        }
    }
}
