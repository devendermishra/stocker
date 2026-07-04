pub mod backup;
pub mod config;
pub mod drive;
pub mod error;
pub mod manifest;
pub mod oauth;
pub mod remote;
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
pub use remote::{
    invalidate_remote_cache, load_local_portfolio_refs, remote_browse_index, remote_exported_at,
    remote_portfolio_allocation_label, remote_portfolio_allocation_stock, remote_portfolio_dashboard,
    remote_portfolio_holdings, remote_portfolio_stock_lots, remote_portfolio_transactions,
    LocalPortfolioRef, PortfolioSyncEntry, PortfolioSyncState, RemoteBrowseIndex, RemoteBrowseSummary,
};
pub use service::{
    SyncAction, SyncRecommendation, SyncStatus, auth, decide, logout, pull, push,
    startup_pull_if_newer, status, sync,
};
pub use vault::{VaultStatus, lock_vault, setup_vault, unlock_vault, vault_status};
