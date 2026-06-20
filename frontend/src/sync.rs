use dioxus::prelude::*;

use crate::routes::Route;
use crate::sync_api::{
    action_needs_restart, restart_app, sync_auth, sync_lock_vault, sync_logout, sync_pull,
    sync_push, sync_run, sync_status,
};
use crate::sync_oauth_modal::{OAuthModalMode, SyncOAuthModal};

#[component]
pub fn DriveSync() -> Element {
    let mut status_json = use_signal(|| None::<String>);
    let mut recommendation = use_signal(|| None::<String>);
    let mut authenticated = use_signal(|| false);
    let mut vault_configured = use_signal(|| false);
    let mut vault_unlocked = use_signal(|| false);
    let mut oauth_configured = use_signal(|| false);
    let mut modal_mode = use_signal(|| None::<OAuthModalMode>);
    let mut busy = use_signal(|| false);
    let mut refresh_tick = use_signal(|| 0u32);
    let mut status_busy = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);
    let mut needs_restart = use_signal(|| false);

    use_effect(move || {
        let _ = refresh_tick();
        status_busy.set(true);
        spawn(async move {
            match sync_status().await {
                Ok(st) => {
                    authenticated.set(st.authenticated);
                    vault_configured.set(st.vault_configured);
                    vault_unlocked.set(st.vault_unlocked);
                    oauth_configured.set(st.oauth_configured);
                    recommendation.set(Some(format!("{:?}", st.recommendation)));
                    status_json.set(Some(
                        serde_json::to_string_pretty(&st).unwrap_or_default(),
                    ));
                    message.set(None);

                    if !st.vault_configured {
                        if st.oauth_configured {
                            modal_mode.set(None);
                        } else {
                            modal_mode.set(Some(OAuthModalMode::Setup));
                        }
                    } else if !st.vault_unlocked {
                        modal_mode.set(Some(OAuthModalMode::Unlock));
                    } else {
                        modal_mode.set(None);
                    }
                }
                Err(e) => message.set(Some(e)),
            }
            status_busy.set(false);
        });
    });

    let mut refresh = move || {
        refresh_tick.set(refresh_tick() + 1);
    };

    let vault_ready = move || !vault_configured() || vault_unlocked();
    let controls_disabled = busy() || !vault_ready() || modal_mode().is_some();

    rsx! {
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 760px; margin: 2rem auto; padding: 0 1rem;",
            Link {
                to: Route::Home { id: String::new(), exchange: String::new() },
                style: "color: #184ad8; text-decoration: none; font-size: 0.9rem;",
                "← Home"
            }
            h1 { style: "margin-top: 1rem;", "Google Drive Sync" }
            p { style: "color: #555;",
                "Back up portfolio.db and stocker.db to your Google Drive app folder. On another device, pull the latest backup."
            }

            if vault_configured() && vault_unlocked() {
                div { style: "margin: 0.75rem 0;",
                    span { style: "font-size: 0.85rem; color: #2e7d32;",
                        "Vault unlocked. Credentials are stored encrypted locally."
                    }
                    button {
                        style: "margin-left: 0.75rem; padding: 0.25rem 0.5rem; background: none; border: none; color: #184ad8; cursor: pointer; font-size: 0.85rem;",
                        onclick: move |_| {
                            sync_lock_vault();
                            modal_mode.set(Some(OAuthModalMode::Unlock));
                            vault_unlocked.set(false);
                            authenticated.set(false);
                        },
                        "Lock vault"
                    }
                }
            }

            div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 1rem 0;",
                button {
                    style: "padding: 0.55rem 1rem; background: #184ad8; color: white; border: none; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: controls_disabled,
                    onclick: move |_| {
                        busy.set(true);
                        message.set(None);
                        spawn(async move {
                            let result = sync_run(false).await;
                            busy.set(false);
                            match result {
                                Ok(action) => {
                                    message.set(Some(format!("Sync complete: {action}")));
                                    if action_needs_restart(&action) {
                                        needs_restart.set(true);
                                    }
                                    refresh();
                                }
                                Err(e) => message.set(Some(e)),
                            }
                        });
                    },
                    if busy() { "Syncing…" } else { "Sync now" }
                }
                if vault_ready() && !authenticated() && oauth_configured() {
                    button {
                        style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; font-weight: 600; cursor: pointer;",
                        disabled: controls_disabled,
                        onclick: move |_| {
                            busy.set(true);
                            message.set(None);
                            spawn(async move {
                                let result = sync_auth().await;
                                busy.set(false);
                                match result {
                                    Ok(()) => {
                                        message.set(Some("Signed in to Google Drive.".into()));
                                        refresh();
                                    }
                                    Err(e) => message.set(Some(e)),
                                }
                            });
                        },
                        "Sign in with Google"
                    }
                }
                if vault_ready() && authenticated() {
                    button {
                        style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                        disabled: controls_disabled,
                        onclick: move |_| {
                            match sync_logout() {
                                Ok(()) => {
                                    message.set(Some("Signed out of Google Drive. Sign in again to refresh permissions.".into()));
                                    authenticated.set(false);
                                    refresh();
                                }
                                Err(e) => message.set(Some(e)),
                            }
                        },
                        "Sign out"
                    }
                }
                button {
                    style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: controls_disabled,
                    onclick: move |_| {
                        busy.set(true);
                        spawn(async move {
                            let result = sync_push(true).await;
                            busy.set(false);
                            match result {
                                Ok(action) => message.set(Some(format!("Push: {action}"))),
                                Err(e) => message.set(Some(e)),
                            }
                            refresh();
                        });
                    },
                    "Force push"
                }
                button {
                    style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: controls_disabled,
                    onclick: move |_| {
                        busy.set(true);
                        spawn(async move {
                            let result = sync_pull(true).await;
                            busy.set(false);
                            match result {
                                Ok(action) => {
                                    message.set(Some(format!("Pull: {action}")));
                                    if action_needs_restart(&action) {
                                        needs_restart.set(true);
                                    }
                                }
                                Err(e) => message.set(Some(e)),
                            }
                            refresh();
                        });
                    },
                    "Force pull"
                }
                button {
                    style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: status_busy(),
                    onclick: move |_| refresh(),
                    if status_busy() { "Loading status…" } else { "Get status" }
                }
            }

            if let Some(rec) = recommendation() {
                p { style: "font-size: 0.95rem; color: #333;",
                    strong { "Recommendation: " }
                    "{rec}"
                }
            }

            if let Some(msg) = message() {
                p { style: "margin-top: 0.75rem; color: #333; font-size: 0.92rem;", "{msg}" }
            }

            if needs_restart() {
                div {
                    style: "margin-top: 1rem; padding: 1rem; background: #fff8e1; border: 1px solid #ffe082; border-radius: 8px;",
                    p { "Databases were replaced. Restart the app to reload them." }
                    button {
                        style: "margin-top: 0.5rem; padding: 0.5rem 1rem; background: #184ad8; color: white; border: none; border-radius: 8px; cursor: pointer;",
                        onclick: move |_| {
                            let _ = restart_app();
                        },
                        "Restart app"
                    }
                }
            }

            if let Some(json) = status_json() {
                pre {
                    style: "margin-top: 1.5rem; padding: 1rem; background: #f6f8fa; border-radius: 8px; overflow: auto; font-size: 0.82rem;",
                    "{json}"
                }
            }

            if !vault_configured() {
                p { style: "margin-top: 1.5rem; font-size: 0.85rem; color: #666;",
                    "Set up Google OAuth credentials to enable Drive sync. They will be stored encrypted with your master password."
                }
            }
        }

        if let Some(mode) = modal_mode() {
            SyncOAuthModal {
                mode,
                on_success: move |_| {
                    modal_mode.set(None);
                    refresh();
                },
            }
        }
    }
}
