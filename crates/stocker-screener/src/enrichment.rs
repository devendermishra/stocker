//! Map screener snapshot rows into lightweight report enrichment structs.

use stocker_core::ScreenerMetricSnapshot;

use crate::query::ScreenRow;

fn metric_f64(row: &ScreenRow, key: &str) -> Option<f64> {
    row.metrics.get(key).and_then(|v| v.as_f64())
}

/// Extract audit-relevant metrics from a screener snapshot row.
pub fn snapshot_to_enrichment(row: &ScreenRow) -> ScreenerMetricSnapshot {
    ScreenerMetricSnapshot {
        operating_cashflow_ttm: metric_f64(row, "operating_cashflow_ttm"),
        profit_after_tax_ttm: metric_f64(row, "profit_after_tax_ttm"),
        interest_coverage_ratio: metric_f64(row, "interest_coverage_ratio"),
        days_receivable_outstanding: metric_f64(row, "days_receivable_outstanding"),
        days_inventory_outstanding: metric_f64(row, "days_inventory_outstanding"),
        days_receivable_change_3y: metric_f64(row, "days_receivable_change_3y"),
        days_inventory_change_3y: metric_f64(row, "days_inventory_change_3y"),
        cumulative_cfo_pat_3y: metric_f64(row, "cumulative_cfo_pat_3y"),
        cumulative_cfo_pat_5y: metric_f64(row, "cumulative_cfo_pat_5y"),
        return_on_capital_employed: metric_f64(row, "return_on_capital_employed"),
        debt_to_equity: metric_f64(row, "debt_to_equity"),
        piotroski_f_score: metric_f64(row, "piotroski_f_score"),
        altman_z_score: metric_f64(row, "altman_z_score"),
        updated_at: row.updated_at,
    }
}

/// Returns true when `updated_at` is within the last `max_age_secs` seconds.
pub fn snapshot_is_fresh(updated_at: Option<i64>, max_age_secs: i64) -> bool {
    let Some(ts) = updated_at else {
        return false;
    };
    let now = chrono::Utc::now().timestamp();
    now - ts <= max_age_secs
}

pub const DEFAULT_SNAPSHOT_MAX_AGE_SECS: i64 = 30 * 24 * 3600;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_to_enrichment_maps_known_columns() {
        let mut metrics = serde_json::Map::new();
        metrics.insert("operating_cashflow_ttm".into(), json!(1200.0));
        metrics.insert("profit_after_tax_ttm".into(), json!(800.0));
        metrics.insert("piotroski_f_score".into(), json!(7.0));
        let row = ScreenRow {
            symbol: "TEST.NS".into(),
            short_name: None,
            sector: None,
            industry: None,
            exchange: None,
            currency: None,
            country: None,
            tier: None,
            face_value: None,
            last_refreshed_at: None,
            last_refresh_status: None,
            last_refresh_error: None,
            updated_at: Some(1_700_000_000),
            metrics,
        };
        let e = snapshot_to_enrichment(&row);
        assert_eq!(e.operating_cashflow_ttm, Some(1200.0));
        assert_eq!(e.profit_after_tax_ttm, Some(800.0));
        assert_eq!(e.piotroski_f_score, Some(7.0));
    }

    #[test]
    fn snapshot_is_fresh_within_window() {
        let now = chrono::Utc::now().timestamp();
        assert!(snapshot_is_fresh(Some(now - 3600), DEFAULT_SNAPSHOT_MAX_AGE_SECS));
        assert!(!snapshot_is_fresh(Some(now - DEFAULT_SNAPSHOT_MAX_AGE_SECS - 10), DEFAULT_SNAPSHOT_MAX_AGE_SECS));
    }
}
