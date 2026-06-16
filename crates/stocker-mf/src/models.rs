use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfSearchHit {
    pub scheme_code: i64,
    pub scheme_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavSnapshot {
    pub scheme_code: i64,
    pub scheme_name: String,
    pub fund_house: Option<String>,
    pub scheme_category: Option<String>,
    pub nav: f64,
    pub nav_date: String,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavPoint {
    /// ISO date `YYYY-MM-DD`
    pub nav_date: String,
    pub nav: f64,
}

#[derive(Debug, Clone)]
pub struct SchemeMeta {
    pub scheme_code: i64,
    pub scheme_name: String,
    pub fund_house: Option<String>,
    pub scheme_category: Option<String>,
    pub isin_growth: Option<String>,
}

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
