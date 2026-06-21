//! Platform config / data directory and SQLite file resolution.

use std::path::{Path, PathBuf};

/// App config directory (`~/.config/stocker`, `%APPDATA%/stocker`, or `STOCKER_CONFIG_DIR`).
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("STOCKER_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("stocker");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("stocker");
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("stocker");
    }
    PathBuf::from(".config/stocker")
}

pub fn ensure_config_dir() -> std::io::Result<PathBuf> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Walk `start` and up to `max_up` parents looking for `filename`.
pub fn find_in_ancestors(start: &Path, filename: &str, max_up: usize) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..=max_up {
        let candidate = dir.join(filename);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Locate an existing SQLite file in cwd, parent dirs, or beside the executable.
pub fn find_existing_data_file(filename: &str) -> Option<PathBuf> {
    if let Ok(cwd) = std::env::current_dir() {
        let in_cwd = cwd.join(filename);
        if in_cwd.is_file() {
            return Some(in_cwd);
        }
        if let Some(found) = find_in_ancestors(&cwd, filename, 6) {
            return Some(found);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let beside_exe = exe_dir.join(filename);
            if beside_exe.is_file() {
                return Some(beside_exe);
            }
            if let Some(found) = find_in_ancestors(exe_dir, filename, 6) {
                return Some(found);
            }
        }
    }
    None
}

/// Every on-disk copy of a database we know about (canonical + legacy locations).
pub fn data_file_candidates(filename: &str) -> Vec<PathBuf> {
    let canonical = config_dir().join(filename);
    let mut out = Vec::new();
    if canonical.is_file() {
        out.push(canonical);
    }
    if let Some(found) = find_existing_data_file(filename) {
        if !out.iter().any(|p| p == &found) {
            out.push(found);
        }
    }
    out
}

fn file_rank(path: &Path) -> (std::time::SystemTime, u64) {
    match std::fs::metadata(path) {
        Ok(meta) => (
            meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            meta.len(),
        ),
        Err(_) => (std::time::SystemTime::UNIX_EPOCH, 0),
    }
}

/// Prefer the newest/largest on-disk copy when multiple locations exist.
pub fn best_existing_data_file(filename: &str) -> Option<PathBuf> {
    data_file_candidates(filename)
        .into_iter()
        .max_by(|a, b| {
            let (ta, sa) = file_rank(a);
            let (tb, sb) = file_rank(b);
            ta.cmp(&tb).then(sa.cmp(&sb))
        })
}

/// Resolve a database path: env override → best existing copy → canonical create path.
pub fn resolve_data_file_path(env_key: &str, filename: &str) -> PathBuf {
    if let Ok(path) = std::env::var(env_key) {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    best_existing_data_file(filename).unwrap_or_else(|| config_dir().join(filename))
}

/// Copy a SQLite database and any WAL sidecar files.
pub fn copy_sqlite_files(source: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, dest)?;
    for ext in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{ext}", source.display()));
        if side.is_file() {
            std::fs::copy(&side, PathBuf::from(format!("{}{ext}", dest.display())))?;
        }
    }
    Ok(())
}

/// Pin database env vars once per process so the UI, CLI, and Drive sync agree on file locations.
pub fn pin_database_paths() {
    let _ = ensure_config_dir();
    pin_one("STOCKER_PORTFOLIO_DB_PATH", "portfolio.db");
    pin_one("STOCKER_DB_PATH", "stocker.db");
    pin_one("STOCKER_MF_DB_PATH", "mf.db");
}

fn pin_one(env_key: &str, filename: &str) {
    if std::env::var(env_key)
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return;
    }

    let canonical = config_dir().join(filename);
    // Migrate a legacy copy only when the canonical file does not exist yet.
    // Never overwrite an existing canonical DB — it may have been restored from Drive.
    if !canonical.is_file() {
        if let Some(legacy) = find_existing_data_file(filename) {
            match copy_sqlite_files(&legacy, &canonical) {
                Ok(()) => eprintln!(
                    "Migrated {filename} from {} to {}",
                    legacy.display(),
                    canonical.display()
                ),
                Err(e) => eprintln!(
                    "Warning: could not migrate {filename} to {} ({e}); using {}",
                    canonical.display(),
                    legacy.display()
                ),
            }
        }
    }
    std::env::set_var(env_key, &canonical);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_config_dir(dir: &Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("STOCKER_CONFIG_DIR", dir);
        std::env::remove_var("STOCKER_PORTFOLIO_DB_PATH");
        f();
        std::env::remove_var("STOCKER_CONFIG_DIR");
        std::env::remove_var("STOCKER_PORTFOLIO_DB_PATH");
    }

    #[test]
    fn find_in_ancestors_locates_parent_db() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let nested = repo.join("target").join("release");
        std::fs::create_dir_all(&nested).unwrap();
        let db = repo.join("stocker.db");
        std::fs::write(&db, b"legacy-data").unwrap();
        assert_eq!(
            find_in_ancestors(&nested, "stocker.db", 6),
            Some(db)
        );
    }

    #[test]
    fn best_existing_prefers_newer_larger_copy() {
        let dir = tempfile::tempdir().unwrap();
        with_config_dir(dir.path(), || {
            let legacy = dir.path().join("legacy").join("portfolio.db");
            std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
            std::fs::write(&legacy, b"old").unwrap();

            let canonical = config_dir().join("portfolio.db");
            std::fs::write(&canonical, b"newer-and-larger-data").unwrap();

            let best = best_existing_data_file("portfolio.db").unwrap();
            assert_eq!(best, canonical);
        });
    }

    #[test]
    fn pin_leaves_existing_canonical_untouched() {
        let dir = tempfile::tempdir().unwrap();
        with_config_dir(dir.path(), || {
            let canonical = config_dir().join("portfolio.db");
            std::fs::write(&canonical, b"from-drive-restore").unwrap();
            pin_one("STOCKER_PORTFOLIO_DB_PATH", "portfolio.db");
            assert_eq!(std::fs::read(&canonical).unwrap(), b"from-drive-restore");
        });
    }

    #[test]
    fn pin_creates_canonical_path_env() {
        let dir = tempfile::tempdir().unwrap();
        with_config_dir(dir.path(), || {
            pin_one("STOCKER_PORTFOLIO_DB_PATH", "portfolio.db");
            assert_eq!(
                std::env::var("STOCKER_PORTFOLIO_DB_PATH").unwrap(),
                config_dir().join("portfolio.db").to_string_lossy().to_string()
            );
        });
    }
}
