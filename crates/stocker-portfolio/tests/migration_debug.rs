//! One-off: diagnose migration checksum mismatch on portfolio.db.

use sha2::{Digest, Sha384};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;

#[tokio::test]
async fn diagnose_portfolio_migration_2() {
    let path = stocker_portfolio::db::default_db_path();
    eprintln!("portfolio db: {}", path.display());

    let url = format!("sqlite://{}", path.display());
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let rows = sqlx::query("SELECT version, description, checksum, success FROM _sqlx_migrations ORDER BY version")
        .fetch_all(&pool)
        .await
        .unwrap();
    for row in &rows {
        let version: i64 = row.get("version");
        let description: String = row.get("description");
        let checksum: Vec<u8> = row.get("checksum");
        let success: bool = row.get("success");
        eprintln!(
            "applied v{version} ({description}) success={success} checksum={}",
            hex::encode(&checksum)
        );
    }

    let migrator = sqlx::migrate!("./migrations");
    for m in migrator.migrations.iter() {
        eprintln!(
            "file v{} ({}) checksum={}",
            m.version,
            m.description,
            hex::encode(m.checksum.as_ref())
        );
    }

    let sql = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0002_related_symbol.sql"
    ))
    .unwrap();
    let manual = Sha384::digest(sql.as_bytes());
    eprintln!("manual sha384 of 0002 file: {}", hex::encode(manual));

    let cols = sqlx::query("PRAGMA table_info(portfolio_snapshots)")
        .fetch_all(&pool)
        .await
        .unwrap();
    eprintln!("portfolio_snapshots columns:");
    for row in cols {
        let name: String = row.get("name");
        eprintln!("  {name}");
    }

    pool.close().await;
}

use std::str::FromStr;
