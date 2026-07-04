//! Google Drive sync API (desktop only).

#[cfg(feature = "desktop")]
pub use desktop::*;

#[cfg(feature = "desktop")]
mod desktop {
    use stocker_sync::{
        OAuthConfig, SyncStatus, VaultStatus, auth, lock_vault, logout, pull, push, setup_vault,
        startup_pull_if_newer, status, sync, unlock_vault, vault_status,
    };

    pub async fn sync_status() -> Result<SyncStatus, String> {
        status().await.map_err(|e| e.to_string())
    }

    pub fn format_recommendation(rec: stocker_sync::SyncRecommendation) -> String {
        use stocker_sync::SyncRecommendation;
        match rec {
            SyncRecommendation::Pull => {
                "Pull from Google Drive — newer backup available on Drive.".into()
            }
            SyncRecommendation::Push => {
                "Push to Google Drive — this device has changes not yet uploaded.".into()
            }
            SyncRecommendation::FirstPush => {
                "Push to Google Drive — no backup on Drive yet (or sign in and refresh).".into()
            }
            SyncRecommendation::Conflict => {
                "Conflict — both Drive and this device changed. Choose Force push or Force pull.".into()
            }
            SyncRecommendation::AlreadyInSync => "Already in sync.".into(),
        }
    }

    pub async fn sync_remote_browse_index(
        force_refresh: bool,
    ) -> Result<stocker_sync::RemoteBrowseIndex, String> {
        let local = crate::portfolio_api::local_portfolio_refs_for_sync().await?;
        stocker_sync::remote_browse_index(force_refresh, local)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn sync_auth() -> Result<(), String> {
        auth().await.map_err(|e| e.to_string())
    }

    pub async fn sync_run(force: bool) -> Result<String, String> {
        let action = sync(force).await.map_err(|e| e.to_string())?;
        let formatted = format!("{action:?}");
        if action_needs_restart(&formatted) {
            crate::portfolio_api::invalidate_portfolio_service();
        }
        Ok(formatted)
    }

    pub async fn sync_push(force: bool) -> Result<String, String> {
        push(force)
            .await
            .map(|a| format!("{a:?}"))
            .map_err(|e| e.to_string())
    }

    pub async fn sync_pull(force: bool) -> Result<String, String> {
        let action = pull(force).await.map_err(|e| e.to_string())?;
        let formatted = format!("{action:?}");
        if action_needs_restart(&formatted) {
            crate::portfolio_api::invalidate_portfolio_service();
        }
        Ok(formatted)
    }

    pub async fn sync_startup_pull() -> Result<Option<String>, String> {
        startup_pull_if_newer()
            .await
            .map(|action| action.map(|a| format!("{a:?}")))
            .map_err(|e| e.to_string())
    }

    pub fn sync_vault_status() -> VaultStatus {
        vault_status()
    }

    pub fn sync_setup_vault(
        client_id: String,
        client_secret: String,
        password: String,
    ) -> Result<(), String> {
        setup_vault(
            &password,
            OAuthConfig {
                client_id,
                client_secret,
            },
        )
        .map_err(|e| e.to_string())
    }

    pub fn sync_unlock_vault(password: String) -> Result<(), String> {
        unlock_vault(&password).map_err(|e| e.to_string())
    }

    pub fn sync_lock_vault() {
        lock_vault();
    }

    pub fn sync_logout() -> Result<(), String> {
        logout().map_err(|e| e.to_string())
    }

    pub fn action_needs_restart(action: &str) -> bool {
        action.contains("Pulled")
    }

    pub fn restart_app() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe)
            .args(std::env::args().skip(1))
            .spawn()
            .map_err(|e| e.to_string())?;
        std::process::exit(0);
    }

    /// Pull replaces database files on disk; restart so in-memory pools reload them.
    pub fn schedule_restart_after_pull() {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let _ = restart_app();
        });
    }
}

#[cfg(not(feature = "desktop"))]
mod stub {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SyncStatus {
        pub authenticated: bool,
        pub vault_configured: bool,
        pub vault_unlocked: bool,
        pub oauth_configured: bool,
        pub recommendation: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct VaultStatus {
        pub configured: bool,
        pub unlocked: bool,
        pub has_oauth: bool,
        pub authenticated: bool,
    }

    pub async fn sync_status() -> Result<SyncStatus, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_auth() -> Result<(), String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_run(_force: bool) -> Result<String, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_push(_force: bool) -> Result<String, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_pull(_force: bool) -> Result<String, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_startup_pull() -> Result<Option<String>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub fn sync_vault_status() -> VaultStatus {
        VaultStatus {
            configured: false,
            unlocked: false,
            has_oauth: false,
            authenticated: false,
        }
    }

    pub fn sync_setup_vault(
        _client_id: String,
        _client_secret: String,
        _password: String,
    ) -> Result<(), String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub fn sync_unlock_vault(_password: String) -> Result<(), String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub fn sync_lock_vault() {}

    pub fn sync_logout() -> Result<(), String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub fn action_needs_restart(_action: &str) -> bool {
        false
    }

    pub fn restart_app() -> Result<(), String> {
        Err("Restart is only available in the desktop app".into())
    }

    pub fn schedule_restart_after_pull() {}
    pub fn format_recommendation(_rec: ()) -> String {
        String::new()
    }
}

#[cfg(not(feature = "desktop"))]
pub use stub::*;
