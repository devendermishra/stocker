use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use env_logger::Env;
use stocker_screener::{db::default_db_path, RefreshConfig, ScreenQuery, ScreenerService};

/// NSE stock research CLI (Yahoo Finance data + screener)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Fetch and print a research report for one symbol (JSON to stdout)
    Report {
        /// Stock symbol (e.g. RELIANCE, RELIANCE.NS, or SOMEBSE.BO)
        symbol: String,
    },
    /// Run a screener query against the local SQLite database
    Screener {
        /// Path to a JSON file containing a [`ScreenQuery`] (filters, optional sort, limit)
        #[arg(short, long)]
        query: PathBuf,
        /// SQLite database path (default: STOCKER_DB_PATH or nearest stocker.db)
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Refresh one symbol into the screener database
    Refresh {
        symbol: String,
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Load universe symbols from local CSV files into the `symbols` table (no exchange network calls)
    Universe {
        /// NSE universe CSV (or set STOCKER_UNIVERSE_CSV). EQUITY_L format or a `symbol` column.
        #[arg(short, long)]
        csv: Option<PathBuf>,
        /// BSE universe CSV (or set STOCKER_BSE_UNIVERSE_CSV). List of Securities export.
        #[arg(long)]
        bse_csv: Option<PathBuf>,
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Print screener DB + scheduler status (symbol counts, refresh progress)
    Status {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Clear refresh timestamps so the background scheduler re-queues every symbol
    ResetJobs {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Stop the in-process refresh scheduler (no-op if not running)
    StopScheduler {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Stop scheduler, reset jobs, and refresh every symbol (foreground backfill)
    Backfill {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Backfill missing sector/industry labels (lightweight Yahoo profile fetch)
    BackfillSectors {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Print per-metric fill rates (full / partial / empty) across snapshots
    Coverage {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Recompute composite metrics from parent columns already in snapshots
    RecomputeComposites {
        #[arg(short, long)]
        db: Option<PathBuf>,
    },
    /// Sync portfolio.db and stocker.db with Google Drive
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
}

#[derive(Subcommand, Debug)]
enum SyncCommand {
    /// Sign in to Google Drive (OAuth desktop flow)
    Auth,
    /// Show local vs remote sync status
    Status,
    /// Upload local databases to Google Drive
    Push {
        #[arg(long)]
        force: bool,
    },
    /// Download and restore databases from Google Drive
    Pull {
        #[arg(long)]
        force: bool,
    },
    /// Smart sync: pull if remote is newer, else push if local changed
    Run {
        #[arg(long)]
        force: bool,
    },
    /// List portfolios in the Google Drive backup vs local (JSON)
    Browse,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    stocker_core::paths::pin_database_paths();

    match Cli::parse().command {
        Command::Report { symbol } => {
            match stocker_core::build_research_report(&symbol, None, None).await {
                Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Command::Screener { query, db } => {
            if let Err(code) = run_screener(&query, db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Refresh { symbol, db } => {
            if let Err(code) = run_refresh(&symbol, db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Universe { csv, bse_csv, db } => {
            if let Err(code) = run_universe(csv.as_deref(), bse_csv.as_deref(), db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Status { db } => {
            if let Err(code) = run_status(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::ResetJobs { db } => {
            if let Err(code) = run_reset_jobs(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::StopScheduler { db } => {
            if let Err(code) = run_stop_scheduler(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Backfill { db } => {
            if let Err(code) = run_backfill(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::BackfillSectors { db } => {
            if let Err(code) = run_backfill_sectors(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Coverage { db } => {
            if let Err(code) = run_coverage(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::RecomputeComposites { db } => {
            if let Err(code) = run_recompute_composites(db.as_deref()).await {
                std::process::exit(code);
            }
        }
        Command::Sync { command } => {
            if let Err(code) = run_sync(command).await {
                std::process::exit(code);
            }
        }
    }
}

async fn open_service(db: Option<&Path>) -> Result<ScreenerService, i32> {
    let path = db.map(Path::to_path_buf).unwrap_or_else(default_db_path);
    ScreenerService::open(&path, RefreshConfig::from_env())
        .await
        .map_err(|e| {
            eprintln!("Error opening DB at {}: {}", path.display(), e);
            1
        })
}

async fn run_screener(query_path: &Path, db: Option<&Path>) -> Result<(), i32> {
    let text = std::fs::read_to_string(query_path).map_err(|e| {
        eprintln!("Error reading query file: {}", e);
        1
    })?;
    let query: ScreenQuery = serde_json::from_str(&text).map_err(|e| {
        eprintln!("Error parsing query JSON: {}", e);
        1
    })?;
    let svc = open_service(db).await?;
    let rows = svc.run_query(&query).await.map_err(|e| {
        eprintln!("Screener query failed: {}", e);
        1
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "rows": rows })).unwrap()
    );
    Ok(())
}

async fn run_coverage(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let report = svc.coverage().await.map_err(|e| {
        eprintln!("{e}");
        1
    })?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}

async fn run_recompute_composites(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let stats = svc.recompute_composites().await.map_err(|e| {
        eprintln!("{e}");
        1
    })?;
    eprintln!(
        "Recomputed composite columns ({} row-updates across all composite SQL statements).",
        stats.rows_touched
    );
    Ok(())
}

async fn run_reset_jobs(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let n = svc.reset_refresh_jobs().await.map_err(|e| {
        eprintln!("{e}");
        1
    })?;
    eprintln!("Reset {n} symbols — background scheduler will refresh them on the next pass.");
    Ok(())
}

async fn run_stop_scheduler(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    svc.stop_scheduler();
    eprintln!("Refresh scheduler stop signalled (safe if nothing was running).");
    Ok(())
}

async fn run_backfill(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let cfg = RefreshConfig::from_env();
    eprintln!(
        "Backfill starting (pacing {}ms, ~{} symbols)…",
        cfg.pacing_ms,
        stocker_screener::snapshot::count_symbols(svc.pool())
            .await
            .unwrap_or(0)
    );
    let stats = svc.backfill(&cfg).await.map_err(|e| {
        eprintln!("Backfill failed: {e}");
        1
    })?;
    eprintln!(
        "Backfill done: {} refreshed, {} errors.",
        stats.refreshed, stats.errors
    );
    Ok(())
}

async fn run_backfill_sectors(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let cfg = RefreshConfig::from_env();
    let missing = stocker_screener::refresh::symbols_missing_sector(svc.pool())
        .await
        .map(|v| v.len())
        .unwrap_or(0);
    eprintln!(
        "Sector backfill starting (pacing {}ms, {} symbols missing a sector)…",
        cfg.pacing_ms, missing
    );
    let stats = svc.backfill_sectors(&cfg).await.map_err(|e| {
        eprintln!("Sector backfill failed: {e}");
        1
    })?;
    eprintln!(
        "Sector backfill done: {} scanned, {} filled, {} still missing, {} errors.",
        stats.scanned, stats.filled, stats.still_missing, stats.errors
    );
    Ok(())
}

async fn run_status(db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let pool = svc.pool();
    let path = db.map(Path::to_path_buf).unwrap_or_else(default_db_path);

    let total = stocker_screener::snapshot::count_symbols(pool)
        .await
        .map_err(|_| 1)?;
    let pending = svc
        .status()
        .await
        .map_err(|_| 1)?;
    let with_snapshots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(pool)
        .await
        .map_err(|e| {
            eprintln!("{e}");
            1
        })?;
    let never_refreshed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM symbols WHERE last_refreshed_at IS NULL",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| 1)?;
    let refresh_ok: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM symbols WHERE last_refresh_status = 'ok'",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| 1)?;
    let refresh_error: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM symbols WHERE last_refresh_status = 'error'",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| 1)?;
    let tier0: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM symbols WHERE tier = 0")
        .fetch_one(pool)
        .await
        .map_err(|_| 1)?;

    let out = serde_json::json!({
        "db_path": path.display().to_string(),
        "scheduler_running": pending.running,
        "universe_size": total,
        "symbols_with_snapshots": with_snapshots,
        "never_refreshed": never_refreshed,
        "last_refresh_ok": refresh_ok,
        "last_refresh_error": refresh_error,
        "currently_due_for_refresh": pending.pending_count,
        "tier0_nifty500": tier0,
        "tier1_rest": total - tier0,
        "last_universe_sync_at": pending.last_universe_sync_at,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    Ok(())
}

async fn run_universe(
    csv: Option<&Path>,
    bse_csv: Option<&Path>,
    db: Option<&Path>,
) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let path = db.map(Path::to_path_buf).unwrap_or_else(default_db_path);
    eprintln!(
        "Loading screener universe from local CSV (no exchange scraping; stock data via Yahoo)…"
    );
    if csv.is_none()
        && bse_csv.is_none()
        && stocker_screener::universe_csv::universe_csv_path().is_none()
        && stocker_screener::universe_csv::bse_universe_csv_path().is_none()
    {
        eprintln!(
            "Hint: pass --csv <nse-file> [--bse-csv <bse-file>] or set {} / {}.",
            stocker_screener::universe_csv::ENV_UNIVERSE_CSV,
            stocker_screener::universe_csv::ENV_BSE_UNIVERSE_CSV,
        );
    }
    let discovered = stocker_screener::universe::discover_universe_all(csv, bse_csv).await;
    eprintln!("Found {} symbols; writing to {}…", discovered.len(), path.display());
    let n = stocker_screener::universe::sync_universe(svc.pool(), &discovered)
        .await
        .map_err(|e| {
            eprintln!("Universe sync failed: {}", e);
            1
        })?;
    let now = chrono::Utc::now().timestamp();
    stocker_screener::universe::record_sync(svc.pool(), now)
        .await
        .map_err(|e| {
            eprintln!("Could not record sync time: {}", e);
            1
        })?;
    let total = stocker_screener::snapshot::count_symbols(svc.pool())
        .await
        .map_err(|_| 1)?;
    eprintln!("Universe sync complete: {} symbols upserted this run, {} total in DB.", n, total);
    Ok(())
}

async fn run_refresh(symbol: &str, db: Option<&Path>) -> Result<(), i32> {
    let svc = open_service(db).await?;
    let symbol = svc.resolve_symbol(symbol).await.map_err(|e| {
        eprintln!("Invalid symbol: {}", e);
        1
    })?;
    svc.refresh_now(&symbol).await.map_err(|e| {
        eprintln!("Refresh failed: {}", e);
        1
    })?;
    eprintln!("Refreshed {}", symbol);
    Ok(())
}

async fn run_sync(command: SyncCommand) -> Result<(), i32> {
    use stocker_sync::{SyncAction, auth, load_local_portfolio_refs, pull, push, remote_browse_index, status, sync};

    let map_err = |e: stocker_sync::Error| {
        eprintln!("Sync error: {e}");
        1
    };

    match command {
        SyncCommand::Auth => auth().await.map_err(map_err),
        SyncCommand::Status => {
            let st = status().await.map_err(map_err)?;
            println!("{}", serde_json::to_string_pretty(&st).unwrap());
            Ok(())
        }
        SyncCommand::Push { force } => {
            match push(force).await.map_err(map_err)? {
                SyncAction::Pushed => eprintln!("Pushed backup to Google Drive."),
                SyncAction::AlreadyInSync => eprintln!("Already in sync — nothing to push."),
                SyncAction::Pulled => {}
            }
            Ok(())
        }
        SyncCommand::Pull { force } => {
            match pull(force).await.map_err(map_err)? {
                SyncAction::Pulled => eprintln!("Restored databases from Google Drive."),
                SyncAction::AlreadyInSync => eprintln!("Already in sync — nothing to pull."),
                SyncAction::Pushed => {}
            }
            Ok(())
        }
        SyncCommand::Run { force } => {
            match sync(force).await.map_err(map_err)? {
                SyncAction::Pulled => eprintln!("Pulled newer backup from Google Drive."),
                SyncAction::Pushed => eprintln!("Pushed local backup to Google Drive."),
                SyncAction::AlreadyInSync => eprintln!("Already in sync."),
            }
            Ok(())
        }
        SyncCommand::Browse => {
            let local = load_local_portfolio_refs().await.map_err(map_err)?;
            let idx = remote_browse_index(false, local).await.map_err(map_err)?;
            println!("{}", serde_json::to_string_pretty(&idx).unwrap());
            Ok(())
        }
    }
}
