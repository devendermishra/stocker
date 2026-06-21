use dioxus::prelude::*;

use crate::routes::Route;
use crate::sync_api::{
    action_needs_restart, restart_app, schedule_restart_after_pull, sync_auth, sync_lock_vault,
    sync_logout, sync_pull, sync_push, sync_run, sync_status,
};
use crate::sync_oauth_modal::{OAuthModalMode, SyncOAuthModal};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingSyncAction {
    Run,
    Push,
    Pull,
    Auth,
}

#[component]
pub fn DriveSync() -> Element {
    let mut status_json = use_signal(|| None::<String>);
    let mut recommendation = use_signal(|| None::<String>);
    let mut authenticated = use_signal(|| false);
    let mut vault_configured = use_signal(|| false);
    let mut vault_unlocked = use_signal(|| false);
    let mut oauth_configured = use_signal(|| false);
    let mut portfolio_db_present = use_signal(|| true);
    let mut last_backup_has_portfolio = use_signal(|| true);
    let mut remote_backup_has_portfolio = use_signal(|| true);
    let mut modal_mode = use_signal(|| None::<OAuthModalMode>);
    let mut pending_action = use_signal(|| None::<PendingSyncAction>);
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
                    portfolio_db_present.set(st.portfolio_db_present);
                    last_backup_has_portfolio.set(
                        st.last_backup_files.is_empty()
                            || st.last_backup_files.iter().any(|f| f == "portfolio.db"),
                    );
                    remote_backup_has_portfolio.set(
                        st.remote_backup_files.is_empty()
                            || st.remote_backup_files.iter().any(|f| f == "portfolio.db"),
                    );
                    recommendation.set(Some(format!("{:?}", st.recommendation)));
                    status_json.set(Some(
                        serde_json::to_string_pretty(&st).unwrap_or_default(),
                    ));
                }
                Err(e) => message.set(Some(e)),
            }
            status_busy.set(false);
        });
    });

    let mut refresh = move || {
        refresh_tick.set(refresh_tick() + 1);
    };

    let mut run_pending = move |action: PendingSyncAction| {
        message.set(None);
        busy.set(true);
        spawn(async move {
            let result = match action {
                PendingSyncAction::Run => sync_run(false).await.map(|action| {
                    if action_needs_restart(&action) {
                        needs_restart.set(true);
                        schedule_restart_after_pull();
                        format!("Sync complete. Restarting to load synced databases…")
                    } else {
                        format!("Sync complete: {action}")
                    }
                }),
                PendingSyncAction::Push => sync_push(true)
                    .await
                    .map(|action| format!("Push: {action}")),
                PendingSyncAction::Pull => sync_pull(true).await.map(|action| {
                    if action_needs_restart(&action) {
                        needs_restart.set(true);
                        schedule_restart_after_pull();
                        format!("Pull complete. Restarting to load synced portfolios…")
                    } else {
                        format!("Pull: {action}")
                    }
                }),
                PendingSyncAction::Auth => sync_auth()
                    .await
                    .map(|_| "Signed in to Google Drive.".into()),
            };
            busy.set(false);
            match result {
                Ok(msg) => message.set(Some(msg)),
                Err(e) => message.set(Some(e)),
            }
            refresh();
        });
    };

    let mut begin_sync = move |action: PendingSyncAction| {
        if !vault_configured() {
            if !oauth_configured() {
                pending_action.set(Some(action));
                modal_mode.set(Some(OAuthModalMode::Setup));
                return;
            }
            run_pending(action);
            return;
        }
        if !vault_unlocked() {
            pending_action.set(Some(action));
            modal_mode.set(Some(OAuthModalMode::Unlock));
            return;
        }
        run_pending(action);
    };

    let controls_disabled = move || busy() || modal_mode().is_some();

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
                            vault_unlocked.set(false);
                            authenticated.set(false);
                        },
                        "Lock vault"
                    }
                }
            } else if vault_configured() {
                p { style: "font-size: 0.85rem; color: #666; margin: 0.75rem 0;",
                    "Master password is required when you sync, push, or pull."
                }
            }

            div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 1rem 0;",
                button {
                    style: "padding: 0.55rem 1rem; background: #184ad8; color: white; border: none; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: controls_disabled(),
                    onclick: move |_| begin_sync(PendingSyncAction::Run),
                    if busy() { "Syncing…" } else { "Sync now" }
                }
                if !authenticated() && oauth_configured() {
                    button {
                        style: "padding: 0.55rem 1rem; background: #fff; color: #184ad8; border: 1px solid #184ad8; border-radius: 8px; font-weight: 600; cursor: pointer;",
                        disabled: controls_disabled(),
                        onclick: move |_| begin_sync(PendingSyncAction::Auth),
                        "Sign in with Google"
                    }
                }
                if vault_unlocked() && authenticated() {
                    button {
                        style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                        disabled: controls_disabled(),
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
                    disabled: controls_disabled(),
                    onclick: move |_| begin_sync(PendingSyncAction::Push),
                    "Force push"
                }
                button {
                    style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: controls_disabled(),
                    onclick: move |_| begin_sync(PendingSyncAction::Pull),
                    "Force pull"
                }
                button {
                    style: "padding: 0.55rem 1rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-weight: 600; cursor: pointer;",
                    disabled: status_busy(),
                    onclick: move |_| refresh(),
                    if status_busy() { "Loading status…" } else { "Get status" }
                }
            }

            if !portfolio_db_present() {
                div {
                    style: "margin: 1rem 0; padding: 1rem; background: #fff3e0; border: 1px solid #ffcc80; border-radius: 8px;",
                    p { style: "margin: 0; font-size: 0.92rem; color: #333;",
                        "Portfolio database not found. Open the Portfolio page once, then use "
                        strong { "Force push" }
                        "."
                    }
                }
            } else if authenticated() && !remote_backup_has_portfolio() {
                div {
                    style: "margin: 1rem 0; padding: 1rem; background: #ffebee; border: 1px solid #ef9a9a; border-radius: 8px;",
                    p { style: "margin: 0; font-size: 0.92rem; color: #333;",
                        "The Google Drive backup does "
                        strong { "not" }
                        " contain portfolio.db. A push from the other device did not upload portfolios (often an older app build or wrong database path). On the device that shows your portfolios, update the app and use "
                        strong { "Force push" }
                        " again."
                    }
                }
            } else if !last_backup_has_portfolio() {
                div {
                    style: "margin: 1rem 0; padding: 1rem; background: #fff3e0; border: 1px solid #ffcc80; border-radius: 8px;",
                    p { style: "margin: 0; font-size: 0.92rem; color: #333;",
                        "The last Google Drive backup did not include portfolio.db. Use "
                        strong { "Force push" }
                        " on this device to upload your portfolios."
                    }
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
                    "Use Sync now, Force push, or Force pull to set up Google OAuth credentials. They will be stored encrypted with your master password."
                }
            }
        }

        if let Some(mode) = modal_mode() {
            SyncOAuthModal {
                mode,
                on_success: move |_| {
                    modal_mode.set(None);
                    vault_configured.set(true);
                    vault_unlocked.set(true);
                    let action = pending_action();
                    pending_action.set(None);
                    if let Some(action) = action {
                        run_pending(action);
                    } else {
                        refresh();
                    }
                },
            }
        }
    }
}
