//! Public façade used by both `stocker-api` and the desktop frontend.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::coverage::CoverageReport;
use crate::error::{Error, Result};
use crate::metrics::{MetricSpec, CATALOG};
use crate::query::{run_query, ScreenQuery, ScreenRow};
use crate::refresh::{BackfillStats, RefreshConfig, RefreshScheduler, SchedulerStatus};
use crate::screens::{self, NewSavedScreen, SavedScreen};
use crate::{db, refresh};

/// One handle bundling DB + scheduler. Cloneable: callers stash an `Arc` of it
/// in their app state.
#[derive(Clone)]
pub struct ScreenerService {
    pool: SqlitePool,
    scheduler: Arc<RefreshScheduler>,
    backfill_running: Arc<AtomicBool>,
}

impl ScreenerService {
    /// Open the DB at `path` and prepare a non-running scheduler.
    /// Call [`Self::start`] to spawn the refresh loop.
    pub async fn open(path: &Path, config: RefreshConfig) -> Result<Self> {
        let pool = db::open(path).await?;
        let scheduler = Arc::new(RefreshScheduler::new(pool.clone(), config));
        Ok(Self {
            pool,
            scheduler,
            backfill_running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start the background refresh task. Idempotent.
    pub fn start(&self) {
        self.scheduler.spawn();
    }

    /// Stop the background refresh loop.
    pub fn stop_scheduler(&self) {
        self.scheduler.stop();
    }

    /// Clear refresh timestamps so symbols are due again.
    pub async fn reset_refresh_jobs(&self) -> Result<u64> {
        refresh::reset_refresh_jobs(&self.pool).await
    }

    /// Stop scheduler, reset jobs, and refresh every symbol in the foreground.
    pub async fn backfill(&self, config: &RefreshConfig) -> Result<BackfillStats> {
        self.scheduler.stop();
        let n = refresh::reset_refresh_jobs(&self.pool).await?;
        log::info!("backfill: reset {n} symbols");
        refresh::backfill_all(&self.pool, config).await
    }

    /// Start a universe backfill in the background. Returns [`Error::AlreadyRunning`]
    /// if a backfill is already in progress.
    pub fn try_start_backfill(&self, config: RefreshConfig) -> Result<()> {
        if self
            .backfill_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return Err(Error::AlreadyRunning);
        }
        let svc = self.clone();
        tokio::spawn(async move {
            let result = svc.backfill(&config).await;
            match result {
                Ok(stats) => log::info!(
                    "screener backfill finished: {} ok, {} errors",
                    stats.refreshed,
                    stats.errors
                ),
                Err(e) => log::warn!("screener backfill failed: {e}"),
            }
            svc.start();
            svc.backfill_running.store(false, Ordering::Relaxed);
        });
        Ok(())
    }

    /// Force a refresh of one symbol now (synchronously) — used by the CLI and
    /// integration tests.
    pub async fn refresh_now(&self, symbol: &str) -> Result<()> {
        refresh::refresh_one(&self.pool, symbol).await
    }

    /// Snapshot metrics for one symbol (normalized NSE ticker).
    pub async fn snapshot_for(&self, symbol: &str) -> Result<Option<crate::query::ScreenRow>> {
        let symbol = stocker_core::normalize_nse_symbol(symbol).map_err(|e| {
            crate::error::Error::InvalidQuery(e.to_string())
        })?;
        crate::query::fetch_snapshot(&self.pool, &symbol).await
    }

    pub async fn run_query(&self, q: &ScreenQuery) -> Result<Vec<ScreenRow>> {
        run_query(&self.pool, q).await
    }

    pub fn catalog(&self) -> &'static [MetricSpec] {
        CATALOG
    }

    pub async fn status(&self) -> Result<SchedulerStatus> {
        let backfill = self.backfill_running.load(Ordering::Relaxed);
        self.scheduler.status(backfill).await
    }

    pub async fn coverage(&self) -> Result<CoverageReport> {
        crate::coverage::coverage_report(&self.pool).await
    }

    pub async fn recompute_composites(&self) -> Result<crate::recompute::RecomputeStats> {
        crate::recompute::recompute_composites(&self.pool).await
    }

    pub async fn list_screens(&self) -> Result<Vec<SavedScreen>> {
        screens::list(&self.pool).await
    }

    pub async fn get_screen(&self, id: i64) -> Result<SavedScreen> {
        screens::get(&self.pool, id).await
    }

    pub async fn create_screen(&self, new: &NewSavedScreen) -> Result<SavedScreen> {
        screens::create(&self.pool, new).await
    }

    pub async fn update_screen(&self, id: i64, new: &NewSavedScreen) -> Result<SavedScreen> {
        screens::update(&self.pool, id, new).await
    }

    pub async fn delete_screen(&self, id: i64) -> Result<()> {
        screens::delete(&self.pool, id).await
    }

    /// Underlying pool (for advanced uses like the CLI dump).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
