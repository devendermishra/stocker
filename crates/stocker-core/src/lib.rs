//! NSE-focused stock research core: Yahoo Finance fetch + heuristics.

pub mod analysis;
pub mod fetcher;
pub mod fundamental_analysis;
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
