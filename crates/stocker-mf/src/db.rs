//! SQLite pool + migrations for mf.db.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::migrate::Migration;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const DB_FILE_NAME: &str = "mf.db";

/// Open (and migrate) the mutual fund SQLite database at `path`.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("create db parent dir: {e}")))?;
        }
    }

    let url = format!("sqlite://{}", path.display());
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
    Ok(pool)
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
    repair_checksums_when_schema_applied(pool).await?;
    Ok(())
}

async fn prune_orphan_applied_migrations(pool: &SqlitePool) -> Result<()> {
    let known: HashSet<i64> = MIGRATOR.migrations.iter().map(|m| m.version).collect();
    let rows = sqlx::query("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await?;
    for row in rows {
        let version: i64 = row.try_get("version")?;
        if !known.contains(&version) {
            log::warn!(
                "pruning orphan mf migration record version {version} (SQL file no longer present)"
            );
            sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
                .bind(version)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

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
            "repairing mf migration checksum for version {} ({})",
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

async fn migration_schema_applied(pool: &SqlitePool, version: i64) -> Result<bool> {
    match version {
        1 => {
            let exists: Option<i32> = sqlx::query_scalar(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mf_schemes'",
            )
            .fetch_optional(pool)
            .await?;
            Ok(exists.is_some())
        }
        _ => Ok(false),
    }
}

/// Default MF DB path (`STOCKER_MF_DB_PATH`, then `%APPDATA%/stocker/mf.db`, then legacy search).
pub fn default_db_path() -> PathBuf {
    stocker_core::paths::resolve_data_file_path("STOCKER_MF_DB_PATH", DB_FILE_NAME)
}
