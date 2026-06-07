//! NSE-focused stock research core: Yahoo Finance fetch + heuristics.
//!
//! **Data sources:** Symbol universes are loaded from local CSV files only
//! (`stocker-screener`). Live quotes, fundamentals, and news use Yahoo Finance
//! endpoints — never NSE/BSE exchange APIs or scraping.

pub mod analysis;
pub mod bank_metrics;
pub mod fetcher;
mod fundamentals_timeseries;
pub mod financial_strength_audit;
pub mod fundamental_analysis;
mod http_policy;
pub mod math;
pub mod models;
pub mod report;
pub mod research_summary;
pub mod sector;
pub mod statements;
pub mod stock_scoring;
pub mod symbol;
pub mod technical_analysis;
pub mod technical_entry_signal;
pub mod valuation_analysis;

pub use models::*;
pub use financial_strength_audit::{
    build_action_guidance, build_financial_strength_audit, cumulative_cfo_pat_for_bundle,
};
pub use math::{cagr, median, pct_change};
pub use report::{ResearchReport, build_research_report};
pub use symbol::{
    default_india_symbol_context, india_base_symbol, india_display_ticker, india_exchange_label,
    normalize_nse_symbol, nse_csv_indices, resolve_india_symbol, to_yahoo_bse_symbol,
    IndiaSymbolContext, NseCsvIndices,
};
pub use error::{Result, StockerError};

mod error;
