//! NSE stock screener crate. Shared by `stocker-api` (server mode) and the
//! `desktop` feature of `stocker-web` (standalone mode).
//!
//! See `crates/stocker-screener/migrations/0001_init.sql` for the schema.
//! See `metrics.rs` for the catalog of fields.

pub mod compute;
pub mod coverage;
pub mod db;
pub mod error;
pub mod metrics;
pub mod query;
pub mod recompute;
pub mod refresh;
pub mod screens;
pub mod service;
pub mod snapshot;
pub mod universe;
pub mod universe_csv;

pub use error::{Error, Result};
pub use metrics::{MetricCategory, MetricId, MetricSpec, SourceKind, Unit, CATALOG};
pub use query::{fetch_snapshot, FilterOp, ScreenFilter, ScreenQuery, ScreenRow, ScreenValue, SortDir};
pub use refresh::{BackfillStats, RefreshConfig, SchedulerStatus};
pub use screens::{NewSavedScreen, SavedScreen};
pub use coverage::{CoverageReport, CoverageSummary, CoverageTier, MetricCoverage, parent_usable};
pub use recompute::{RecomputeStats, recompute_composites};
pub use service::ScreenerService;
pub use snapshot::StockSnapshot;
