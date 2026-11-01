/// Portfolio transaction symbol for a mutual fund holding.
pub fn mf_symbol(scheme_code: i64) -> String {
    format!("MF:{scheme_code}")
}

/// Parse `MF:{scheme_code}`; returns `None` for equity symbols.
pub fn parse_mf_symbol(symbol: &str) -> Option<i64> {
    symbol.strip_prefix("MF:")?.parse().ok()
}

pub fn is_mutual_fund_symbol(symbol: &str) -> bool {
    parse_mf_symbol(symbol).is_some()
}
