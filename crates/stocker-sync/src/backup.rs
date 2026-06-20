use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::config::{
    MANIFEST_FILENAME, PORTFOLIO_DB_NAME, SCREENER_DB_NAME, portfolio_db_path, screener_db_path,
};
use crate::error::{Error, Result};
use crate::manifest::{SyncManifest, file_entry, sha256_hex};

/// Create a consistent SQLite snapshot using WAL checkpoint + VACUUM INTO.
pub async fn snapshot_db(source: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let url = format!("sqlite://{}", source.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| Error::Other(format!("invalid sqlite url: {e}")))?
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    sqlx::query("PRAGMA wal_checkpoint(FULL)")
        .execute(&pool)
        .await?;

    let dest_str = dest.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{dest_str}'"))
        .execute(&pool)
        .await?;

    pool.close().await;
    Ok(())
}

pub async fn create_archive(device_id: Uuid, dest_zip: &Path) -> Result<SyncManifest> {
    let temp_dir = tempfile::tempdir()?;
    let portfolio_snap = temp_dir.path().join(PORTFOLIO_DB_NAME);
    let screener_snap = temp_dir.path().join(SCREENER_DB_NAME);

    let portfolio_src = portfolio_db_path();
    let screener_src = screener_db_path();

    if portfolio_src.is_file() {
        snapshot_db(&portfolio_src, &portfolio_snap).await?;
    }
    if screener_src.is_file() {
        snapshot_db(&screener_src, &screener_snap).await?;
    }

    let mut files = BTreeMap::new();
    if portfolio_snap.is_file() {
        files.insert(
            PORTFOLIO_DB_NAME.to_string(),
            file_entry(&portfolio_snap)?,
        );
    }
    if screener_snap.is_file() {
        files.insert(
            SCREENER_DB_NAME.to_string(),
            file_entry(&screener_snap)?,
        );
    }

    if files.is_empty() {
        return Err(Error::Other(
            "no database files found to back up".to_string(),
        ));
    }

    let manifest = SyncManifest::new(device_id, files);
    let manifest_json = manifest.to_json()?;

    if dest_zip.exists() {
        std::fs::remove_file(dest_zip)?;
    }
    if let Some(parent) = dest_zip.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let file = std::fs::File::create(dest_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(MANIFEST_FILENAME, options)?;
    zip.write_all(manifest_json.as_bytes())?;

    for name in [PORTFOLIO_DB_NAME, SCREENER_DB_NAME] {
        let snap_path = temp_dir.path().join(name);
        if !snap_path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&snap_path)?;
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
    }

    zip.finish()?;
    Ok(manifest)
}

pub fn read_manifest_from_zip(zip_path: &Path) -> Result<SyncManifest> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut manifest_file = archive.by_name(MANIFEST_FILENAME).map_err(|_| {
        Error::Other(format!("missing {MANIFEST_FILENAME} in backup archive"))
    })?;
    let mut text = String::new();
    manifest_file.read_to_string(&mut text)?;
    SyncManifest::from_json(&text)
}

pub fn restore_archive(zip_path: &Path) -> Result<SyncManifest> {
    let manifest = read_manifest_from_zip(zip_path)?;

    let file = std::fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for (name, entry) in &manifest.files {
        let mut zipped = archive.by_name(name).map_err(|_| {
            Error::Other(format!("missing {name} in backup archive"))
        })?;
        let mut bytes = Vec::new();
        zipped.read_to_end(&mut bytes)?;
        let actual = sha256_hex(&bytes);
        if actual != entry.sha256 {
            return Err(Error::ChecksumMismatch(name.clone()));
        }

        let dest = match name.as_str() {
            PORTFOLIO_DB_NAME => portfolio_db_path(),
            SCREENER_DB_NAME => screener_db_path(),
            other => {
                return Err(Error::Other(format!("unexpected file in archive: {other}")));
            }
        };

        restore_one_db(&dest, &bytes)?;
    }

    Ok(manifest)
}

fn restore_one_db(dest: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let backup = dest.with_extension("db.bak");
    if dest.is_file() {
        std::fs::rename(dest, &backup)?;
    }

    let temp = dest.with_extension("db.new");
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, dest)?;

    for sidecar in [
        PathBuf::from(format!("{}-wal", dest.display())),
        PathBuf::from(format!("{}-shm", dest.display())),
    ] {
        let _ = std::fs::remove_file(sidecar);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    async fn init_test_db(path: &Path) {
        let url = format!("sqlite://{}", path.display());
        let opts = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t (v) VALUES ('hello')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn archive_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let portfolio = dir.path().join("portfolio.db");
        let screener = dir.path().join("stocker.db");
        init_test_db(&portfolio).await;
        init_test_db(&screener).await;

        std::env::set_var("STOCKER_PORTFOLIO_DB_PATH", portfolio.to_str().unwrap());
        std::env::set_var("STOCKER_DB_PATH", screener.to_str().unwrap());

        let zip_path = dir.path().join("backup.zip");
        let manifest = create_archive(Uuid::new_v4(), &zip_path).await.unwrap();
        assert_eq!(manifest.files.len(), 2);

        sqlx::query("DELETE FROM t")
            .execute(
                &SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(
                        SqliteConnectOptions::from_str(&format!("sqlite://{}", portfolio.display()))
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();

        restore_archive(&zip_path).unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(
                &SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(
                        SqliteConnectOptions::from_str(&format!("sqlite://{}", portfolio.display()))
                            .unwrap(),
                    )
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
