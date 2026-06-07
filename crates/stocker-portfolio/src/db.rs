//! SQLite pool + migrations for portfolio.db.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

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

/// Default portfolio DB path.
pub fn default_db_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("STOCKER_PORTFOLIO_DB_PATH") {
        return PathBuf::from(env_path);
    }
    if let Some(found) = find_existing_db_path() {
        return found;
    }
    PathBuf::from(DB_FILE_NAME)
}

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
