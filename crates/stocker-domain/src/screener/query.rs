//! Screen query model. SQL rendering stays in `stocker-screener`.

use serde::{Deserialize, Serialize};

use super::metrics::MetricId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FilterOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Between,
    IsNotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScreenValue {
    Number(f64),
    Range(f64, f64),
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFilter {
    pub field: MetricId,
    pub op: FilterOp,
    #[serde(default = "default_value")]
    pub value: ScreenValue,
}

fn default_value() -> ScreenValue {
    ScreenValue::None
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenQuery {
    #[serde(default)]
    pub filters: Vec<ScreenFilter>,
    #[serde(default)]
    pub sort: Option<(MetricId, SortDir)>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    100
}

/// One result row. Identity columns plus the metric values the screen filtered on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenRow {
    pub symbol: String,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub tier: Option<i64>,
    pub face_value: Option<f64>,
    pub last_refreshed_at: Option<i64>,
    pub last_refresh_status: Option<String>,
    pub last_refresh_error: Option<String>,
    pub updated_at: Option<i64>,
    /// Metric values keyed by column name (snake_case from the catalog).
    pub metrics: serde_json::Map<String, serde_json::Value>,
}
