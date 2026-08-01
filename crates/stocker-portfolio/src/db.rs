//! SQLite pool + migrations for portfolio.db.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::Migration;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const DB_FILE_NAME: &str = "portfolio.db";

/// Open (and migrate) the portfolio SQLite database at `path`.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("create db parent dir: {e}")))?;
        }
    }

    let opts = SqliteConnectOptions::new()
        .filename(path)
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
    Ok(pool)
}

/// Open an existing portfolio database read-only without running migrations.
pub async fn open_existing_readonly(path: &Path) -> Result<SqlitePool> {
    if !path.is_file() {
        return Err(Error::Other(format!(
            "portfolio database not found at {}",
            path.display()
        )));
    }

    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));

    SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .map_err(Error::from)
}

/// Open an in-memory DB for tests.
pub async fn open_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| Error::Other(format!("invalid sqlite url: {e}")))?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    repair_applied_migrations(&pool).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

struct AppliedMigration {
    checksum: Vec<u8>,
}

fn migration_file(version: i64) -> Option<&'static Migration> {
    MIGRATOR.migrations.iter().find(|m| m.version == version)
}

/// Fix `_sqlx_migrations` drift before sqlx runs (orphan rows, renumbered migrations).
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
    repair_swapped_valuation_related_migrations(pool).await?;
    repair_checksums_when_schema_applied(pool).await?;
    Ok(())
}

/// Drop `_sqlx_migrations` rows for SQL files that no longer exist.
async fn prune_orphan_applied_migrations(pool: &SqlitePool) -> Result<()> {
    let known: HashSet<i64> = MIGRATOR.migrations.iter().map(|m| m.version).collect();
    let rows = sqlx::query("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await?;
    for row in rows {
        let version: i64 = row.try_get("version")?;
        if !known.contains(&version) {
            log::warn!(
                "pruning orphan portfolio migration record version {version} (SQL file no longer present)"
            );
            sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
                .bind(version)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Versions 2 and 4 were briefly swapped during development; repair when schema matches.
async fn repair_swapped_valuation_related_migrations(pool: &SqlitePool) -> Result<()> {
    let Some(file_v2) = migration_file(2) else {
        return Ok(());
    };
    let Some(file_v4) = migration_file(4) else {
        return Ok(());
    };

    let Some(rec_v2) = applied_migration(pool, 2).await? else {
        return Ok(());
    };
    let Some(rec_v4) = applied_migration(pool, 4).await? else {
        return Ok(());
    };

    if rec_v2.checksum == *file_v2.checksum.as_ref() && rec_v4.checksum == *file_v4.checksum.as_ref() {
        return Ok(());
    }
    if rec_v2.checksum != *file_v4.checksum.as_ref() || rec_v4.checksum != *file_v2.checksum.as_ref() {
        return Ok(());
    }
    if !schema_has_valuation_snapshot_columns(pool).await?
        || !schema_has_related_symbol_column(pool).await?
    {
        return Ok(());
    }

    log::warn!(
        "repairing swapped portfolio migration records for versions 2 (valuation snapshot) and 4 (related symbol)"
    );
    update_migration_record(pool, file_v2).await?;
    update_migration_record(pool, file_v4).await?;
    Ok(())
}

/// When a migration's SQL changed after it ran but the schema already matches, refresh checksums.
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
            "repairing portfolio migration record for version {} ({})",
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

async fn table_has_column(pool: &SqlitePool, table: &str, column: &str) -> Result<bool> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .any(|name| name == column))
}

async fn schema_has_valuation_snapshot_columns(pool: &SqlitePool) -> Result<bool> {
    Ok(table_has_column(pool, "portfolio_snapshots", "holdings_json").await?
        && table_has_column(pool, "portfolio_snapshots", "valuation_summary_json").await?
        && table_has_column(pool, "portfolio_snapshots", "priced_at").await?
        && table_has_column(pool, "portfolio_snapshots", "symbol_prices_json").await?)
}

async fn schema_has_related_symbol_column(pool: &SqlitePool) -> Result<bool> {
    table_has_column(pool, "transactions", "related_symbol").await
}

async fn migration_schema_applied(pool: &SqlitePool, version: i64) -> Result<bool> {
    match version {
        1 => Ok(true),
        2 => schema_has_valuation_snapshot_columns(pool).await,
        3 => {
            let exists: Option<i32> = sqlx::query_scalar(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mf_schedules'",
            )
            .fetch_optional(pool)
            .await?;
            Ok(exists.is_some() && table_has_column(pool, "transactions", "schedule_id").await?)
        }
        4 => schema_has_related_symbol_column(pool).await,
        _ => Ok(false),
    }
}

/// Default portfolio DB path (`STOCKER_PORTFOLIO_DB_PATH`, then `%APPDATA%/stocker/portfolio.db`, then legacy search).
pub fn default_db_path() -> PathBuf {
    stocker_core::paths::resolve_data_file_path("STOCKER_PORTFOLIO_DB_PATH", DB_FILE_NAME)
}
