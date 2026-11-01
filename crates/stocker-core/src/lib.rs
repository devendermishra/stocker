//! Yahoo Finance fetch and research-report orchestration.
//!
//! Domain rules live in `stocker-domain`. This crate fetches market data and
//! assembles a [`stocker_domain::ResearchReport`].

pub mod bank_metrics;
pub mod fetcher;
mod fundamentals_timeseries;
mod http_policy;
pub mod paths;
pub mod report;
pub mod sector;
pub mod symbol;
pub mod yahoo_metrics;

pub use report::build_research_report;
pub use symbol::{default_india_symbol_context, load_nse_bases_from_equity_l};
pub use error::{Result, StockerError};

mod error;
