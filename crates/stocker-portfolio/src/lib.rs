//! User portfolio module — transaction-based investment ledger.
//!
//! Portfolio data lives in a separate SQLite file (`portfolio.db`).
//! Market prices and stock metadata come from `stocker-screener`.

pub mod analytics;
pub mod auth;
pub mod db;
pub mod engine;
pub mod error;
pub mod import;
pub mod labels;
pub mod models;
pub mod portfolios;
pub mod returns;
pub mod service;
pub mod mf_schedule;
pub mod portfolio_refresh;
pub mod sip_refresh;
pub mod swp_refresh;
pub mod transactions;

pub use analytics::{allocations_by_label, allocations_by_stock, PortfolioView, PortfolioViewOptions};
pub use auth::{ensure_local_user, AuthSession, LoginRequest, RegisterRequest, LOCAL_USER_EMAIL};
pub use db::default_db_path;
pub use error::{Error, Result};
pub use models::*;
pub use import::{
    build_preview, bulk_import, parse_date, parse_file, parse_number, parse_txn_type, preview_rows,
    ImportApplyRequest, ImportField, ImportParseBody, ImportResult, ImportRowPreview, ParsePreview,
    RawGrid,
};
pub use service::PortfolioService;
pub use mf_schedule::{
    inactivate_schedule, list_mf_schedules, register_mf_schedule, refresh_active_schedules,
};
pub use portfolio_refresh::{
    apply_portfolio_refresh, scan_portfolio_refresh, PendingSipMaterialization,
    PendingSwpMaterialization, PortfolioRefreshApplyResult, PortfolioRefreshScan, ScanError,
    SuggestedCorporateAction, SuggestedSipInstallment, SuggestedSwpInstallment,
};
pub use sip_refresh::{refresh_sip_transactions, SipRefreshFailure, SipRefreshResult};
pub use swp_refresh::{refresh_swp_transactions, SwpRefreshFailure, SwpRefreshResult};
pub use transactions::TransactionFilter;
