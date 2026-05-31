//! StockSnapshot row + upsert helpers.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::{Arguments, Row, SqlitePool};

use crate::error::{Error, Result};
use crate::metrics::MetricId;

/// One symbol's worth of metric values + metadata.
#[derive(Debug, Clone)]
pub struct StockSnapshot {
    pub symbol: String,
    pub metrics: HashMap<MetricId, Option<f64>>,
}

impl StockSnapshot {
    pub fn new(symbol: impl Into<String>, metrics: HashMap<MetricId, Option<f64>>) -> Self {
        Self {
            symbol: symbol.into(),
            metrics,
        }
    }

    /// `INSERT OR REPLACE INTO snapshots (...)` covering every catalog column.
    /// Built dynamically from `MetricId::ALL` so a new metric only needs to be
    /// added in one place (`metrics.rs`).
    pub async fn upsert(&self, pool: &SqlitePool) -> Result<()> {
        let now = Utc::now().timestamp();

        let mut col_list = String::from("symbol");
        let mut placeholders = String::from("?");
        for id in MetricId::ALL {
            col_list.push_str(", ");
            col_list.push_str(id.column());
            placeholders.push_str(", ?");
        }
        col_list.push_str(", updated_at");
        placeholders.push_str(", ?");

        let sql = format!(
            "INSERT OR REPLACE INTO snapshots ({}) VALUES ({})",
            col_list, placeholders
        );

        let mut args = sqlx::sqlite::SqliteArguments::default();
        args.add(&self.symbol).map_err(|e| Error::Other(e.to_string()))?;
        for id in MetricId::ALL {
            args.add(self.metrics.get(id).copied().flatten())
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        args.add(now).map_err(|e| Error::Other(e.to_string()))?;

        sqlx::query_with(&sql, args).execute(pool).await?;
        Ok(())
    }
}

/// Identity row in the `symbols` table, written when discovery finds a new symbol
/// and updated as the refresh job processes it.
#[derive(Debug, Clone, Default)]
pub struct SymbolRow {
    pub symbol: String,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    /// Par / face value from NSE EQUITY_L.csv.
    pub face_value: Option<f64>,
    /// 0 = NIFTY 500, 1 = rest.
    pub tier: i64,
    pub last_refreshed_at: Option<i64>,
    pub last_refresh_status: Option<String>,
    pub last_refresh_error: Option<String>,
}

impl SymbolRow {
    /// Insert if missing; tier and identity-fields are updated when re-supplied.
    pub async fn upsert_identity(&self, pool: &SqlitePool) -> Result<()> {
        let sql = r#"
            INSERT INTO symbols (symbol, short_name, sector, industry, exchange, currency, country, tier, face_value)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(symbol) DO UPDATE SET
                short_name = COALESCE(excluded.short_name, short_name),
                sector     = COALESCE(excluded.sector, sector),
                industry   = COALESCE(excluded.industry, industry),
                exchange   = COALESCE(excluded.exchange, exchange),
                currency   = COALESCE(excluded.currency, currency),
                country    = COALESCE(excluded.country, country),
                tier       = excluded.tier,
                face_value = COALESCE(excluded.face_value, face_value)
        "#;
        sqlx::query(sql)
            .bind(&self.symbol)
            .bind(&self.short_name)
            .bind(&self.sector)
            .bind(&self.industry)
            .bind(&self.exchange)
            .bind(&self.currency)
            .bind(&self.country)
            .bind(self.tier)
            .bind(self.face_value)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Face value per share from the `symbols` table (NSE CSV).
    pub async fn face_value_for(pool: &SqlitePool, symbol: &str) -> Result<Option<f64>> {
        let row = sqlx::query("SELECT face_value FROM symbols WHERE symbol = ?")
            .bind(symbol)
            .fetch_optional(pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let v: Option<f64> = row.try_get("face_value")?;
        Ok(v.filter(|x| *x > 0.0))
    }

    /// Mark a symbol's refresh result.
    pub async fn mark_refreshed(
        pool: &SqlitePool,
        symbol: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE symbols SET last_refreshed_at = ?, last_refresh_status = ?, last_refresh_error = ? WHERE symbol = ?",
        )
        .bind(now)
        .bind(status)
        .bind(error)
        .bind(symbol)
        .execute(pool)
        .await?;
        Ok(())
    }
}

/// Total number of registered symbols in the universe.
pub async fn count_symbols(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS c FROM symbols")
        .fetch_one(pool)
        .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Number of symbols currently lacking a snapshot (or older than the tier interval).
pub async fn count_pending(pool: &SqlitePool, tier0_interval_secs: i64, tier1_interval_secs: i64) -> Result<i64> {
    let now = Utc::now().timestamp();
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) AS c FROM symbols
        WHERE last_refreshed_at IS NULL
           OR (tier = 0 AND last_refreshed_at < ? - ?)
           OR (tier <> 0 AND last_refreshed_at < ? - ?)
        "#,
    )
    .bind(now)
    .bind(tier0_interval_secs)
    .bind(now)
    .bind(tier1_interval_secs)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get::<i64, _>("c")?)
}

/// Fetch the next symbol due for refresh, oldest-first within tier-0 then tier-1.
pub async fn next_due_symbol(
    pool: &SqlitePool,
    tier0_interval_secs: i64,
    tier1_interval_secs: i64,
) -> Result<Option<SymbolRow>> {
    let now = Utc::now().timestamp();
    let row_opt = sqlx::query(
        r#"
        SELECT symbol, short_name, sector, industry, exchange, currency, country, tier, face_value,
               last_refreshed_at, last_refresh_status, last_refresh_error
        FROM symbols
        WHERE last_refreshed_at IS NULL
           OR (tier = 0 AND last_refreshed_at < ? - ?)
           OR (tier <> 0 AND last_refreshed_at < ? - ?)
        ORDER BY tier ASC, COALESCE(last_refreshed_at, 0) ASC
        LIMIT 1
        "#,
    )
    .bind(now)
    .bind(tier0_interval_secs)
    .bind(now)
    .bind(tier1_interval_secs)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row_opt else {
        return Ok(None);
    };
    Ok(Some(SymbolRow {
        symbol: row.try_get("symbol")?,
        short_name: row.try_get("short_name")?,
        sector: row.try_get("sector")?,
        industry: row.try_get("industry")?,
        exchange: row.try_get("exchange")?,
        currency: row.try_get("currency")?,
        country: row.try_get("country")?,
        face_value: row.try_get("face_value")?,
        tier: row.try_get("tier")?,
        last_refreshed_at: row.try_get("last_refreshed_at")?,
        last_refresh_status: row.try_get("last_refresh_status")?,
        last_refresh_error: row.try_get("last_refresh_error")?,
    }))
}
