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
    /// ISIN from exchange CSV when available (links NSE/BSE listings).
    pub isin: Option<String>,
}

impl SymbolRow {
    /// Insert if missing; tier and identity-fields are updated when re-supplied.
    pub async fn upsert_identity(&self, pool: &SqlitePool) -> Result<()> {
        // Ticker suffix is authoritative — ignore Yahoo codes (NSI/YHD) on .NS/.BO symbols.
        let yahoo_hint = if self.symbol.ends_with(".NS") || self.symbol.ends_with(".BO") {
            None
        } else {
            self.exchange.as_deref()
        };
        let exchange =
            stocker_core::india_exchange_label(&self.symbol, yahoo_hint).to_string();
        let sql = r#"
            INSERT INTO symbols (symbol, short_name, sector, industry, exchange, currency, country, tier, face_value, isin)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(symbol) DO UPDATE SET
                short_name = COALESCE(excluded.short_name, short_name),
                sector     = COALESCE(excluded.sector, sector),
                industry   = COALESCE(excluded.industry, industry),
                exchange   = excluded.exchange,
                currency   = COALESCE(excluded.currency, currency),
                country    = COALESCE(excluded.country, country),
                tier       = excluded.tier,
                face_value = COALESCE(excluded.face_value, face_value),
                isin       = COALESCE(excluded.isin, isin)
        "#;
        sqlx::query(sql)
            .bind(&self.symbol)
            .bind(&self.short_name)
            .bind(&self.sector)
            .bind(&self.industry)
            .bind(&exchange)
            .bind(&self.currency)
            .bind(&self.country)
            .bind(self.tier)
            .bind(self.face_value)
            .bind(&self.isin)
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

/// Delete BSE rows when an NSE listing exists for the same ISIN, ticker base, or name.
pub async fn remove_dual_listed_bse_symbols(pool: &SqlitePool) -> Result<u64> {
    use std::collections::HashSet;

    use crate::symbols::normalize_company_name;

    let nse_rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT symbol, short_name, isin FROM symbols WHERE symbol LIKE '%.NS'",
    )
    .fetch_all(pool)
    .await?;

    let mut nse_isins = HashSet::new();
    let mut nse_bases = HashSet::new();
    let mut nse_names = HashSet::new();
    for (sym, name, isin) in &nse_rows {
        if let Some(isin) = isin.as_ref().map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()) {
            nse_isins.insert(isin);
        }
        if let Some(base) = stocker_core::india_base_symbol(sym).ok() {
            nse_bases.insert(base.to_uppercase());
        }
        if let Some(name) = name {
            let n = normalize_company_name(name);
            if !n.is_empty() {
                nse_names.insert(n);
            }
        }
    }

    let bse_rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT symbol, short_name, isin FROM symbols WHERE symbol LIKE '%.BO'",
    )
    .fetch_all(pool)
    .await?;

    let mut removed = 0u64;
    for (sym, name, isin) in bse_rows {
        let drop = isin
            .as_ref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .is_some_and(|i| nse_isins.contains(&i))
            || stocker_core::india_base_symbol(&sym)
                .ok()
                .is_some_and(|b| nse_bases.contains(&b.to_uppercase()))
            || name.as_ref().is_some_and(|n| {
                let key = normalize_company_name(n);
                !key.is_empty() && nse_names.contains(&key)
            });
        if drop {
            sqlx::query("DELETE FROM symbols WHERE symbol = ?")
                .bind(&sym)
                .execute(pool)
                .await?;
            removed += 1;
        }
    }
    Ok(removed)
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
        SELECT symbol, short_name, sector, industry, exchange, currency, country, tier, face_value, isin,
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
        isin: row.try_get("isin").ok(),
        tier: row.try_get("tier")?,
        last_refreshed_at: row.try_get("last_refreshed_at")?,
        last_refresh_status: row.try_get("last_refresh_status")?,
        last_refresh_error: row.try_get("last_refresh_error")?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn upsert_identity_stores_nse_and_bse_from_ticker_suffix() {
        let pool = db::open_memory().await.unwrap();

        SymbolRow {
            symbol: "RELIANCE.NS".to_string(),
            short_name: Some("Reliance Industries Ltd".to_string()),
            exchange: Some("NSE".to_string()),
            tier: 1,
            ..Default::default()
        }
        .upsert_identity(&pool)
        .await
        .unwrap();
        SymbolRow {
            symbol: "SOMEBSE.BO".to_string(),
            short_name: Some("Some BSE Co Ltd".to_string()),
            exchange: Some("BSE".to_string()),
            tier: 1,
            ..Default::default()
        }
        .upsert_identity(&pool)
        .await
        .unwrap();

        let nse: String =
            sqlx::query_scalar("SELECT exchange FROM symbols WHERE symbol = 'RELIANCE.NS'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let bse: String =
            sqlx::query_scalar("SELECT exchange FROM symbols WHERE symbol = 'SOMEBSE.BO'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(nse, "NSE");
        assert_eq!(bse, "BSE");
    }

    #[tokio::test]
    async fn upsert_identity_replaces_yahoo_exchange_codes() {
        let pool = db::open_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO symbols (symbol, short_name, exchange, tier) VALUES ('FOO.BO', 'Foo Ltd', 'YHD', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        SymbolRow {
            symbol: "FOO.BO".to_string(),
            tier: 1,
            exchange: Some("YHD".to_string()),
            ..Default::default()
        }
        .upsert_identity(&pool)
        .await
        .unwrap();

        let ex: String = sqlx::query_scalar("SELECT exchange FROM symbols WHERE symbol = 'FOO.BO'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ex, "BSE");
    }
}
