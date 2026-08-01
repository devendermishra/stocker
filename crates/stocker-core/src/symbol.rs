//! Normalize user input to Yahoo India tickers (`*.NS` / `*.BO`).
//!
//! Dual-listed stocks resolve to NSE (`.NS`) when the base symbol is in the NSE universe.

use std::collections::HashSet;
use std::path::Path;

/// NSE trading symbols (no suffix) used to prefer `.NS` over `.BO`.
#[derive(Debug, Clone, Default)]
pub struct IndiaSymbolContext {
    pub nse_bases: HashSet<String>,
}

impl IndiaSymbolContext {
    pub fn from_nse_bases(bases: impl IntoIterator<Item = String>) -> Self {
        Self {
            nse_bases: bases.into_iter().collect(),
        }
    }

    pub fn is_nse_listed(&self, base: &str) -> bool {
        self.nse_bases.contains(&base.to_uppercase())
    }
}

/// Strip `.NS` / `.BO` suffix and return uppercase base symbol.
pub fn india_base_symbol(input: &str) -> Result<String, crate::StockerError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    let upper = s.to_uppercase();
    let base = upper
        .strip_suffix(".NS")
        .or_else(|| upper.strip_suffix(".BO"))
        .unwrap_or(&upper)
        .trim();
    if base.is_empty() {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    Ok(base.to_string())
}

/// Resolve a user symbol to a Yahoo ticker, preferring NSE when listed on both exchanges.
///
/// - Explicit `.NS` input always maps to `.NS`.
/// - Otherwise, if `base` is in `ctx.nse_bases` → `.NS`.
/// - Else → `.BO`.
///
/// Rejects names with whitespace (fund names, company prose) — those are not tickers.
pub fn resolve_india_symbol(
    input: &str,
    ctx: &IndiaSymbolContext,
) -> Result<String, crate::StockerError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    if s.chars().any(char::is_whitespace) {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    let upper = s.to_uppercase();
    let base = india_base_symbol(s)?;
    if upper.ends_with(".NS") {
        return Ok(format!("{base}.NS"));
    }
    if ctx.is_nse_listed(&base) {
        Ok(format!("{base}.NS"))
    } else {
        Ok(format!("{base}.BO"))
    }
}

/// Returns uppercase Yahoo symbol ending in `.NS`.
/// Accepts `RELIANCE`, `reliance.ns`, `RELIANCE.NS`.
pub fn normalize_nse_symbol(input: &str) -> Result<String, crate::StockerError> {
    let base = india_base_symbol(input)?;
    Ok(format!("{base}.NS"))
}

/// Map a BSE security id to a Yahoo ticker (`*.BO`).
pub fn to_yahoo_bse_symbol(bse_security_id: &str) -> String {
    let s = bse_security_id.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.ends_with(".BO") {
        return s.to_uppercase();
    }
    format!("{}.BO", s.to_uppercase())
}

/// Column indices for NSE `EQUITY_L.csv`-style headers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NseCsvIndices {
    pub symbol_col: Option<usize>,
    pub series_col: Option<usize>,
    pub simple_col: bool,
}

/// Detect symbol and series column positions in an uppercased CSV header row.
pub fn nse_csv_indices(headers: &[String]) -> NseCsvIndices {
    let symbol_col = headers
        .iter()
        .position(|h| h == "SYMBOL" || h == "SYMBOLS" || h == "TICKER" || h == "SYMBOL_NS");
    let series_col = headers.iter().position(|h| h == "SERIES");
    let simple_col = symbol_col.is_none() && headers.len() <= 1;
    NseCsvIndices {
        symbol_col,
        series_col,
        simple_col,
    }
}

/// Parse NSE `EQUITY_L.csv` and return the set of trading symbols (no suffix).
pub fn load_nse_bases_from_equity_l(path: &Path) -> HashSet<String> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashSet::new();
    };
    parse_nse_bases_from_csv(&bytes)
}

/// Parse NSE `EQUITY_L.csv` bytes and return trading symbols (no suffix).
pub fn parse_nse_bases_from_csv(bytes: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(bytes);

    let headers: Vec<String> = rdr
        .headers()
        .ok()
        .map(|h| h.iter().map(|c| c.trim().to_uppercase()).collect())
        .unwrap_or_default();

    let cols = nse_csv_indices(&headers);

    for record in rdr.records().flatten() {
        let raw_sym = if cols.simple_col {
            record.get(0)
        } else {
            cols.symbol_col
                .and_then(|i| record.get(i))
                .or_else(|| record.get(0))
        }
        .map(str::trim)
        .filter(|s| !s.is_empty());

        let Some(raw_sym) = raw_sym else {
            continue;
        };
        if raw_sym.eq_ignore_ascii_case("SYMBOL") || raw_sym.starts_with('#') {
            continue;
        }
        if let Some(si) = cols.series_col {
            let series = record.get(si).unwrap_or("").trim();
            if !series.is_empty() && !series.eq_ignore_ascii_case("EQ") {
                continue;
            }
        }
        if let Ok(base) = india_base_symbol(raw_sym) {
            out.insert(base);
        }
    }
    out
}

/// Build an [`IndiaSymbolContext`] from `data/EQUITY_L.csv` when present.
pub fn default_india_symbol_context() -> IndiaSymbolContext {
    let path = Path::new("data/EQUITY_L.csv");
    if path.is_file() {
        IndiaSymbolContext::from_nse_bases(load_nse_bases_from_equity_l(path))
    } else {
        IndiaSymbolContext::default()
    }
}

/// Strip `.NS` / `.BO` for display.
pub fn india_display_ticker(yahoo_symbol: &str) -> String {
    yahoo_symbol
        .trim_end_matches(".NS")
        .trim_end_matches(".BO")
        .to_string()
}

/// Map a Yahoo ticker or Yahoo Finance exchange code to `NSE` or `BSE`.
///
/// Yahoo's quote API returns internal codes such as **NSI** (NSE India) and **YHD**
/// (BSE / historical BSE feed) — not the exchange names users expect. Prefer the
/// ticker suffix (`.NS` / `.BO`) when present.
pub fn india_exchange_label(symbol: &str, yahoo_exchange: Option<&str>) -> &'static str {
    let upper = symbol.trim().to_uppercase();
    if upper.ends_with(".NS") {
        return "NSE";
    }
    if upper.ends_with(".BO") {
        return "BSE";
    }
    if let Some(raw) = yahoo_exchange {
        let u = raw.trim().to_uppercase();
        if matches!(u.as_str(), "NSI" | "NSE") || u.contains("NATIONAL") {
            return "NSE";
        }
        if matches!(u.as_str(), "YHD" | "BOM" | "BSE") || u.contains("BOMBAY") {
            return "BSE";
        }
    }
    "NSE"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_nse(symbols: &[&str]) -> IndiaSymbolContext {
        IndiaSymbolContext::from_nse_bases(symbols.iter().map(|s| s.to_string()))
    }

    #[test]
    fn adds_ns_suffix() {
        assert_eq!(normalize_nse_symbol("RELIANCE").unwrap(), "RELIANCE.NS");
    }

    #[test]
    fn preserves_existing_ns() {
        assert_eq!(normalize_nse_symbol("TCS.NS").unwrap(), "TCS.NS");
    }

    #[test]
    fn resolve_rejects_fund_names_with_spaces() {
        let ctx = ctx_with_nse(&[]);
        assert!(resolve_india_symbol(
            "PARAG PARIKH FLEXI CAP FUND - DIRECT PLAN - GROWTH.BO",
            &ctx
        )
        .is_err());
        assert!(resolve_india_symbol("Parag Parikh Flexi Cap", &ctx).is_err());
    }

    #[test]
    fn resolve_nse_listed_bare() {
        let ctx = ctx_with_nse(&["RELIANCE"]);
        assert_eq!(resolve_india_symbol("RELIANCE", &ctx).unwrap(), "RELIANCE.NS");
    }

    #[test]
    fn resolve_nse_priority_over_bo_input() {
        let ctx = ctx_with_nse(&["RELIANCE"]);
        assert_eq!(resolve_india_symbol("RELIANCE.BO", &ctx).unwrap(), "RELIANCE.NS");
    }

    #[test]
    fn resolve_bse_only() {
        let ctx = ctx_with_nse(&["RELIANCE"]);
        assert_eq!(resolve_india_symbol("SOMEBSE", &ctx).unwrap(), "SOMEBSE.BO");
    }

    #[test]
    fn resolve_explicit_ns_preserved() {
        let ctx = ctx_with_nse(&[]);
        assert_eq!(resolve_india_symbol("FOO.NS", &ctx).unwrap(), "FOO.NS");
    }

    #[test]
    fn parse_nse_bases_from_csv_sample() {
        let sample = b"SYMBOL,NAME OF COMPANY, SERIES\nRELIANCE,Reliance Industries Ltd,EQ\nTCS,TCS Ltd,EQ\nFOO,Foo Ltd,BE\n";
        let bases = parse_nse_bases_from_csv(sample);
        assert!(bases.contains("RELIANCE"));
        assert!(bases.contains("TCS"));
        assert!(!bases.contains("FOO"));
    }

    #[test]
    fn display_ticker_strips_suffixes() {
        assert_eq!(india_display_ticker("RELIANCE.NS"), "RELIANCE");
        assert_eq!(india_display_ticker("SOMEBSE.BO"), "SOMEBSE");
    }

    #[test]
    fn exchange_label_from_suffix_and_yahoo_codes() {
        assert_eq!(india_exchange_label("RELIANCE.NS", Some("NSI")), "NSE");
        assert_eq!(india_exchange_label("FOO.BO", Some("YHD")), "BSE");
        assert_eq!(india_exchange_label("FOO", Some("NSI")), "NSE");
        assert_eq!(india_exchange_label("FOO", Some("YHD")), "BSE");
    }

    #[test]
    fn stored_exchange_never_yahoo_codes_for_suffixed_tickers() {
        assert_eq!(india_exchange_label("MOVINGPI.BO", Some("YHD")), "BSE");
        assert_eq!(india_exchange_label("RELIANCE.NS", Some("NSI")), "NSE");
    }
}
