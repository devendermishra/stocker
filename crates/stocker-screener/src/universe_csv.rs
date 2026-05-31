//! Load screener universe from a **local** CSV file (user-provided).
//!
//! Stocker does not call NSE websites or APIs. Download symbol lists yourself
//! (e.g. NSE "Securities available for trading" CSV) and point `STOCKER_UNIVERSE_CSV`
//! or `stocker-cli universe --csv` at that file.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::universe::DiscoveredSymbol;

/// Environment variable: path to a local universe CSV (no network fetch).
pub const ENV_UNIVERSE_CSV: &str = "STOCKER_UNIVERSE_CSV";

/// Map an NSE trading symbol to a Yahoo Finance ticker (`*.NS`).
pub fn to_yahoo_symbol(nse_symbol: &str) -> String {
    let s = nse_symbol.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.ends_with(".NS") || s.ends_with(".BO") {
        return s.to_uppercase();
    }
    format!("{}.NS", s.to_uppercase())
}

/// Resolved path from `STOCKER_UNIVERSE_CSV`, if set.
pub fn universe_csv_path() -> Option<PathBuf> {
    std::env::var(ENV_UNIVERSE_CSV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Read and parse a local universe CSV.
pub async fn load_universe_csv(path: &Path) -> Result<Vec<DiscoveredSymbol>> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| Error::Other(format!("read universe CSV {}: {e}", path.display())))?;
    Ok(parse_universe_csv(&bytes))
}

/// Parse universe CSV bytes.
///
/// Supported formats:
/// - NSE `EQUITY_L.csv`: `SYMBOL`, optional `SERIES` (only `EQ` rows kept), `FACE VALUE`
/// - Simple list: header `symbol` or a single column of tickers (with or without `.NS`)
pub fn parse_universe_csv(bytes: &[u8]) -> Vec<DiscoveredSymbol> {
    let mut out = Vec::new();
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(bytes);

    let headers: Vec<String> = rdr
        .headers()
        .ok()
        .map(|h| h.iter().map(|c| c.trim().to_uppercase()).collect())
        .unwrap_or_default();

    let sym_idx = headers
        .iter()
        .position(|h| h == "SYMBOL" || h == "SYMBOLS" || h == "TICKER" || h == "SYMBOL_NS");
    let name_idx = headers
        .iter()
        .position(|h| h.starts_with("NAME") || h.contains("COMPANY"));
    let series_idx = headers.iter().position(|h| h == "SERIES");
    let face_idx = headers
        .iter()
        .position(|h| h == "FACE VALUE" || h == "FACEVALUE");
    let paid_up_idx = headers
        .iter()
        .position(|h| h == "PAID UP VALUE" || h == "PAIDUPVALUE");

    // Simple one-column file (e.g. bundled nifty500.csv).
    let simple_col = sym_idx.is_none() && headers.len() <= 1;

    for record in rdr.records().flatten() {
        let raw_sym = if simple_col {
            record.get(0)
        } else {
            sym_idx.and_then(|i| record.get(i)).or_else(|| record.get(0))
        }
        .map(str::trim)
        .filter(|s| !s.is_empty());

        let Some(raw_sym) = raw_sym else {
            continue;
        };
        if raw_sym.eq_ignore_ascii_case("SYMBOL") || raw_sym.starts_with('#') {
            continue;
        }
        if let Some(si) = series_idx {
            let series = record.get(si).unwrap_or("").trim();
            if !series.is_empty() && !series.eq_ignore_ascii_case("EQ") {
                continue;
            }
        }
        let yahoo = to_yahoo_symbol(raw_sym);
        if yahoo.is_empty() {
            continue;
        }
        let short_name = name_idx.and_then(|i| record.get(i)).map(|s| s.trim().to_string());
        let face_value = face_idx
            .and_then(|i| record.get(i))
            .or_else(|| paid_up_idx.and_then(|i| record.get(i)))
            .and_then(parse_face_value);
        out.push(DiscoveredSymbol {
            symbol: yahoo,
            short_name,
            exchange: Some("NSE".to_string()),
            currency: Some("INR".to_string()),
            face_value,
            ..Default::default()
        });
    }
    out
}

fn parse_face_value(raw: &str) -> Option<f64> {
    let v: f64 = raw.trim().parse().ok()?;
    if v > 0.0 && v.is_finite() {
        Some(v)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yahoo_symbol_suffix() {
        assert_eq!(to_yahoo_symbol("reliance"), "RELIANCE.NS");
        assert_eq!(to_yahoo_symbol("RELIANCE.NS"), "RELIANCE.NS");
        assert_eq!(to_yahoo_symbol("BAJAJ-AUTO"), "BAJAJ-AUTO.NS");
    }

    #[test]
    fn parse_nse_equity_l_sample() {
        let sample = b"SYMBOL,NAME OF COMPANY, SERIES\nRELIANCE,Reliance Industries Ltd,EQ\nTCS,TCS Ltd,EQ\nFOO,Foo Ltd,BE\n";
        let rows = parse_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.symbol == "RELIANCE.NS"));
    }

    #[test]
    fn parse_equity_l_face_value_column() {
        let sample = b"SYMBOL,NAME OF COMPANY, SERIES, DATE OF LISTING, PAID UP VALUE, MARKET LOT, ISIN NUMBER, FACE VALUE\nRELIANCE,Reliance Industries Limited,EQ,29-NOV-1977,10,1,INE002A01018,10\nTCS,Tata Consultancy Services Limited,EQ,25-AUG-2004,1,1,INE467B01029,1\n";
        let rows = parse_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        let rel = rows.iter().find(|r| r.symbol == "RELIANCE.NS").unwrap();
        assert_eq!(rel.face_value, Some(10.0));
        let tcs = rows.iter().find(|r| r.symbol == "TCS.NS").unwrap();
        assert_eq!(tcs.face_value, Some(1.0));
    }

    #[test]
    fn universe_module_has_no_http_client_dependency() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            !manifest.contains("reqwest"),
            "stocker-screener must not perform HTTP fetches; use stocker-core (Yahoo only)"
        );
    }
}
