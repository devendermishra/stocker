//! Load screener universe from a **local** CSV file (user-provided).
//!
//! Stocker does not call NSE or BSE websites or APIs. Download symbol lists yourself
//! (e.g. NSE `EQUITY_L.csv`, BSE "List of Securities" export) and point
//! `STOCKER_UNIVERSE_CSV` / `STOCKER_BSE_UNIVERSE_CSV` or CLI flags at those files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::universe::DiscoveredSymbol;

/// Environment variable: path to a local NSE universe CSV (no network fetch).
pub const ENV_UNIVERSE_CSV: &str = "STOCKER_UNIVERSE_CSV";
/// Environment variable: path to a local BSE universe CSV (no network fetch).
pub const ENV_BSE_UNIVERSE_CSV: &str = "STOCKER_BSE_UNIVERSE_CSV";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseCsvFormat {
    Nse,
    Bse,
    Combined,
}

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

/// Map a BSE security id to a Yahoo Finance ticker (`*.BO`).
pub fn to_yahoo_bse_symbol(bse_security_id: &str) -> String {
    stocker_core::to_yahoo_bse_symbol(bse_security_id)
}

/// Resolved path from `STOCKER_UNIVERSE_CSV`, if set.
pub fn universe_csv_path() -> Option<PathBuf> {
    std::env::var(ENV_UNIVERSE_CSV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Resolved path from `STOCKER_BSE_UNIVERSE_CSV`, if set.
pub fn bse_universe_csv_path() -> Option<PathBuf> {
    std::env::var(ENV_BSE_UNIVERSE_CSV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Read and parse a local universe CSV (format auto-detected).
pub async fn load_universe_csv(path: &Path) -> Result<Vec<DiscoveredSymbol>> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| Error::Other(format!("read universe CSV {}: {e}", path.display())))?;
    Ok(parse_universe_csv(&bytes))
}

/// Parse universe CSV bytes with format auto-detection.
pub fn parse_universe_csv(bytes: &[u8]) -> Vec<DiscoveredSymbol> {
    let headers = read_csv_headers(bytes);
    match detect_csv_format(&headers) {
        UniverseCsvFormat::Bse => parse_bse_universe_csv(bytes),
        UniverseCsvFormat::Combined => parse_combined_universe_csv(bytes),
        UniverseCsvFormat::Nse => parse_nse_universe_csv(bytes),
    }
}

/// Parse NSE `EQUITY_L.csv` or simple NSE symbol list.
pub fn parse_nse_universe_csv(bytes: &[u8]) -> Vec<DiscoveredSymbol> {
    parse_exchange_universe_csv(bytes, ExchangeParseMode::Nse)
}

/// Parse BSE "List of Securities" export.
pub fn parse_bse_universe_csv(bytes: &[u8]) -> Vec<DiscoveredSymbol> {
    parse_exchange_universe_csv(bytes, ExchangeParseMode::Bse)
}

/// Parse a combined CSV with an `EXCHANGE` column or explicit `.NS` / `.BO` suffixes.
pub fn parse_combined_universe_csv(bytes: &[u8]) -> Vec<DiscoveredSymbol> {
    parse_exchange_universe_csv(bytes, ExchangeParseMode::Combined)
}

/// Merge NSE and BSE universes; dual-listed stocks (same ISIN, ticker base, or
/// normalized company name) keep the NSE row only.
pub fn merge_india_universes(
    nse: Vec<DiscoveredSymbol>,
    bse: Vec<DiscoveredSymbol>,
) -> Vec<DiscoveredSymbol> {
    use crate::symbols::normalize_company_name;

    let mut out = nse;
    let mut nse_isins = HashSet::new();
    let mut nse_bases = HashSet::new();
    let mut nse_names = HashSet::new();

    for row in &out {
        if let Some(isin) = normalize_isin(row.isin.as_deref()) {
            nse_isins.insert(isin);
        }
        if let Some(base) = yahoo_base_symbol(&row.symbol) {
            nse_bases.insert(base);
        }
        if let Some(ref name) = row.short_name {
            let n = normalize_company_name(name);
            if !n.is_empty() {
                nse_names.insert(n);
            }
        }
    }

    for bse_row in bse {
        if let Some(isin) = normalize_isin(bse_row.isin.as_deref()) {
            if nse_isins.contains(&isin) {
                continue;
            }
        }
        if let Some(base) = yahoo_base_symbol(&bse_row.symbol) {
            if nse_bases.contains(&base) {
                continue;
            }
        }
        if let Some(ref name) = bse_row.short_name {
            let n = normalize_company_name(name);
            if !n.is_empty() && nse_names.contains(&n) {
                continue;
            }
        }
        out.push(bse_row);
    }
    out
}

enum ExchangeParseMode {
    Nse,
    Bse,
    Combined,
}

fn read_csv_headers(bytes: &[u8]) -> Vec<String> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(bytes);
    rdr.headers()
        .ok()
        .map(|h| h.iter().map(|c| c.trim().to_uppercase()).collect())
        .unwrap_or_default()
}

/// Detect the universe CSV format from raw bytes.
pub fn detect_universe_csv_format(bytes: &[u8]) -> UniverseCsvFormat {
    detect_csv_format(&read_csv_headers(bytes))
}

fn detect_csv_format(headers: &[String]) -> UniverseCsvFormat {
    if headers.iter().any(|h| h == "EXCHANGE") {
        return UniverseCsvFormat::Combined;
    }
    let has_bse_id = headers.iter().any(|h| {
        h == "SECURITY ID"
            || h == "SCRIP_ID"
            || h == "SECURITY_ID"
            || h == "SECURITYID"
    });
    let has_security_code = headers
        .iter()
        .any(|h| h == "SECURITY CODE" || h == "SCRIP_CD" || h == "SECURITY_CODE");
    let has_nse_symbol = headers
        .iter()
        .any(|h| h == "SYMBOL" || h == "SYMBOLS" || h == "TICKER" || h == "SYMBOL_NS");
    if has_bse_id || (has_security_code && !has_nse_symbol) {
        return UniverseCsvFormat::Bse;
    }
    UniverseCsvFormat::Nse
}

fn parse_exchange_universe_csv(bytes: &[u8], mode: ExchangeParseMode) -> Vec<DiscoveredSymbol> {
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

    let nse_cols = stocker_core::nse_csv_indices(&headers);
    let sym_idx = nse_cols.symbol_col;
    let bse_id_idx = headers.iter().position(|h| {
        h == "SECURITY ID"
            || h == "SCRIP_ID"
            || h == "SECURITY_ID"
            || h == "SECURITYID"
    });
    let name_idx = headers.iter().position(|h| {
        h.starts_with("NAME")
            || h.contains("COMPANY")
            || h == "ISSUER NAME"
            || h == "SECURITY NAME"
            || h == "SCRIP_NAME"
    });
    let series_idx = nse_cols.series_col;
    let status_idx = headers.iter().position(|h| h == "STATUS");
    let exchange_idx = headers.iter().position(|h| h == "EXCHANGE");
    let isin_idx = headers.iter().position(|h| {
        h == "ISIN NUMBER"
            || h == "ISIN NO"
            || h == "ISIN_NO"
            || h == "ISIN_NUMBER"
            || h == "ISIN"
    });
    let face_idx = headers
        .iter()
        .position(|h| h == "FACE VALUE" || h == "FACEVALUE");
    let paid_up_idx = headers
        .iter()
        .position(|h| h == "PAID UP VALUE" || h == "PAIDUPVALUE");

    let simple_col = nse_cols.simple_col
        && bse_id_idx.is_none()
        && exchange_idx.is_none();

    for record in rdr.records().flatten() {
        let raw_sym = if simple_col {
            record.get(0)
        } else if matches!(mode, ExchangeParseMode::Bse) {
            bse_id_idx
                .and_then(|i| record.get(i))
                .or_else(|| sym_idx.and_then(|i| record.get(i)))
                .or_else(|| record.get(0))
        } else {
            sym_idx
                .and_then(|i| record.get(i))
                .or_else(|| record.get(0))
        }
        .map(str::trim)
        .filter(|s| !s.is_empty());

        let Some(raw_sym) = raw_sym else {
            continue;
        };
        if raw_sym.eq_ignore_ascii_case("SYMBOL")
            || raw_sym.eq_ignore_ascii_case("SECURITY ID")
            || raw_sym.starts_with('#')
        {
            continue;
        }

        if let Some(si) = series_idx {
            let series = record.get(si).unwrap_or("").trim();
            if !series.is_empty() && !series.eq_ignore_ascii_case("EQ") {
                continue;
            }
        }
        if let Some(si) = status_idx {
            let status = record.get(si).unwrap_or("").trim();
            if !status.is_empty() && !status.eq_ignore_ascii_case("ACTIVE") {
                continue;
            }
        }

        let exchange_hint = exchange_idx
            .and_then(|i| record.get(i))
            .map(|s| s.trim().to_uppercase());
        let (yahoo, exchange) = resolve_yahoo_ticker(raw_sym, &mode, exchange_hint.as_deref());
        if yahoo.is_empty() {
            continue;
        }

        let short_name = name_idx.and_then(|i| record.get(i)).map(|s| s.trim().to_string());
        let face_value = face_idx
            .and_then(|i| record.get(i))
            .or_else(|| paid_up_idx.and_then(|i| record.get(i)))
            .and_then(parse_face_value);
        let isin = isin_idx
            .and_then(|i| record.get(i))
            .and_then(|s| normalize_isin(Some(s)));

        out.push(DiscoveredSymbol {
            symbol: yahoo,
            short_name,
            exchange: Some(exchange.to_string()),
            currency: Some("INR".to_string()),
            face_value,
            isin,
            ..Default::default()
        });
    }
    out
}

fn resolve_yahoo_ticker(
    raw_sym: &str,
    mode: &ExchangeParseMode,
    exchange_hint: Option<&str>,
) -> (String, &'static str) {
    let upper = raw_sym.to_uppercase();
    if upper.ends_with(".NS") {
        return (upper, "NSE");
    }
    if upper.ends_with(".BO") {
        return (upper, "BSE");
    }

    match mode {
        ExchangeParseMode::Bse => (to_yahoo_bse_symbol(raw_sym), "BSE"),
        ExchangeParseMode::Nse => (to_yahoo_symbol(raw_sym), "NSE"),
        ExchangeParseMode::Combined => {
            let exchange = exchange_hint.unwrap_or("NSE");
            if exchange.contains("BSE") {
                (to_yahoo_bse_symbol(raw_sym), "BSE")
            } else {
                (to_yahoo_symbol(raw_sym), "NSE")
            }
        }
    }
}

fn normalize_isin(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() || s == "-" {
        return None;
    }
    Some(s.to_uppercase())
}

fn yahoo_base_symbol(yahoo: &str) -> Option<String> {
    let upper = yahoo.to_uppercase();
    let base = upper
        .strip_suffix(".NS")
        .or_else(|| upper.strip_suffix(".BO"))
        .unwrap_or(&upper);
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
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
    fn yahoo_bse_symbol_suffix() {
        assert_eq!(to_yahoo_bse_symbol("reliance"), "RELIANCE.BO");
        assert_eq!(to_yahoo_bse_symbol("RELIANCE.BO"), "RELIANCE.BO");
    }

    #[test]
    fn parse_nse_equity_l_sample() {
        let sample = b"SYMBOL,NAME OF COMPANY, SERIES\nRELIANCE,Reliance Industries Ltd,EQ\nTCS,TCS Ltd,EQ\nFOO,Foo Ltd,BE\n";
        let rows = parse_nse_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.symbol == "RELIANCE.NS"));
    }

    #[test]
    fn parse_equity_l_face_value_and_isin() {
        let sample = b"SYMBOL,NAME OF COMPANY, SERIES, DATE OF LISTING, PAID UP VALUE, MARKET LOT, ISIN NUMBER, FACE VALUE\nRELIANCE,Reliance Industries Limited,EQ,29-NOV-1977,10,1,INE002A01018,10\nTCS,Tata Consultancy Services Limited,EQ,25-AUG-2004,1,1,INE467B01029,1\n";
        let rows = parse_nse_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        let rel = rows.iter().find(|r| r.symbol == "RELIANCE.NS").unwrap();
        assert_eq!(rel.face_value, Some(10.0));
        assert_eq!(rel.isin.as_deref(), Some("INE002A01018"));
        let tcs = rows.iter().find(|r| r.symbol == "TCS.NS").unwrap();
        assert_eq!(tcs.face_value, Some(1.0));
    }

    #[test]
    fn parse_bse_sample() {
        let sample = b"Security Code,Issuer Name,Security Id,Security Name,Status,Group,Face Value,ISIN No\n500325,Reliance Industries Ltd,RELIANCE,Reliance Industries Ltd,Active,A,10,INE002A01018\n543210,Some BSE Co,SOMEBSE,Some BSE Co Ltd,Active,B,10,INE999Z01099\n";
        let rows = parse_bse_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.symbol == "RELIANCE.BO"));
        assert!(rows.iter().any(|r| r.symbol == "SOMEBSE.BO"));
        let rel = rows.iter().find(|r| r.symbol == "RELIANCE.BO").unwrap();
        assert_eq!(rel.exchange.as_deref(), Some("BSE"));
        assert_eq!(rel.isin.as_deref(), Some("INE002A01018"));
    }

    #[test]
    fn merge_dedup_by_isin() {
        let nse = vec![DiscoveredSymbol {
            symbol: "RELIANCE.NS".to_string(),
            exchange: Some("NSE".to_string()),
            isin: Some("INE002A01018".to_string()),
            ..Default::default()
        }];
        let bse = vec![
            DiscoveredSymbol {
                symbol: "RELIANCE.BO".to_string(),
                exchange: Some("BSE".to_string()),
                isin: Some("INE002A01018".to_string()),
                ..Default::default()
            },
            DiscoveredSymbol {
                symbol: "SOMEBSE.BO".to_string(),
                exchange: Some("BSE".to_string()),
                isin: Some("INE999Z01099".to_string()),
                ..Default::default()
            },
        ];
        let merged = merge_india_universes(nse, bse);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|r| r.symbol == "RELIANCE.NS"));
        assert!(merged.iter().any(|r| r.symbol == "SOMEBSE.BO"));
        assert!(!merged.iter().any(|r| r.symbol == "RELIANCE.BO"));
    }

    #[test]
    fn merge_dedup_by_normalized_name_without_bse_isin() {
        let nse = vec![DiscoveredSymbol {
            symbol: "RELIANCE.NS".to_string(),
            short_name: Some("Reliance Industries Limited".to_string()),
            exchange: Some("NSE".to_string()),
            isin: Some("INE002A01018".to_string()),
            ..Default::default()
        }];
        let bse = vec![DiscoveredSymbol {
            symbol: "RELIANCE.BO".to_string(),
            short_name: Some("RELIANCE INDUSTRIES LTD.".to_string()),
            exchange: Some("BSE".to_string()),
            isin: None,
            ..Default::default()
        }];
        let merged = merge_india_universes(nse, bse);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].symbol, "RELIANCE.NS");
    }

    #[test]
    fn parse_combined_csv_with_exchange_column() {
        let sample = b"SYMBOL,EXCHANGE,ISIN\nRELIANCE,NSE,INE002A01018\nSOMEBSE,BSE,INE999Z01099\n";
        let rows = parse_combined_universe_csv(sample);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.symbol == "RELIANCE.NS"));
        assert!(rows.iter().any(|r| r.symbol == "SOMEBSE.BO"));
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
