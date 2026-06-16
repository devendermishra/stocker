//! Mutual fund service — scheme resolution, NAV cache, mfapi refresh.

use std::path::Path;
use std::sync::Arc;

use chrono::Local;
use sqlx::{Row, SqlitePool};

use crate::db;
use crate::error::{Error, Result};
use crate::fetcher::{latest_nav_on_or_after, MfFetcher};
use crate::models::{mf_symbol, MfSearchHit, NavPoint, NavSnapshot};
use crate::scheme_index::{
    default_scheme_list_cache_path, load_scheme_index_from_file, save_scheme_list_cache,
    SchemeIndex, SchemeListEntry,
};
use crate::trading_day::should_refresh_nav;

#[derive(Clone)]
pub struct MfService {
    pool: SqlitePool,
    fetcher: Arc<MfFetcher>,
}

impl MfService {
    pub async fn open(path: &Path) -> Result<Self> {
        let pool = db::open(path).await?;
        Ok(Self {
            pool,
            fetcher: Arc::new(MfFetcher::new()),
        })
    }

    pub async fn open_memory() -> Result<Self> {
        let pool = db::open_memory().await?;
        Ok(Self {
            pool,
            fetcher: Arc::new(MfFetcher::new()),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Load scheme index from local cache file (empty if cache missing).
    pub fn scheme_index_from_cache(&self) -> SchemeIndex {
        let path = default_scheme_list_cache_path();
        load_scheme_index_from_file(&path).unwrap_or_default()
    }

    /// Ensure the full scheme list cache file exists; fetch from mfapi if missing.
    pub async fn ensure_scheme_index_cache(&self) -> Result<SchemeIndex> {
        let path = default_scheme_list_cache_path();
        if path.is_file() {
            return load_scheme_index_from_file(&path);
        }
        self.refresh_scheme_index_cache().await?;
        load_scheme_index_from_file(&path)
    }

    /// Download all schemes from mfapi and write the cache file.
    pub async fn refresh_scheme_index_cache(&self) -> Result<Vec<SchemeListEntry>> {
        let mut all = Vec::new();
        let mut offset = 0usize;
        const PAGE: usize = 1000;
        loop {
            let page = self.fetcher.fetch_schemes_page(PAGE, offset).await?;
            let n = page.len();
            if n == 0 {
                break;
            }
            offset += n;
            all.extend(page);
            if n < PAGE {
                break;
            }
        }
        let path = default_scheme_list_cache_path();
        save_scheme_list_cache(&path, &all)?;
        log::info!("cached {} mutual fund schemes at {}", all.len(), path.display());
        Ok(all)
    }

    /// Search mfapi by fund name (for UI autocomplete).
    pub async fn search(&self, query: &str) -> Result<Vec<MfSearchHit>> {
        self.fetcher.search(query).await
    }

    /// Resolve a user-provided fund name to a scheme code.
    ///
    /// Prefers an exact case-insensitive name match. If multiple results and no
    /// exact match, returns an error asking the user to pick from search results.
    pub async fn resolve_by_name(&self, name: &str) -> Result<i64> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(Error::InvalidInput("fund name is required".into()));
        }

        // Already resolved symbol?
        if let Some(code) = crate::models::parse_mf_symbol(trimmed) {
            self.ensure_scheme(code).await?;
            return Ok(code);
        }

        let hits = self.fetcher.search(trimmed).await?;
        if hits.is_empty() {
            return Err(Error::NotFound(format!(
                "no mutual fund found matching \"{trimmed}\""
            )));
        }

        let lower = trimmed.to_ascii_lowercase();
        let exact: Vec<_> = hits
            .iter()
            .filter(|h| h.scheme_name.to_ascii_lowercase() == lower)
            .collect();

        let chosen = if exact.len() == 1 {
            exact[0].clone()
        } else if hits.len() == 1 {
            hits.into_iter().next().unwrap()
        } else {
            return Err(Error::InvalidInput(format!(
                "multiple funds match \"{trimmed}\"; pick one from search results"
            )));
        };

        self.upsert_scheme_from_api(chosen.scheme_code).await?;
        Ok(chosen.scheme_code)
    }

    /// Ensure scheme metadata exists in DB (fetch from mfapi if missing).
    pub async fn ensure_scheme(&self, scheme_code: i64) -> Result<()> {
        let exists: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM mf_schemes WHERE scheme_code = ?",
        )
        .bind(scheme_code)
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_none() {
            self.upsert_scheme_from_api(scheme_code).await?;
        }
        Ok(())
    }

    /// Latest NAV for a scheme, refreshing from mfapi when stale or missing.
    pub async fn latest_nav(&self, scheme_code: i64) -> Result<NavSnapshot> {
        let now = chrono::Utc::now().timestamp();
        let today = Local::now().date_naive();

        let cached = self.load_cached_nav(scheme_code).await?;

        if should_refresh_nav(cached.as_ref().map(|c| c.fetched_at), now, today) {
            self.refresh_nav(scheme_code, now).await
        } else {
            cached.ok_or_else(|| Error::NotFound(format!("NAV missing for {scheme_code}")))
        }
    }

    /// Latest NAV on or after `sip_date` using mfapi date-range query.
    pub async fn nav_on_or_after(
        &self,
        scheme_code: i64,
        sip_date: &str,
    ) -> Result<NavPoint> {
        let end_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
        let points = self
            .fetcher
            .fetch_nav_range(scheme_code, sip_date, &end_date)
            .await?;
        latest_nav_on_or_after(&points, sip_date)
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "no NAV published on or after {sip_date} for scheme {scheme_code}"
                ))
            })
    }

    async fn load_cached_nav(&self, scheme_code: i64) -> Result<Option<NavSnapshot>> {
        let row = sqlx::query(
            r#"
            SELECT s.scheme_code, s.scheme_name, s.fund_house, s.scheme_category,
                   n.nav, n.nav_date, n.fetched_at
            FROM mf_schemes s
            JOIN mf_nav n ON n.scheme_code = s.scheme_code
            WHERE s.scheme_code = ?
            "#,
        )
        .bind(scheme_code)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| NavSnapshot {
            scheme_code: r.try_get("scheme_code").unwrap_or(scheme_code),
            scheme_name: r.try_get("scheme_name").unwrap_or_default(),
            fund_house: r.try_get("fund_house").ok(),
            scheme_category: r.try_get("scheme_category").ok(),
            nav: r.try_get("nav").unwrap_or(0.0),
            nav_date: r.try_get("nav_date").unwrap_or_default(),
            fetched_at: r.try_get("fetched_at").unwrap_or(0),
        }))
    }

    async fn refresh_nav(&self, scheme_code: i64, now: i64) -> Result<NavSnapshot> {
        let (meta, nav, nav_date) = self.fetcher.fetch_latest(scheme_code).await?;
        self.persist_scheme_and_nav(&meta, nav, &nav_date, now).await?;
        Ok(NavSnapshot {
            scheme_code: meta.scheme_code,
            scheme_name: meta.scheme_name,
            fund_house: meta.fund_house,
            scheme_category: meta.scheme_category,
            nav,
            nav_date,
            fetched_at: now,
        })
    }

    async fn upsert_scheme_from_api(&self, scheme_code: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let (meta, nav, nav_date) = self.fetcher.fetch_latest(scheme_code).await?;
        self.persist_scheme_and_nav(&meta, nav, &nav_date, now).await
    }

    async fn persist_scheme_and_nav(
        &self,
        meta: &crate::models::SchemeMeta,
        nav: f64,
        nav_date: &str,
        fetched_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO mf_schemes (scheme_code, scheme_name, fund_house, scheme_category, isin_growth, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(scheme_code) DO UPDATE SET
                scheme_name = excluded.scheme_name,
                fund_house = excluded.fund_house,
                scheme_category = excluded.scheme_category,
                isin_growth = excluded.isin_growth
            "#,
        )
        .bind(meta.scheme_code)
        .bind(&meta.scheme_name)
        .bind(&meta.fund_house)
        .bind(&meta.scheme_category)
        .bind(&meta.isin_growth)
        .bind(fetched_at)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO mf_nav (scheme_code, nav, nav_date, fetched_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(scheme_code) DO UPDATE SET
                nav = excluded.nav,
                nav_date = excluded.nav_date,
                fetched_at = excluded.fetched_at
            "#,
        )
        .bind(meta.scheme_code)
        .bind(nav)
        .bind(nav_date)
        .bind(fetched_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Resolve fund name to portfolio symbol `MF:{code}`.
pub async fn resolve_mf_symbol(svc: &MfService, name: &str) -> Result<String> {
    let code = svc.resolve_by_name(name).await?;
    Ok(mf_symbol(code))
}

#[cfg(test)]
mod tests {
    use crate::models::{is_mutual_fund_symbol, mf_symbol, parse_mf_symbol};

    #[test]
    fn mf_symbol_roundtrip() {
        assert_eq!(mf_symbol(122639), "MF:122639");
        assert_eq!(parse_mf_symbol("MF:122639"), Some(122639));
        assert!(!is_mutual_fund_symbol("RELIANCE.NS"));
    }
}
