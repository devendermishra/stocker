//! AND-only screen query compiler. All filters share a single SQL `WHERE`,
//! conditions joined by AND. No string interpolation of user input — every
//! field becomes a column via the typed [`MetricId`] enum.

use serde::{Deserialize, Serialize};
use sqlx::{Arguments, Row, SqlitePool};

use crate::error::{Error, Result};
use crate::metrics::MetricId;

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

/// One result row. Identity columns + metric values that the user filters on
/// (we always return the full snapshot — let the UI pick what to show).
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

const SYMBOL_SNAPSHOT_SELECT: &str = "s.symbol, sym.short_name, sym.sector, sym.industry, sym.exchange, \
    sym.currency, sym.country, sym.tier, sym.face_value AS symbol_face_value, sym.last_refreshed_at, \
    sym.last_refresh_status, sym.last_refresh_error, s.updated_at";

fn metrics_map_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut metrics = serde_json::Map::new();
    for id in MetricId::ALL {
        let v: Option<f64> = row.try_get(id.column())?;
        metrics.insert(
            id.column().to_string(),
            match v {
                Some(n) => serde_json::json!(n),
                None => serde_json::Value::Null,
            },
        );
    }
    Ok(metrics)
}

fn row_from_sql(row: &sqlx::sqlite::SqliteRow) -> Result<ScreenRow> {
    let metrics = metrics_map_from_row(row)?;
    let symbol: String = row.try_get("symbol")?;
    let exchange = row
        .try_get::<Option<String>, _>("exchange")?
        .map(|raw| stocker_core::india_exchange_label(&symbol, Some(raw.as_str())).to_string());
    Ok(ScreenRow {
        symbol,
        short_name: row.try_get("short_name")?,
        sector: row.try_get("sector")?,
        industry: row.try_get("industry")?,
        exchange,
        currency: row.try_get("currency")?,
        country: row.try_get("country")?,
        tier: row.try_get("tier")?,
        face_value: row.try_get("symbol_face_value")?,
        last_refreshed_at: row.try_get("last_refreshed_at")?,
        last_refresh_status: row.try_get("last_refresh_status")?,
        last_refresh_error: row.try_get("last_refresh_error")?,
        updated_at: row.try_get("updated_at")?,
        metrics,
    })
}

/// Compile + run the query, returning at most `query.limit` rows.
pub async fn run_query(pool: &SqlitePool, query: &ScreenQuery) -> Result<Vec<ScreenRow>> {
    let limit = query.limit.clamp(1, 5000) as i64;

    let mut sql = String::from("SELECT ");
    sql.push_str(SYMBOL_SNAPSHOT_SELECT);
    for id in MetricId::ALL {
        sql.push_str(", s.");
        sql.push_str(id.column());
    }
    sql.push_str(" FROM snapshots s JOIN symbols sym USING(symbol)");

    let mut args = sqlx::sqlite::SqliteArguments::default();
    let mut where_parts: Vec<String> = Vec::new();
    for filter in &query.filters {
        let col = filter.field.column();
        match (filter.op, &filter.value) {
            (FilterOp::Gt, ScreenValue::Number(v)) => {
                where_parts.push(format!("s.{col} > ?"));
                args.add(*v).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::Gte, ScreenValue::Number(v)) => {
                where_parts.push(format!("s.{col} >= ?"));
                args.add(*v).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::Lt, ScreenValue::Number(v)) => {
                where_parts.push(format!("s.{col} < ?"));
                args.add(*v).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::Lte, ScreenValue::Number(v)) => {
                where_parts.push(format!("s.{col} <= ?"));
                args.add(*v).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::Eq, ScreenValue::Number(v)) => {
                where_parts.push(format!("s.{col} = ?"));
                args.add(*v).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::Between, ScreenValue::Range(a, b)) => {
                where_parts.push(format!("s.{col} BETWEEN ? AND ?"));
                args.add(*a).map_err(|e| Error::Other(e.to_string()))?;
                args.add(*b).map_err(|e| Error::Other(e.to_string()))?;
            }
            (FilterOp::IsNotNull, _) => {
                where_parts.push(format!("s.{col} IS NOT NULL"));
            }
            _ => {
                return Err(Error::InvalidQuery(format!(
                    "filter op {:?} requires a value of the right shape",
                    filter.op
                )));
            }
        }
    }
    if !where_parts.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&where_parts.join(" AND "));
    }

    if let Some((id, dir)) = &query.sort {
        let dir_sql = match dir {
            SortDir::Asc => "ASC",
            SortDir::Desc => "DESC",
        };
        sql.push_str(&format!(" ORDER BY s.{} {} NULLS LAST", id.column(), dir_sql));
    } else {
        sql.push_str(" ORDER BY s.market_cap DESC NULLS LAST");
    }

    sql.push_str(" LIMIT ?");
    args.add(limit).map_err(|e| Error::Other(e.to_string()))?;

    let rows = sqlx::query_with(&sql, args).fetch_all(pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row_from_sql(&row)?);
    }
    Ok(out)
}

/// Load one symbol's snapshot row (all metric columns), if present.
///
/// `symbol` must already be a canonical Yahoo ticker (e.g. `RELIANCE.NS` or `SOMEBSE.BO`).
pub async fn fetch_snapshot(pool: &SqlitePool, symbol: &str) -> Result<Option<ScreenRow>> {
    let mut sql = String::from("SELECT ");
    sql.push_str(SYMBOL_SNAPSHOT_SELECT);
    for id in MetricId::ALL {
        sql.push_str(", s.");
        sql.push_str(id.column());
    }
    sql.push_str(" FROM snapshots s JOIN symbols sym USING(symbol) WHERE s.symbol = ? LIMIT 1");

    let row = sqlx::query(&sql)
        .bind(symbol)
        .fetch_optional(pool)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(row_from_sql(&row)?))
}
