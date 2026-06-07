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

/// Default DB path.
///
/// Resolution order:
/// 1. `STOCKER_DB_PATH` environment variable
/// 2. Existing `stocker.db` in the current directory, beside the executable, or a parent (up to 6 levels)
/// 3. `./stocker.db` in the current directory (created on first open if missing)
pub fn default_db_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("STOCKER_DB_PATH") {
        return PathBuf::from(env_path);
    }
    if let Some(found) = find_existing_db_path() {
        return found;
    }
    PathBuf::from(DB_FILE_NAME)
}

/// Walk `start` and up to `max_up` parents looking for `stocker.db`.
fn find_in_ancestors(start: &Path, max_up: usize) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..=max_up {
        let candidate = dir.join(DB_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn find_existing_db_path() -> Option<PathBuf> {
    if let Ok(cwd) = std::env::current_dir() {
        let in_cwd = cwd.join(DB_FILE_NAME);
        if in_cwd.is_file() {
            return Some(in_cwd);
        }
        if let Some(found) = find_in_ancestors(&cwd, 6) {
            return Some(found);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let beside_exe = exe_dir.join(DB_FILE_NAME);
            if beside_exe.is_file() {
                return Some(beside_exe);
            }
            if let Some(found) = find_in_ancestors(exe_dir, 6) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn find_in_ancestors_locates_parent_db() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let nested = repo.join("target").join("release");
        fs::create_dir_all(&nested).unwrap();
        let db = repo.join(DB_FILE_NAME);
        fs::write(&db, b"").unwrap();
        assert_eq!(find_in_ancestors(&nested, 6), Some(db));
    }
}
