//! Universe definition for the screener (local CSV only — no exchange network access).
//!
//! Metric refresh uses Yahoo Finance (`stocker-core`, same endpoints as yfinance).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};
use stocker_core::IndiaSymbolContext;

use crate::error::Result;
use crate::snapshot::{remove_dual_listed_bse_symbols, SymbolRow};
use crate::universe_csv::{
    self, bse_universe_csv_path, detect_universe_csv_format, merge_india_universes,
    parse_universe_csv, universe_csv_path, UniverseCsvFormat,
};

const NIFTY500_CSV: &str = include_str!("../data/nifty500.csv");
const DEFAULT_EQUITY_L: &str = "data/EQUITY_L.csv";
pub const DEFAULT_BSE_EQUITY_L: &str = "data/EQUITY_L_BSE.csv";

/// One symbol in the screener universe (Yahoo ticker `SYMBOL.NS` or `SYMBOL.BO`).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct DiscoveredSymbol {
    pub symbol: String,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    /// Par / face value per share from exchange CSV when available.
    pub face_value: Option<f64>,
    /// ISIN from exchange CSV when available (used for NSE/BSE dedup).
    pub isin: Option<String>,
}

/// Parse the bundled NIFTY 500 CSV into a `HashSet<String>` of `SYMBOL.NS` tickers.
pub fn nifty500_seed() -> HashSet<String> {
    let mut out = HashSet::new();
    for row in universe_csv::parse_nse_universe_csv(NIFTY500_CSV.as_bytes()) {
        out.insert(row.symbol);
    }
    out
}

/// NSE trading symbols (no suffix) from a discovered universe slice.
pub fn nse_bases_from_discovered(discovered: &[DiscoveredSymbol]) -> HashSet<String> {
    discovered
        .iter()
        .filter(|d| d.symbol.ends_with(".NS"))
        .filter_map(|d| stocker_core::india_base_symbol(&d.symbol).ok())
        .collect()
}

/// Build an [`IndiaSymbolContext`] from discovered universe rows.
pub fn india_symbol_context_from_discovered(discovered: &[DiscoveredSymbol]) -> IndiaSymbolContext {
    IndiaSymbolContext::from_nse_bases(nse_bases_from_discovered(discovered))
}

/// Load NSE bases from symbols currently stored in the screener DB.
pub async fn nse_bases_from_db(pool: &SqlitePool) -> Result<HashSet<String>> {
    let rows = sqlx::query("SELECT symbol FROM symbols WHERE symbol LIKE '%.NS'")
        .fetch_all(pool)
        .await?;
    let mut out = HashSet::new();
    for row in rows {
        let sym: String = row.try_get("symbol")?;
        if let Ok(base) = stocker_core::india_base_symbol(&sym) {
            out.insert(base);
        }
    }
    Ok(out)
}

/// Build an [`IndiaSymbolContext`] from the screener DB (falls back to `data/EQUITY_L.csv`).
pub async fn india_symbol_context_from_db(pool: &SqlitePool) -> Result<IndiaSymbolContext> {
    let bases = nse_bases_from_db(pool).await?;
    if bases.is_empty() {
        Ok(stocker_core::default_india_symbol_context())
    } else {
        Ok(IndiaSymbolContext::from_nse_bases(bases))
    }
}

/// Load universe from local CSV files, merging NSE + BSE with NSE price priority.
pub async fn discover_universe(
    nse_csv_override: Option<&Path>,
    bse_csv_override: Option<&Path>,
) -> Vec<DiscoveredSymbol> {
    let (nse_rows, bse_rows, combined) = load_universe_csvs(nse_csv_override, bse_csv_override).await;
    if let Some(rows) = combined {
        return rows;
    }
    if !nse_rows.is_empty() || !bse_rows.is_empty() {
        let merged = merge_india_universes(nse_rows, bse_rows);
        log::info!("screener universe: {} symbols after NSE/BSE merge", merged.len());
        return merged;
    }

    let n = nifty500_seed().len();
    log::warn!(
        "no universe CSV loaded — using bundled NIFTY 500 ({n}). \
         Set {} / {} or run `stocker-cli universe --csv <file>` with lists you obtained legally.",
        universe_csv::ENV_UNIVERSE_CSV,
        universe_csv::ENV_BSE_UNIVERSE_CSV,
    );
    universe_csv::parse_nse_universe_csv(NIFTY500_CSV.as_bytes())
}

/// Load universe from local CSV files: NSE rows first, then BSE-only symbols
/// (dual-listed names/ISINs/ticker bases collapsed to the NSE listing).
pub async fn discover_universe_all(
    nse_csv_override: Option<&Path>,
    bse_csv_override: Option<&Path>,
) -> Vec<DiscoveredSymbol> {
    let (nse_rows, bse_rows, combined) = load_universe_csvs(nse_csv_override, bse_csv_override).await;
    if let Some(rows) = combined {
        return rows;
    }
    if !nse_rows.is_empty() || !bse_rows.is_empty() {
        let merged = merge_india_universes(nse_rows, bse_rows);
        log::info!(
            "screener universe: {} symbols (NSE first, BSE-only where no NSE match)",
            merged.len()
        );
        return merged;
    }

    let n = nifty500_seed().len();
    log::warn!(
        "no universe CSV loaded — using bundled NIFTY 500 ({n}). \
         Set {} / {} or run `stocker-cli universe --csv <file>` with lists you obtained legally.",
        universe_csv::ENV_UNIVERSE_CSV,
        universe_csv::ENV_BSE_UNIVERSE_CSV,
    );
    universe_csv::parse_nse_universe_csv(NIFTY500_CSV.as_bytes())
}

/// Returns `(nse_rows, bse_rows, combined_if_single_csv)`.
async fn load_universe_csvs(
    nse_csv_override: Option<&Path>,
    bse_csv_override: Option<&Path>,
) -> (Vec<DiscoveredSymbol>, Vec<DiscoveredSymbol>, Option<Vec<DiscoveredSymbol>>) {
    let mut nse_rows = Vec::new();
    let mut bse_rows = Vec::new();

    for path in nse_csv_candidates(nse_csv_override) {
        match load_csv_bytes(&path).await {
            Ok((bytes, path)) => {
                let fmt = detect_universe_csv_format(&bytes);
                match fmt {
                    UniverseCsvFormat::Combined => {
                        let rows = parse_universe_csv(&bytes);
                        if !rows.is_empty() {
                            log::info!(
                                "screener universe: {} symbols from combined CSV {}",
                                rows.len(),
                                path.display()
                            );
                            return (Vec::new(), Vec::new(), Some(rows));
                        }
                    }
                    UniverseCsvFormat::Bse => {
                        let rows = universe_csv::parse_bse_universe_csv(&bytes);
                        if !rows.is_empty() {
                            bse_rows = rows;
                            log::info!(
                                "screener universe: {} BSE symbols from {}",
                                bse_rows.len(),
                                path.display()
                            );
                            break;
                        }
                    }
                    UniverseCsvFormat::Nse => {
                        let rows = universe_csv::parse_nse_universe_csv(&bytes);
                        if !rows.is_empty() {
                            nse_rows = rows;
                            log::info!(
                                "screener universe: {} NSE symbols from {}",
                                nse_rows.len(),
                                path.display()
                            );
                            break;
                        }
                    }
                }
            }
            Err(e) => log::warn!("screener universe CSV {}: {e}", path.display()),
        }
    }

    for path in bse_csv_candidates(bse_csv_override) {
        match load_csv_bytes(&path).await {
            Ok((bytes, path)) => {
                let rows = universe_csv::parse_bse_universe_csv(&bytes);
                if !rows.is_empty() {
                    bse_rows = rows;
                    log::info!(
                        "screener universe: {} BSE symbols from {}",
                        bse_rows.len(),
                        path.display()
                    );
                    break;
                }
            }
            Err(e) => log::warn!("screener BSE universe CSV {}: {e}", path.display()),
        }
    }

    (nse_rows, bse_rows, None)
}

async fn load_csv_bytes(path: &Path) -> Result<(Vec<u8>, PathBuf)> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| crate::error::Error::Other(format!("read universe CSV {}: {e}", path.display())))?;
    Ok((bytes, path.to_path_buf()))
}

fn nse_csv_candidates(override_path: Option<&Path>) -> Vec<PathBuf> {
    if let Some(p) = override_path {
        return vec![p.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = universe_csv_path().into_iter().collect();
    let default_equity = PathBuf::from(DEFAULT_EQUITY_L);
    if default_equity.is_file() {
        v.push(default_equity);
    }
    v
}

fn bse_csv_candidates(override_path: Option<&Path>) -> Vec<PathBuf> {
    if let Some(p) = override_path {
        return vec![p.to_path_buf()];
    }
    let mut v: Vec<PathBuf> = bse_universe_csv_path().into_iter().collect();
    let default_bse = PathBuf::from(DEFAULT_BSE_EQUITY_L);
    if default_bse.is_file() {
        v.push(default_bse);
    }
    v
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
            isin: d.isin.clone(),
            tier,
            ..Default::default()
        };
        row.upsert_identity(pool).await?;
        inserted += 1;
    }
    let removed = remove_dual_listed_bse_symbols(pool).await?;
    if removed > 0 {
        log::info!("screener universe: removed {removed} duplicate BSE symbol row(s)");
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
    if bse_universe_csv_path()
        .map(|p| p.is_file())
        .unwrap_or(false)
    {
        return true;
    }
    Path::new(DEFAULT_EQUITY_L).is_file() || Path::new(DEFAULT_BSE_EQUITY_L).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::universe_csv::merge_india_universes;

    #[tokio::test]
    async fn sync_universe_stores_nse_and_bse_on_first_load() {
        let pool = db::open_memory().await.unwrap();
        let nse = vec![DiscoveredSymbol {
            symbol: "RELIANCE.NS".to_string(),
            short_name: Some("Reliance Industries Ltd".to_string()),
            exchange: Some("NSE".to_string()),
            currency: Some("INR".to_string()),
            isin: Some("INE002A01018".to_string()),
            ..Default::default()
        }];
        let bse = vec![
            DiscoveredSymbol {
                symbol: "RELIANCE.BO".to_string(),
                short_name: Some("Reliance Industries Ltd".to_string()),
                exchange: Some("BSE".to_string()),
                currency: Some("INR".to_string()),
                isin: Some("INE002A01018".to_string()),
                ..Default::default()
            },
            DiscoveredSymbol {
                symbol: "SOMEBSE.BO".to_string(),
                short_name: Some("Some BSE Co Ltd".to_string()),
                exchange: Some("BSE".to_string()),
                currency: Some("INR".to_string()),
                isin: Some("INE999Z01099".to_string()),
                ..Default::default()
            },
        ];
        let discovered = merge_india_universes(nse, bse);
        sync_universe(&pool, &discovered).await.unwrap();

        let nse: String =
            sqlx::query_scalar("SELECT exchange FROM symbols WHERE symbol = 'RELIANCE.NS'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let bse: String =
            sqlx::query_scalar("SELECT exchange FROM symbols WHERE symbol = 'SOMEBSE.BO'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(nse, "NSE");
        assert_eq!(bse, "BSE");

        let dup: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM symbols WHERE symbol = 'RELIANCE.BO'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(dup, 0);
    }
}
