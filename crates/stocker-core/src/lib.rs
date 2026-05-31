//! NSE-focused stock research core: Yahoo Finance fetch + heuristics.
//!
//! **Data sources:** Symbol universes are loaded from local CSV files only
//! (`stocker-screener`). Live quotes, fundamentals, and news use Yahoo Finance
//! endpoints — never NSE/BSE exchange APIs or scraping.

pub mod analysis;
pub mod fetcher;
mod fundamentals_timeseries;
pub mod fundamental_analysis;
mod http_policy;
pub mod models;
pub mod report;
pub mod research_summary;
pub mod sector;
pub mod stock_scoring;
pub mod symbol;
pub mod technical_analysis;
pub mod technical_entry_signal;
pub mod valuation_analysis;

pub use models::*;
pub use report::{ResearchReport, build_research_report};
pub use symbol::normalize_nse_symbol;
pub use error::{Result, StockerError};

mod error;
