use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::models::BankingMetrics;

/// Environment variable pointing to a local CSV containing bank metrics keyed by symbol.
///
/// This keeps Stocker Yahoo-only for live data while still allowing professional bank analysis
/// using audited filing-derived numbers the user provides.
pub const ENV_BANK_METRICS_CSV: &str = "STOCKER_BANK_METRICS_CSV";

#[derive(Debug, Deserialize, Clone)]
struct BankMetricsCsvRow {
    pub symbol: String,
    #[serde(default)]
    pub gnpa_pct: Option<f64>,
    #[serde(default)]
    pub nnpa_pct: Option<f64>,
    #[serde(default)]
    pub provision_coverage_ratio_pct: Option<f64>,
    #[serde(default)]
    pub credit_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub deposit_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub casa_ratio_pct: Option<f64>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

fn load_bank_metrics_map() -> Option<HashMap<String, BankingMetrics>> {
    let path = std::env::var(ENV_BANK_METRICS_CSV).ok()?;
    let path = PathBuf::from(path);
    let mut rdr = csv::Reader::from_path(&path).ok()?;
    let mut out = HashMap::new();
    for row in rdr.deserialize::<BankMetricsCsvRow>().flatten() {
        let ctx = crate::default_india_symbol_context();
        let sym = crate::resolve_india_symbol(&row.symbol, &ctx)
            .ok()
            .unwrap_or(row.symbol);
        out.insert(
            sym,
            BankingMetrics {
                gnpa_pct: row.gnpa_pct,
                nnpa_pct: row.nnpa_pct,
                provision_coverage_ratio_pct: row.provision_coverage_ratio_pct,
                credit_growth_yoy_pct: row.credit_growth_yoy_pct,
                deposit_growth_yoy_pct: row.deposit_growth_yoy_pct,
                casa_ratio_pct: row.casa_ratio_pct,
                as_of_date: row.date,
                source: row.source,
            },
        );
    }
    Some(out)
}

static BANK_METRICS: OnceLock<Option<HashMap<String, BankingMetrics>>> = OnceLock::new();

/// Load bank metrics for a symbol from the local CSV (if configured).
pub fn bank_metrics_for(symbol: &str) -> Option<BankingMetrics> {
    let map = BANK_METRICS.get_or_init(load_bank_metrics_map);
    map.as_ref()
        .as_ref()
        .and_then(|m| m.get(symbol).cloned())
}

