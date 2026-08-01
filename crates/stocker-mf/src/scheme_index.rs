//! Local cache of the full mfapi scheme list for name/ISIN lookup (import, offline).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::models::mf_symbol;

const CACHE_FILE_NAME: &str = "mf_schemes_list.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemeListEntry {
    #[serde(rename = "schemeCode")]
    pub scheme_code: i64,
    #[serde(rename = "schemeName")]
    pub scheme_name: String,
    #[serde(rename = "isinGrowth", default)]
    pub isin_growth: Option<String>,
    #[serde(rename = "isinDivReinvestment", default)]
    pub isin_div_reinvestment: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SchemeIndex {
    name_to_code: HashMap<String, i64>,
    isin_to_code: HashMap<String, i64>,
}

impl SchemeIndex {
    pub fn from_entries(entries: Vec<SchemeListEntry>) -> Self {
        let mut name_to_code = HashMap::new();
        let mut isin_to_code = HashMap::new();
        for entry in entries {
            let key = normalize_mf_name_key(&entry.scheme_name);
            name_to_code.entry(key).or_insert(entry.scheme_code);
            if let Some(isin) = normalize_isin(entry.isin_growth.as_deref()) {
                isin_to_code.entry(isin).or_insert(entry.scheme_code);
            }
            if let Some(isin) = normalize_isin(entry.isin_div_reinvestment.as_deref()) {
                isin_to_code.entry(isin).or_insert(entry.scheme_code);
            }
        }
        Self {
            name_to_code,
            isin_to_code,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name_to_code.is_empty() && self.isin_to_code.is_empty()
    }

    pub fn lookup_name(&self, name: &str) -> Option<i64> {
        self.name_to_code.get(&normalize_mf_name_key(name)).copied()
    }

    pub fn lookup_isin(&self, isin: &str) -> Option<i64> {
        normalize_isin(Some(isin)).and_then(|k| self.isin_to_code.get(&k).copied())
    }

    pub fn lookup_symbol(&self, raw: &str) -> Option<i64> {
        let trimmed = raw.trim();
        if let Some(code) = crate::models::parse_mf_symbol(trimmed) {
            return Some(code);
        }
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return trimmed.parse().ok();
        }
        self.lookup_name(trimmed)
    }

    pub fn resolve_symbol(&self, symbol: Option<&str>, name: Option<&str>, isin: Option<&str>) -> Option<String> {
        if let Some(isin) = isin {
            if let Some(code) = self.lookup_isin(isin) {
                return Some(mf_symbol(code));
            }
        }
        if let Some(sym) = symbol.filter(|s| !s.trim().is_empty()) {
            if let Some(code) = self.lookup_symbol(sym) {
                return Some(mf_symbol(code));
            }
        }
        if let Some(name) = name.filter(|s| !s.trim().is_empty()) {
            if let Some(code) = self.lookup_name(name) {
                return Some(mf_symbol(code));
            }
        }
        None
    }
}

/// Default path for the cached `/mf` scheme list JSON.
pub fn default_scheme_list_cache_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("STOCKER_MF_SCHEMES_CACHE_PATH") {
        return PathBuf::from(env_path);
    }
    if let Ok(mf_db) = std::env::var("STOCKER_MF_DB_PATH") {
        let p = PathBuf::from(mf_db);
        if let Some(parent) = p.parent() {
            return parent.join(CACHE_FILE_NAME);
        }
    }
    PathBuf::from(CACHE_FILE_NAME)
}

pub fn load_scheme_index_from_file(path: &Path) -> Result<SchemeIndex> {
    let bytes = std::fs::read(path)
        .map_err(|e| Error::Other(format!("read scheme list cache {}: {e}", path.display())))?;
    let entries: Vec<SchemeListEntry> = serde_json::from_slice(&bytes)?;
    Ok(SchemeIndex::from_entries(entries))
}

pub fn save_scheme_list_cache(path: &Path, entries: &[SchemeListEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("create cache parent dir: {e}")))?;
        }
    }
    let json = serde_json::to_vec_pretty(entries)?;
    std::fs::write(path, json)
        .map_err(|e| Error::Other(format!("write scheme list cache {}: {e}", path.display())))?;
    Ok(())
}

pub fn normalize_mf_name_key(name: &str) -> String {
    let stripped = strip_yahoo_exchange_suffix(name);
    stripped
        .trim()
        .to_lowercase()
        .replace(" ltd.", " limited")
        .replace(" ltd", " limited")
        .replace('.', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip a trailing Yahoo `.NS` / `.BO` suffix (often wrongly appended to fund names).
pub fn strip_yahoo_exchange_suffix(s: &str) -> &str {
    let t = s.trim();
    if t.len() > 3 {
        let suffix = &t[t.len() - 3..];
        if suffix.eq_ignore_ascii_case(".NS") || suffix.eq_ignore_ascii_case(".BO") {
            return t[..t.len() - 3].trim_end();
        }
    }
    t
}

fn normalize_isin(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim().to_uppercase();
    if s.len() == 12 && s.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(s)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<SchemeListEntry> {
        vec![
            SchemeListEntry {
                scheme_code: 141957,
                scheme_name: "BHARAT 22 ETF".into(),
                isin_growth: Some("INF109KB15Y7".into()),
                isin_div_reinvestment: None,
            },
            SchemeListEntry {
                scheme_code: 122639,
                scheme_name: "Parag Parikh Flexi Cap Fund - Direct Plan - Growth".into(),
                isin_growth: Some("INF879O01027".into()),
                isin_div_reinvestment: None,
            },
        ]
    }

    #[test]
    fn lookup_bharat_22_by_name_and_isin() {
        let idx = SchemeIndex::from_entries(sample_entries());
        assert_eq!(idx.lookup_name("Bharat 22 ETF"), Some(141957));
        assert_eq!(idx.lookup_isin("INF109KB15Y7"), Some(141957));
        assert_eq!(
            idx.lookup_symbol("141957"),
            Some(141957)
        );
    }

    #[test]
    fn lookup_ignores_spurious_yahoo_suffix() {
        let idx = SchemeIndex::from_entries(sample_entries());
        assert_eq!(
            idx.lookup_name("Parag Parikh Flexi Cap Fund - Direct Plan - Growth.BO"),
            Some(122639)
        );
        assert_eq!(
            idx.lookup_symbol("Parag Parikh Flexi Cap Fund - Direct Plan - Growth.BO"),
            Some(122639)
        );
    }

    #[test]
    fn resolve_symbol_returns_mf_prefix() {
        let idx = SchemeIndex::from_entries(sample_entries());
        assert_eq!(
            idx.resolve_symbol(None, Some("BHARAT 22 ETF"), None),
            Some("MF:141957".into())
        );
    }
}
