//! User portfolio module — transaction-based investment ledger.
//!
//! Portfolio data lives in a separate SQLite file (`portfolio.db`).
//! Market prices and stock metadata come from `stocker-screener`.

pub mod auth;
pub mod db;
pub mod engine;
pub mod error;
pub mod labels;
pub mod models;
pub mod portfolios;
pub mod service;
pub mod transactions;

pub use auth::{ensure_local_user, AuthSession, LoginRequest, RegisterRequest, LOCAL_USER_EMAIL};
pub use db::default_db_path;
pub use error::{Error, Result};
pub use models::*;
pub use service::PortfolioService;
pub use transactions::TransactionFilter;
