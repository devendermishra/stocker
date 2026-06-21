//! SQLite pool + migrations for mf.db.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

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
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Default MF DB path (`STOCKER_MF_DB_PATH`, then `%APPDATA%/stocker/mf.db`, then legacy search).
pub fn default_db_path() -> PathBuf {
    stocker_core::paths::resolve_data_file_path("STOCKER_MF_DB_PATH", DB_FILE_NAME)
}
