//! SQLite pool + migrations + schema validation against the catalog.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::metrics::{validate_catalog, MetricId};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Open (and migrate) the SQLite database at `path`. Creates parent dirs and the
/// file itself if missing. Validates that the schema matches the metric catalog.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("create db parent dir: {e}")))?;
        }
    }

    let url = format!("sqlite://{}", path.display());
    // Build options imperatively so we can also accept "sqlite::memory:"-style URLs.
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| Error::Other(format!("invalid sqlite url: {e}")))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;

    prune_orphan_applied_migrations(&pool).await?;
    MIGRATOR.run(&pool).await?;

    validate_catalog().map_err(Error::Other)?;
    validate_schema(&pool).await?;

    Ok(pool)
}

/// Connect to an existing SQLite file without running migrations (maintenance only).
pub async fn connect_without_migrate(path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite://{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| Error::Other(format!("invalid sqlite url: {e}")))?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(Into::into)
}

/// Open an in-memory DB. Useful for tests.
#[allow(dead_code)]
pub async fn open_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| Error::Other(format!("invalid sqlite url: {e}")))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    MIGRATOR.run(&pool).await?;
    validate_catalog().map_err(Error::Other)?;
    validate_schema(&pool).await?;
    Ok(pool)
}

/// Drop `_sqlx_migrations` rows for SQL files that no longer exist (e.g. one-off
/// repair migrations removed after they ran on existing databases).
async fn prune_orphan_applied_migrations(pool: &SqlitePool) -> Result<()> {
    let exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if exists.is_none() {
        return Ok(());
    }

    let known: HashSet<i64> = MIGRATOR.migrations.iter().map(|m| m.version).collect();
    let rows = sqlx::query("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await?;
    for row in rows {
        let version: i64 = row.try_get("version")?;
        if !known.contains(&version) {
            log::warn!(
                "pruning orphan migration record version {version} (SQL file no longer present)"
            );
            sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
                .bind(version)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Confirm that every catalog column exists on `snapshots`. Catches drift between
/// the migration SQL and the `metrics::CATALOG` enum.
pub async fn validate_schema(pool: &SqlitePool) -> Result<()> {
    let rows = sqlx::query("PRAGMA table_info(snapshots)")
        .fetch_all(pool)
        .await?;
    let mut cols = std::collections::HashSet::new();
    for row in rows {
        let name: String = row.try_get("name")?;
        cols.insert(name);
    }
    for id in MetricId::ALL {
        if !cols.contains(id.column()) {
            return Err(Error::Other(format!(
                "snapshots is missing column `{}` for metric {:?}",
                id.column(),
                id
            )));
        }
    }
    Ok(())
}

const DB_FILE_NAME: &str = "stocker.db";

/// Default DB path (`STOCKER_DB_PATH`, then `%APPDATA%/stocker/stocker.db`, then legacy search).
pub fn default_db_path() -> PathBuf {
    stocker_core::paths::resolve_data_file_path("STOCKER_DB_PATH", DB_FILE_NAME)
}
