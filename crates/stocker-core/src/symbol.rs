//! Load the NSE symbol universe from disk. Parsing and ticker rules live in `stocker-domain`.

use std::collections::HashSet;
use std::path::Path;

use stocker_domain::symbol::{parse_nse_bases_from_csv, IndiaSymbolContext};

/// Parse NSE `EQUITY_L.csv` and return the set of trading symbols (no suffix).
pub fn load_nse_bases_from_equity_l(path: &Path) -> HashSet<String> {
    let Ok(bytes) = std::fs::read(path) else {
        return HashSet::new();
    };
    parse_nse_bases_from_csv(&bytes)
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
