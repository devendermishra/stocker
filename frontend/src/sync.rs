use dioxus::prelude::*;

use crate::routes::Route;
use crate::sync_api::{
    action_needs_restart, format_recommendation, restart_app, schedule_restart_after_pull,
    sync_auth, sync_lock_vault, sync_logout, sync_pull, sync_push, sync_run, sync_status,
    sync_vault_status,
};
use crate::sync_oauth_modal::{OAuthModalMode, SyncOAuthModal};
use crate::sync_portfolio_api::{sync_remote_browse_index, PortfolioSyncState};

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
    let mut browse_busy = use_signal(|| false);
    let mut browse_tick = use_signal(|| 0u32);
    let mut browse_index = use_signal(|| None::<crate::sync_portfolio_api::RemoteBrowseIndex>);
    let mut message = use_signal(|| None::<String>);
    let mut needs_restart = use_signal(|| false);

    use_effect(move || {
        let _ = refresh_tick();
        let vs = sync_vault_status();
        vault_configured.set(vs.configured);
        vault_unlocked.set(vs.unlocked);

        if vs.configured && !vs.unlocked {
            status_busy.set(false);
            status_json.set(None);
            recommendation.set(None);
            authenticated.set(false);
            oauth_configured.set(false);
            return;
        }

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
                    recommendation.set(Some(format_recommendation(st.recommendation.clone())));
                    status_json.set(Some(
                        serde_json::to_string_pretty(&st).unwrap_or_default(),
                    ));
                    if st.authenticated && st.vault_unlocked {
                        browse_tick.set(browse_tick() + 1);
                    }
                }
                Err(e) => message.set(Some(e)),
            }
            status_busy.set(false);
        });
    });

    use_effect(move || {
        let _ = browse_tick();
        if !authenticated() || !vault_unlocked() {
            browse_index.set(None);
            return;
        }
        browse_busy.set(true);
        spawn(async move {
            match sync_remote_browse_index(false).await {
                Ok(idx) => browse_index.set(Some(idx)),
                Err(e) => message.set(Some(e)),
            }
            browse_busy.set(false);
        });
    });

    let mut refresh = move || {
        refresh_tick.set(refresh_tick() + 1);
        browse_tick.set(browse_tick() + 1);
    };

    let mut refresh_remote_browse = move || {
        if !authenticated() || !vault_unlocked() {
            return;
        }
        browse_busy.set(true);
        spawn(async move {
            match sync_remote_browse_index(true).await {
                Ok(idx) => browse_index.set(Some(idx)),
                Err(e) => message.set(Some(e)),
            }
            browse_busy.set(false);
        });
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

    let mut begin_refresh = move || {
        let vs = sync_vault_status();
        vault_configured.set(vs.configured);
        vault_unlocked.set(vs.unlocked);
        if vs.configured && !vs.unlocked {
            pending_action.set(None);
            modal_mode.set(Some(OAuthModalMode::Unlock));
            return;
        }
        refresh();
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
                "Sync portfolio.db and stocker.db with Google Drive. On a new device, sign in here to see what is on Drive, then pull to sync locally."
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
                    "Master password is required to view status, sync, push, or pull."
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
                    disabled: status_busy() || modal_mode().is_some(),
                    onclick: move |_| begin_refresh(),
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
                    strong { "Next step: " }
                    "{rec}"
                }
            }

            if authenticated() && vault_unlocked() {
                div { style: "margin-top: 1.5rem;",
                    div { style: "display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; margin-bottom: 0.75rem;",
                        h2 { style: "margin: 0; font-size: 1.1rem;", "Portfolio sync status" }
                        button {
                            style: "padding: 0.4rem 0.75rem; background: #fff; color: #333; border: 1px solid #ccc; border-radius: 8px; font-size: 0.85rem; cursor: pointer;",
                            disabled: browse_busy(),
                            onclick: move |_| refresh_remote_browse(),
                            if browse_busy() { "Refreshing…" } else { "Refresh from Drive" }
                        }
                    }
                    if let Some(idx) = browse_index() {
                        if idx.too_large {
                            p { style: "color: #b00020; font-size: 0.9rem;",
                                "Drive backup is too large to preview. Use Force pull to download it locally."
                            }
                        } else if let Some(err) = idx.error {
                            p { style: "color: #b00020; font-size: 0.9rem;", "{err}" }
                        } else if !idx.has_portfolio_db {
                            p { style: "color: #666; font-size: 0.9rem;",
                                "Nothing on Google Drive yet. Push from your main device first."
                            }
                        } else {
                            SyncSummaryCards { summary: idx.summary.clone() }
                            if idx.summary.pending_pull > 0 {
                                div {
                                    style: "margin: 0.75rem 0; padding: 0.85rem 1rem; background: #e8f0fe; border: 1px solid #b6cef7; border-radius: 8px; font-size: 0.9rem; color: #1a3d7c;",
                                    strong { "{idx.summary.pending_pull}" }
                                    " portfolio(s) on Google Drive are not on this device yet. Use "
                                    strong { "Sync now" }
                                    " or "
                                    strong { "Force pull" }
                                    " to download them, then restart the app."
                                }
                            }
                            if let Some(ts) = idx.remote_exported_at {
                                p { style: "color: #666; font-size: 0.85rem; margin: 0.75rem 0;",
                                    "Drive backup exported: {ts}"
                                }
                            }
                            if idx.entries.is_empty() {
                                p { style: "color: #666; font-size: 0.9rem;", "No portfolios in the Drive backup." }
                            } else {
                                div { style: "overflow-x: auto;",
                                    table { style: "width: 100%; border-collapse: collapse; font-size: 0.88rem;",
                                        thead {
                                            tr { style: "background: #f6f8fb; text-align: left;",
                                                th { style: "padding: 0.5rem;", "Portfolio" }
                                                th { style: "padding: 0.5rem;", "Status" }
                                                th { style: "padding: 0.5rem;", "On Drive" }
                                                th { style: "padding: 0.5rem;", "Local" }
                                                th { style: "padding: 0.5rem;", "Remote txns" }
                                                th { style: "padding: 0.5rem;", "" }
                                            }
                                        }
                                        tbody {
                                            for entry in idx.entries.iter() {
                                                tr { style: "border-top: 1px solid #eee;",
                                                    td { style: "padding: 0.5rem;", "{entry.name}" }
                                                    td { style: "padding: 0.5rem;",
                                                        SyncStateBadge { state: entry.state }
                                                    }
                                                    td { style: "padding: 0.5rem; text-align: center;",
                                                        if entry.remote_id.is_some() { "✓" } else { "—" }
                                                    }
                                                    td { style: "padding: 0.5rem; text-align: center;",
                                                        if entry.local_id.is_some() { "✓" } else { "—" }
                                                    }
                                                    td { style: "padding: 0.5rem;",
                                                        "{entry.transaction_count.map(|c| c.to_string()).unwrap_or_else(|| \"—\".into())}"
                                                    }
                                                    td { style: "padding: 0.5rem; white-space: nowrap;",
                                                        if let Some(rid) = entry.remote_id {
                                                            Link {
                                                                to: Route::SyncPortfolioOverview { id: rid },
                                                                style: "color: #184ad8; font-weight: 600; font-size: 0.85rem;",
                                                                "Preview on Drive"
                                                            }
                                                        } else if let Some(lid) = entry.local_id {
                                                            Link {
                                                                to: Route::PortfolioOverview { id: lid },
                                                                style: "color: #184ad8; font-size: 0.85rem;",
                                                                "View local"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if browse_busy() {
                        p { style: "color: #666; font-size: 0.9rem;", "Loading from Google Drive…" }
                    }
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
                details { style: "margin-top: 1.5rem;",
                    summary { style: "cursor: pointer; color: #666; font-size: 0.85rem;", "Technical sync details" }
                    pre {
                        style: "margin-top: 0.5rem; padding: 1rem; background: #f6f8fa; border-radius: 8px; overflow: auto; font-size: 0.82rem;",
                        "{json}"
                    }
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

#[component]
fn SyncSummaryCards(summary: crate::sync_portfolio_api::RemoteBrowseSummary) -> Element {
    const CARD: &str = "background: #fff; border: 1px solid #dfe3eb; border-radius: 10px; padding: 0.75rem 1rem; min-width: 120px;";
    rsx! {
        div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 0.65rem; margin-bottom: 0.5rem;",
            div { style: "{CARD}",
                div { style: "font-size: 0.78rem; color: #666;", "On Google Drive" }
                div { style: "font-size: 1.35rem; font-weight: 700; color: #184ad8;", "{summary.on_drive}" }
            }
            div { style: "{CARD}",
                div { style: "font-size: 0.78rem; color: #666;", "Synced" }
                div { style: "font-size: 1.35rem; font-weight: 700; color: #2e7d32;", "{summary.synced}" }
            }
            div { style: "{CARD}",
                div { style: "font-size: 0.78rem; color: #666;", "Pending pull" }
                div { style: "font-size: 1.35rem; font-weight: 700; color: #1565c0;", "{summary.pending_pull}" }
            }
            div { style: "{CARD}",
                div { style: "font-size: 0.78rem; color: #666;", "Pending push" }
                div { style: "font-size: 1.35rem; font-weight: 700; color: #e65100;", "{summary.pending_push}" }
            }
        }
    }
}

#[component]
fn SyncStateBadge(state: PortfolioSyncState) -> Element {
    let (label, bg, fg) = match state {
        PortfolioSyncState::Matched => ("Synced", "#e8f5e9", "#2e7d32"),
        PortfolioSyncState::DriveOnly => ("Pending pull", "#e8f0fe", "#1565c0"),
        PortfolioSyncState::LocalOnly => ("Pending push", "#fff3e0", "#e65100"),
    };
    rsx! {
        span {
            style: "display: inline-block; padding: 0.15rem 0.5rem; border-radius: 999px; font-size: 0.78rem; font-weight: 600; background: {bg}; color: {fg};",
            "{label}"
        }
    }
}
