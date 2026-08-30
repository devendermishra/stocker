//! SQLite pool + migrations + schema validation against the catalog.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::Migration;
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

    repair_applied_migrations(&pool).await?;
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
    repair_applied_migrations(&pool).await?;
    MIGRATOR.run(&pool).await?;
    validate_catalog().map_err(Error::Other)?;
    validate_schema(&pool).await?;
    Ok(pool)
}

struct AppliedMigration {
    checksum: Vec<u8>,
}

/// Fix `_sqlx_migrations` drift before sqlx runs (orphan rows, edited SQL checksums,
/// or DDL that landed without a migration row — common after interrupted ALTER chains).
async fn repair_applied_migrations(pool: &SqlitePool) -> Result<()> {
    let exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if exists.is_none() {
        return Ok(());
    }

    prune_orphan_applied_migrations(pool).await?;
    backfill_missing_migration_records(pool).await?;
    repair_checksums_when_schema_applied(pool).await?;
    Ok(())
}

/// Drop `_sqlx_migrations` rows for SQL files that no longer exist (e.g. one-off
/// repair migrations removed after they ran on existing databases).
async fn prune_orphan_applied_migrations(pool: &SqlitePool) -> Result<()> {
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

/// If DDL from a migration is already on the DB but `_sqlx_migrations` has no row
/// (e.g. SQLite auto-committed ALTER then migration failed), insert the record so
/// sqlx does not try to re-run it.
async fn backfill_missing_migration_records(pool: &SqlitePool) -> Result<()> {
    for file in MIGRATOR.migrations.iter() {
        if applied_migration(pool, file.version).await?.is_some() {
            continue;
        }
        if !migration_schema_applied(pool, file.version).await? {
            continue;
        }
        log::warn!(
            "backfilling screener migration record for version {} ({}) — schema already present",
            file.version,
            file.description
        );
        insert_migration_record(pool, file).await?;
    }
    Ok(())
}

/// When a migration's SQL changed after it ran but the schema already matches, refresh checksums.
/// Avoids sqlx's "migration N was previously applied but has been modified" after local SQL
/// content/CRLF edits that do not need to re-run DDL.
async fn repair_checksums_when_schema_applied(pool: &SqlitePool) -> Result<()> {
    for file in MIGRATOR.migrations.iter() {
        let Some(rec) = applied_migration(pool, file.version).await? else {
            continue;
        };
        if rec.checksum == *file.checksum.as_ref() {
            continue;
        }
        if !migration_schema_applied(pool, file.version).await? {
            continue;
        }
        log::warn!(
            "repairing screener migration checksum for version {} ({})",
            file.version,
            file.description
        );
        update_migration_record(pool, file).await?;
    }
    Ok(())
}

async fn applied_migration(pool: &SqlitePool, version: i64) -> Result<Option<AppliedMigration>> {
    let row = sqlx::query("SELECT version, checksum FROM _sqlx_migrations WHERE version = ?")
        .bind(version)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|row| AppliedMigration {
        checksum: row.try_get("checksum").unwrap_or_default(),
    }))
}

async fn update_migration_record(pool: &SqlitePool, file: &Migration) -> Result<()> {
    sqlx::query("UPDATE _sqlx_migrations SET description = ?, checksum = ? WHERE version = ?")
        .bind(&file.description)
        .bind(file.checksum.as_ref())
        .bind(file.version)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_migration_record(pool: &SqlitePool, file: &Migration) -> Result<()> {
    // Match sqlx's `_sqlx_migrations` shape (version, description, installed_on, success, checksum, execution_time).
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
         VALUES (?, ?, CURRENT_TIMESTAMP, 1, ?, 0)",
    )
    .bind(file.version)
    .bind(&file.description)
    .bind(file.checksum.as_ref())
    .execute(pool)
    .await?;
    Ok(())
}

async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool> {
    let exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(table)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn table_has_column(pool: &SqlitePool, table: &str, column: &str) -> Result<bool> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .any(|name| name == column))
}

async fn migration_schema_applied(pool: &SqlitePool, version: i64) -> Result<bool> {
    match version {
        1 => Ok(table_exists(pool, "symbols").await? && table_exists(pool, "snapshots").await?),
        2 => table_has_column(pool, "symbols", "face_value").await,
        3 => Ok(table_has_column(pool, "snapshots", "cumulative_cfo_pat_3y").await?
            && table_has_column(pool, "snapshots", "days_inventory_change_3y").await?),
        4 => table_has_column(pool, "symbols", "isin").await,
        5 => Ok(table_has_column(pool, "snapshots", "moat_score").await?
            && table_has_column(pool, "snapshots", "business_tier").await?),
        _ => Ok(false),
    }
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
