//! Mutual fund NAV cache and mfapi.in integration.
//!
//! Scheme metadata and latest NAV live in a separate SQLite file (`mf.db`).

pub mod db;
pub mod error;
pub mod fetcher;
pub mod models;
pub mod scheme_index;
pub mod service;
pub mod trading_day;

pub use db::default_db_path;
pub use error::{Error, Result};
pub use models::{
    is_mutual_fund_symbol, mf_symbol, parse_mf_symbol, MfSearchHit, NavPoint, NavSnapshot,
};
pub use scheme_index::{
    default_scheme_list_cache_path, load_scheme_index_from_file, save_scheme_list_cache,
    SchemeIndex, SchemeListEntry,
};
pub use service::{resolve_mf_symbol, MfService};
pub use trading_day::{is_trading_day, should_refresh_nav};
