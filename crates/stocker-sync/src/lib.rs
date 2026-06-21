pub mod backup;
pub mod config;
pub mod drive;
pub mod error;
pub mod manifest;
pub mod oauth;
pub mod service;
pub mod state;
pub mod vault;

pub use config::{
    BACKUP_FILENAME, MANIFEST_FILENAME, PORTFOLIO_DB_NAME, SCREENER_DB_NAME, config_dir,
    pending_restore_path, portfolio_db_path, screener_db_path, OAuthConfig,
};
pub use backup::{apply_pending_restore_if_any, databases_replaceable};
pub use error::{Error, Result};
pub use oauth::{clear_authentication, is_authenticated};
pub use service::{
    SyncAction, SyncRecommendation, SyncStatus, auth, decide, logout, pull, push,
    startup_pull_if_newer, status, sync,
};
pub use vault::{VaultStatus, lock_vault, setup_vault, unlock_vault, vault_status};
