use dioxus::prelude::*;

const OVERLAY: &str = "position: fixed; inset: 0; background: rgba(0,0,0,0.35); z-index: 1000; display: flex; align-items: center; justify-content: center; padding: 1rem;";
const MODAL: &str = "background: #fff; border-radius: 12px; max-width: 520px; width: 100%; max-height: 90vh; overflow: auto; padding: 1.25rem; box-shadow: 0 8px 32px rgba(0,0,0,0.15);";
const INPUT: &str = "width: 100%; padding: 0.55rem 0.65rem; border: 1px solid #ccc; border-radius: 8px; font-size: 0.95rem; box-sizing: border-box;";
const LABEL: &str = "display: block; font-size: 0.85rem; font-weight: 600; margin-bottom: 0.35rem; color: #333;";
const BTN_PRIMARY: &str = "padding: 0.55rem 1rem; background: #184ad8; color: white; border: none; border-radius: 8px; font-weight: 600; cursor: pointer;";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthModalMode {
    Setup,
    Unlock,
}

#[component]
pub fn SyncOAuthModal(
    mode: OAuthModalMode,
    on_success: EventHandler<()>,
) -> Element {
    let mut client_id = use_signal(String::new);
    let mut client_secret = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let do_submit = move |_| {
        busy.set(true);
        error.set(None);
        let mode = mode;
        let cid = client_id();
        let csec = client_secret();
        let pwd = password();
        let cpwd = confirm_password();
        spawn(async move {
            let result = match mode {
                OAuthModalMode::Setup => {
                    if cid.trim().is_empty() || csec.trim().is_empty() {
                        Err("Client ID and client secret are required.".into())
                    } else if pwd.len() < 6 {
                        Err("Master password must be at least 6 characters.".into())
                    } else if pwd != cpwd {
                        Err("Passwords do not match.".into())
                    } else {
                        crate::sync_api::sync_setup_vault(cid.trim().into(), csec.trim().into(), pwd)
                    }
                }
                OAuthModalMode::Unlock => {
                    if pwd.is_empty() {
                        Err("Enter your master password.".into())
                    } else {
                        crate::sync_api::sync_unlock_vault(pwd)
                    }
                }
            };
            busy.set(false);
            match result {
                Ok(()) => on_success.call(()),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    rsx! {
        div { style: "{OVERLAY}",
            div { style: "{MODAL}",
                h2 { style: "margin-top: 0;",
                    if mode == OAuthModalMode::Setup {
                        "Set up Google OAuth"
                    } else {
                        "Unlock sync vault"
                    }
                }

                if mode == OAuthModalMode::Setup {
                    p { style: "color: #666; font-size: 0.9rem; margin-bottom: 1rem;",
                        "Create a Google Cloud Desktop OAuth client with the Drive API enabled. "
                        "Credentials are encrypted locally with your master password."
                    }
                    ol { style: "color: #555; font-size: 0.85rem; padding-left: 1.25rem; margin: 0 0 1rem;",
                        li { "Open the "
                            a {
                                href: "https://console.cloud.google.com/",
                                target: "_blank",
                                style: "color: #184ad8;",
                                "Google Cloud Console"
                            }
                        }
                        li { "Enable the Google Drive API" }
                        li { "Create an OAuth client ID (Desktop app)" }
                        li { "Paste the client ID and secret below" }
                    }

                    label { style: "{LABEL}", "Client ID" }
                    input {
                        style: "{INPUT}",
                        r#type: "text",
                        placeholder: "xxxx.apps.googleusercontent.com",
                        value: "{client_id}",
                        oninput: move |e| client_id.set(e.value()),
                    }

                    label { style: "{LABEL} margin-top: 0.75rem;", "Client secret" }
                    input {
                        style: "{INPUT}",
                        r#type: "password",
                        autocomplete: "off",
                        value: "{client_secret}",
                        oninput: move |e| client_secret.set(e.value()),
                    }

                    label { style: "{LABEL} margin-top: 0.75rem;", "Master password" }
                    input {
                        style: "{INPUT}",
                        r#type: "password",
                        autocomplete: "new-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }

                    label { style: "{LABEL} margin-top: 0.75rem;", "Confirm password" }
                    input {
                        style: "{INPUT}",
                        r#type: "password",
                        autocomplete: "new-password",
                        value: "{confirm_password}",
                        oninput: move |e| confirm_password.set(e.value()),
                    }
                } else {
                    p { style: "color: #666; font-size: 0.9rem; margin-bottom: 1rem;",
                        "Enter your master password to decrypt sync credentials and state."
                    }
                    label { style: "{LABEL}", "Master password" }
                    input {
                        style: "{INPUT}",
                        r#type: "password",
                        autocomplete: "current-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }

                if let Some(err) = error() {
                    p { style: "color: #b00020; font-size: 0.9rem; margin-top: 0.75rem;", "{err}" }
                }

                div { style: "display: flex; gap: 0.5rem; margin-top: 1.25rem; justify-content: flex-end;",
                    button {
                        style: "{BTN_PRIMARY}",
                        disabled: busy(),
                        onclick: do_submit,
                        if busy() {
                            if mode == OAuthModalMode::Setup { "Saving…" } else { "Unlocking…" }
                        } else {
                            if mode == OAuthModalMode::Setup { "Save credentials" } else { "Unlock" }
                        }
                    }
                }
            }
        }
    }
}
