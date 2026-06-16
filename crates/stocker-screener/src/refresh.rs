//! Tiered refresh scheduler. A single tokio task pulls due symbols, runs the
//! deep enrich path, and upserts snapshots — paced so Yahoo doesn't rate-limit us.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::Notify;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use stocker_core::fetcher::{
    fetch_asset_profile, fetch_chart_history_10y, fetch_financials, fetch_price,
    fetch_statement_bundle,
};

use crate::compute::{compute_all, ComputeInputs};
use crate::error::Result;
use crate::metrics::MetricId;
use crate::snapshot::{count_pending, count_symbols, next_due_symbol, StockSnapshot, SymbolRow};
use crate::universe;

#[derive(Debug, Clone)]
pub struct RefreshConfig {
    pub tier0_interval_secs: i64,
    pub tier1_interval_secs: i64,
    pub pacing_ms: u64,
    pub burst_pause_every_n: u32,
    pub burst_pause_ms: u64,
    pub universe_sync_interval_secs: i64,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl RefreshConfig {
    /// Read defaults from environment variables (used by both server + standalone).
    pub fn from_env() -> Self {
        fn env_u64(name: &str, default: u64) -> u64 {
            std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
        }
        fn env_i64(name: &str, default: i64) -> i64 {
            std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
        }
        fn env_u32(name: &str, default: u32) -> u32 {
            std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
        }
        Self {
            tier0_interval_secs: env_i64("SCREENER_TIER0_INTERVAL_SECS", 24 * 60 * 60),
            tier1_interval_secs: env_i64("SCREENER_TIER1_INTERVAL_SECS", 7 * 24 * 60 * 60),
            // ~800ms between symbols (~3× faster than the old 2500ms default).
            // If Yahoo starts rate-limiting, raise via SCREENER_PACING_MS (e.g. 1500).
            pacing_ms: env_u64("SCREENER_PACING_MS", 800),
            burst_pause_every_n: env_u32("SCREENER_BURST_PAUSE_EVERY_N", 50),
            burst_pause_ms: env_u64("SCREENER_BURST_PAUSE_MS", 15_000),
            universe_sync_interval_secs: env_i64("SCREENER_UNIVERSE_SYNC_INTERVAL_SECS", 24 * 60 * 60),
        }
    }
}

/// Status snapshot for the UI footer / `/status` endpoint.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchedulerStatus {
    pub running: bool,
    pub backfill_running: bool,
    pub universe_size: i64,
    pub pending_count: i64,
    pub last_universe_sync_at: i64,
}

#[derive(Debug)]
pub struct RefreshScheduler {
    pool: SqlitePool,
    config: RefreshConfig,
    cancel: Arc<std::sync::Mutex<CancellationToken>>,
    running: Arc<std::sync::atomic::AtomicBool>,
    notify_kick: Arc<Notify>,
}

impl RefreshScheduler {
    pub fn new(pool: SqlitePool, config: RefreshConfig) -> Self {
        Self {
            pool,
            config,
            cancel: Arc::new(std::sync::Mutex::new(CancellationToken::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            notify_kick: Arc::new(Notify::new()),
        }
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.lock().unwrap().clone()
    }

    pub fn config(&self) -> &RefreshConfig {
        &self.config
    }

    /// Spawn the loop on the current tokio runtime. No-op if already running.
    pub fn spawn(&self) -> tokio::task::JoinHandle<()> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return tokio::spawn(async {});
        }
        let cancel = {
            let mut guard = self.cancel.lock().unwrap();
            if guard.is_cancelled() {
                *guard = CancellationToken::new();
            }
            guard.clone()
        };
        let pool = self.pool.clone();
        let cfg = self.config.clone();
        let running = self.running.clone();
        let kick = self.notify_kick.clone();
        running.store(true, std::sync::atomic::Ordering::Relaxed);
        tokio::spawn(async move {
            log::info!(
                "screener refresh starting (pacing {}ms, tier0 {}s, tier1 {}s)",
                cfg.pacing_ms, cfg.tier0_interval_secs, cfg.tier1_interval_secs
            );
            let mut burst = 0u32;
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                if let Err(e) = sync_universe_if_due(&pool, &cfg).await {
                    log::warn!("screener universe sync failed: {e}");
                }
                let due = match next_due_symbol(&pool, cfg.tier0_interval_secs, cfg.tier1_interval_secs).await {
                    Ok(opt) => opt,
                    Err(e) => {
                        log::warn!("screener pick: {e}");
                        sleep_or_kick(&cancel, &kick, Duration::from_secs(30)).await;
                        continue;
                    }
                };
                let Some(symbol) = due else {
                    // Nothing due; wait until kicked or 5 minutes pass.
                    sleep_or_kick(&cancel, &kick, Duration::from_secs(5 * 60)).await;
                    continue;
                };
                log::debug!("screener refreshing {} (tier {})", symbol.symbol, symbol.tier);
                match refresh_one(&pool, &symbol.symbol).await {
                    Ok(()) => {
                        let _ = SymbolRow::mark_refreshed(&pool, &symbol.symbol, "ok", None).await;
                    }
                    Err(e) => {
                        log::warn!("screener refresh {}: {}", symbol.symbol, e);
                        let _ = SymbolRow::mark_refreshed(&pool, &symbol.symbol, "error", Some(&e.to_string())).await;
                    }
                }
                burst = burst.saturating_add(1);
                if cfg.burst_pause_every_n > 0 && burst >= cfg.burst_pause_every_n {
                    burst = 0;
                    sleep_or_kick(&cancel, &kick, Duration::from_millis(cfg.burst_pause_ms)).await;
                } else {
                    sleep_or_kick(&cancel, &kick, Duration::from_millis(cfg.pacing_ms)).await;
                }
            }
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            log::info!("screener refresh stopped");
        })
    }

    pub async fn status(&self, backfill_running: bool) -> Result<SchedulerStatus> {
        Ok(SchedulerStatus {
            running: self.running.load(std::sync::atomic::Ordering::Relaxed),
            backfill_running,
            universe_size: count_symbols(&self.pool).await?,
            pending_count: count_pending(&self.pool, self.config.tier0_interval_secs, self.config.tier1_interval_secs).await?,
            last_universe_sync_at: universe::last_sync_at(&self.pool).await.unwrap_or(0),
        })
    }

    /// Wake the scheduler immediately (e.g. after a manual symbol push).
    pub fn kick(&self) {
        self.notify_kick.notify_waiters();
    }

    /// Stop the background refresh loop (in-flight symbol may still finish).
    pub fn stop(&self) {
        self.cancel.lock().unwrap().cancel();
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Result of a foreground backfill run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackfillStats {
    pub refreshed: u64,
    pub errors: u64,
}

/// Reset refresh timestamps and refresh every symbol once (foreground).
pub async fn reset_refresh_jobs(pool: &SqlitePool) -> Result<u64> {
    let r = sqlx::query(
        "UPDATE symbols SET last_refreshed_at = NULL, last_refresh_status = NULL, last_refresh_error = NULL",
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// Refresh all symbols until none are pending. Does not start the background scheduler.
pub async fn backfill_all(pool: &SqlitePool, cfg: &RefreshConfig) -> Result<BackfillStats> {
    let mut refreshed = 0u64;
    let mut errors = 0u64;
    let mut burst = 0u32;
    loop {
        let Some(symbol) =
            next_due_symbol(pool, cfg.tier0_interval_secs, cfg.tier1_interval_secs).await?
        else {
            break;
        };
        log::info!("backfill refreshing {}", symbol.symbol);
        match refresh_one(pool, &symbol.symbol).await {
            Ok(()) => {
                let _ = SymbolRow::mark_refreshed(pool, &symbol.symbol, "ok", None).await;
                refreshed += 1;
            }
            Err(e) => {
                log::warn!("backfill {}: {}", symbol.symbol, e);
                let _ =
                    SymbolRow::mark_refreshed(pool, &symbol.symbol, "error", Some(&e.to_string()))
                        .await;
                errors += 1;
            }
        }
        burst = burst.saturating_add(1);
        if cfg.burst_pause_every_n > 0 && burst >= cfg.burst_pause_every_n {
            burst = 0;
            sleep(Duration::from_millis(cfg.burst_pause_ms)).await;
        } else {
            sleep(Duration::from_millis(cfg.pacing_ms)).await;
        }
    }
    Ok(BackfillStats { refreshed, errors })
}

async fn sleep_or_kick(cancel: &CancellationToken, kick: &Notify, dur: Duration) {
    tokio::select! {
        _ = sleep(dur) => {}
        _ = cancel.cancelled() => {}
        _ = kick.notified() => {}
    }
}

async fn sync_universe_if_due(pool: &SqlitePool, cfg: &RefreshConfig) -> Result<()> {
    if !universe::auto_sync_enabled() {
        return Ok(());
    }
    let last = universe::last_sync_at(pool).await.unwrap_or(0);
    let now = Utc::now().timestamp();
    if now - last < cfg.universe_sync_interval_secs {
        return Ok(());
    }
    let discovered = universe::discover_universe_all(None, None).await;
    let n = universe::sync_universe(pool, &discovered).await?;
    universe::record_sync(pool, now).await?;
    log::info!("screener universe synced; {} symbols upserted", n);
    Ok(())
}

/// Deep enrich one symbol and upsert its snapshot.
pub async fn refresh_one(pool: &SqlitePool, symbol: &str) -> Result<()> {
    let (mut financials, statements, chart, profile) = tokio::join!(
        fetch_financials(symbol),
        fetch_statement_bundle(symbol),
        fetch_chart_history_10y(symbol),
        fetch_asset_profile(symbol),
    );
    if let Some(fv) = SymbolRow::face_value_for(pool, symbol).await? {
        financials.face_value = fv;
    }
    let inputs = ComputeInputs {
        financials: &financials,
        statements: &statements,
        chart_10y: &chart,
        peer_quote: None,
        asset_profile: &profile,
    };
    let mut metrics = compute_all(&inputs);
    let has_price = metrics
        .get(&MetricId::CurrentPrice)
        .and_then(|v| *v)
        .filter(|p| p.is_finite() && *p > 0.0)
        .is_some();
    if !has_price {
        let live = fetch_price(symbol).await;
        if live > 0.0 {
            metrics.insert(MetricId::CurrentPrice, Some(live));
        }
    }

    // Ensure FK parent row exists before writing the snapshot.
    SymbolRow {
        symbol: symbol.to_string(),
        tier: tier_for_symbol(symbol),
        ..Default::default()
    }
    .upsert_identity(pool)
    .await?;

    let snap = StockSnapshot::new(symbol.to_string(), metrics);
    snap.upsert(pool).await?;

    // Identity from Yahoo profile (exchange is already NSE/BSE — see fetch_asset_profile).
    SymbolRow {
        symbol: symbol.to_string(),
        short_name: profile.long_name.clone(),
        sector: profile.sector.clone(),
        industry: profile.industry.clone(),
        exchange: profile.exchange.clone(),
        currency: profile.currency.clone(),
        country: profile.country.clone(),
        tier: tier_for_symbol(symbol),
        ..Default::default()
    }
    .upsert_identity(pool)
    .await?;

    Ok(())
}

fn tier_for_symbol(symbol: &str) -> i64 {
    if universe::nifty500_seed().contains(symbol) { 0 } else { 1 }
}
