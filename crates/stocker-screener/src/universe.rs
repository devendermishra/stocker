//! Universe definition for the screener (local CSV only — no NSE network access).
//!
//! Metric refresh uses Yahoo Finance (`stocker-core`, same endpoints as yfinance).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};

use crate::error::Result;
use crate::snapshot::SymbolRow;
use crate::universe_csv::{self, load_universe_csv, universe_csv_path};

const NIFTY500_CSV: &str = include_str!("../data/nifty500.csv");
const DEFAULT_EQUITY_L: &str = "data/EQUITY_L.csv";

/// One symbol in the screener universe (Yahoo ticker `SYMBOL.NS`).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DiscoveredSymbol {
    pub symbol: String,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    /// Par / face value per share from NSE `EQUITY_L.csv` when available.
    pub face_value: Option<f64>,
}

/// Parse the bundled NIFTY 500 CSV into a `HashSet<String>` of `SYMBOL.NS` tickers.
pub fn nifty500_seed() -> HashSet<String> {
    let mut out = HashSet::new();
    for row in universe_csv::parse_universe_csv(NIFTY500_CSV.as_bytes()) {
        out.insert(row.symbol);
    }
    out
}

/// Load universe from a user CSV path, then `STOCKER_UNIVERSE_CSV`, then bundled NIFTY 500.
pub async fn discover_universe(csv_override: Option<&Path>) -> Vec<DiscoveredSymbol> {
    let candidates: Vec<PathBuf> = csv_override
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_else(|| {
            let mut v: Vec<PathBuf> = universe_csv_path().into_iter().collect();
            let default_equity = PathBuf::from(DEFAULT_EQUITY_L);
            if default_equity.is_file() {
                v.push(default_equity);
            }
            v
        });

    for path in candidates {
        match load_universe_csv(&path).await {
            Ok(rows) if !rows.is_empty() => {
                log::info!(
                    "screener universe: {} symbols from {}",
                    rows.len(),
                    path.display()
                );
                return rows;
            }
            Ok(_) => log::warn!("screener universe CSV is empty: {}", path.display()),
            Err(e) => log::warn!("screener universe CSV {}: {e}", path.display()),
        }
    }

    let n = nifty500_seed().len();
    log::warn!(
        "no universe CSV loaded — using bundled NIFTY 500 ({n}). \
         Set {} or run `stocker-cli universe --csv <file>` with a list you obtained legally.",
        universe_csv::ENV_UNIVERSE_CSV,
    );
    universe_csv::parse_universe_csv(NIFTY500_CSV.as_bytes())
}

/// Sync the discovered universe into the `symbols` table. New symbols are inserted,
/// existing ones get their identity fields refreshed. Tier 0 = NIFTY 500 (per
/// bundled seed), tier 1 = everything else.
pub async fn sync_universe(pool: &SqlitePool, discovered: &[DiscoveredSymbol]) -> Result<usize> {
    let seed = nifty500_seed();
    let mut inserted = 0usize;
    for d in discovered {
        let tier = if seed.contains(&d.symbol) { 0 } else { 1 };
        let row = SymbolRow {
            symbol: d.symbol.clone(),
            short_name: d.short_name.clone(),
            sector: d.sector.clone(),
            industry: d.industry.clone(),
            exchange: d.exchange.clone(),
            currency: d.currency.clone(),
            country: Some("India".to_string()),
            face_value: d.face_value,
            tier,
            ..Default::default()
        };
        row.upsert_identity(pool).await?;
        inserted += 1;
    }
    for sym in seed {
        let exists = sqlx::query("SELECT 1 FROM symbols WHERE symbol = ?")
            .bind(&sym)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            SymbolRow {
                symbol: sym,
                tier: 0,
                country: Some("India".to_string()),
                ..Default::default()
            }
            .upsert_identity(pool)
            .await?;
            inserted += 1;
        }
    }
    Ok(inserted)
}

/// Read meta.last_universe_sync_at; `0` if never run.
pub async fn last_sync_at(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT value FROM meta WHERE key = 'last_universe_sync_at'")
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(0);
    };
    let v: String = row.try_get("value")?;
    Ok(v.parse::<i64>().unwrap_or(0))
}

pub async fn record_sync(pool: &SqlitePool, ts: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO meta (key, value) VALUES ('last_universe_sync_at', ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(ts.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

/// Whether automatic universe re-sync is allowed (local file configured).
pub fn auto_sync_enabled() -> bool {
    if universe_csv_path()
        .map(|p| p.is_file())
        .unwrap_or(false)
    {
        return true;
    }
    Path::new(DEFAULT_EQUITY_L).is_file()
}
