//! NSE-focused stock research core: Yahoo Finance fetch + heuristics.

pub mod analysis;
pub mod fetcher;
pub mod models;
pub mod report;
pub mod sector;
pub mod symbol;

pub use models::*;
pub use report::{ResearchReport, build_research_report};
pub use symbol::normalize_nse_symbol;
pub use error::{Result, StockerError};

mod error;
