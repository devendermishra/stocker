//! NSE stock screener crate. Shared by `stocker-api` (server mode) and the
//! `desktop` feature of `stocker-web` (standalone mode).
//!
//! See `crates/stocker-screener/migrations/0001_init.sql` for the schema.
//! See `metrics.rs` for the catalog of fields.

pub mod compute;
pub mod coverage;
pub mod db;
pub mod enrichment;
pub mod error;
pub mod metrics;
pub mod query;
pub mod recompute;
pub mod refresh;
pub mod screens;
pub mod sectors;
pub mod service;
pub mod snapshot;
pub mod symbols;
pub mod universe;
pub mod universe_csv;

pub use error::{Error, Result};
pub use metrics::{MetricCategory, MetricId, MetricSpec, SourceKind, Unit, CATALOG};
pub use query::{fetch_snapshot, FilterOp, ScreenFilter, ScreenQuery, ScreenRow, ScreenValue, SortDir};
pub use refresh::{BackfillStats, RefreshConfig, SchedulerStatus, SectorBackfillStats};
pub use screens::{NewSavedScreen, SavedScreen};
pub use sectors::{SectorDetail, SectorListItem, SectorMember};
pub use coverage::{CoverageReport, CoverageSummary, CoverageTier, MetricCoverage, parent_usable};
pub use recompute::{RecomputeStats, recompute_composites};
pub use enrichment::{
    snapshot_is_fresh, snapshot_to_enrichment, DEFAULT_SNAPSHOT_MAX_AGE_SECS,
};
pub use service::ScreenerService;
pub use symbols::{search_symbols, symbol_pair, symbol_pair_from_id, resolve_ticker, SymbolListing, SymbolPair};
pub use universe::{
    india_symbol_context_from_db, india_symbol_context_from_discovered, nse_bases_from_db,
    nse_bases_from_discovered, discover_universe_all, DEFAULT_BSE_EQUITY_L,
};
pub use universe_csv::{
    bse_universe_csv_path, detect_universe_csv_format, merge_india_universes,
    parse_bse_universe_csv, parse_nse_universe_csv, ENV_BSE_UNIVERSE_CSV, UniverseCsvFormat,
};
pub use snapshot::StockSnapshot;
