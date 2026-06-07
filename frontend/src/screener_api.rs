//! Screener API client. Mirrors the `web` / `desktop` split used for the
//! research-report calls in [`crate::api`].

use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use crate::api::API_BASE;

/// Catalog entry as returned by `/api/v1/screener/fields`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub category_label: String,
    pub unit: String,
    pub column: String,
    pub needs_review: bool,
    pub source_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogResponse {
    pub fields: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub metrics: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub rows: Vec<ScreenRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenerStatus {
    pub running: bool,
    #[serde(default)]
    pub backfill_running: bool,
    pub universe_size: i64,
    pub pending_count: i64,
    pub last_universe_sync_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageTier {
    Full,
    Partial,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub full: usize,
    pub partial: usize,
    pub empty: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricCoverage {
    pub id: String,
    pub label: String,
    pub description: String,
    pub category_label: String,
    pub column: String,
    pub source_kind: String,
    pub needs_review: bool,
    pub filled: i64,
    pub fill_pct: f64,
    pub tier: CoverageTier,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub snapshot_count: i64,
    pub summary: CoverageSummary,
    pub metrics: Vec<MetricCoverage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedScreen {
    pub id: i64,
    pub name: String,
    pub filters: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedScreensResponse {
    pub screens: Vec<SavedScreen>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolListing {
    pub symbol: String,
    pub id: String,
    pub short_name: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolPair {
    pub symbol: String,
    pub id: String,
    pub nse_id: Option<String>,
    pub bse_id: Option<String>,
    pub short_name: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResponse {
    pub rows: Vec<SymbolListing>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPairResponse {
    pub listing: SymbolPair,
}

// =================== WEB BACKEND ===================

#[cfg(feature = "web")]
pub async fn list_fields() -> Result<Vec<CatalogEntry>, String> {
    let url = format!("{}/api/v1/screener/fields", API_BASE);
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    let parsed: CatalogResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.fields)
}

#[cfg(feature = "web")]
pub async fn run_search(query: serde_json::Value) -> Result<Vec<ScreenRow>, String> {
    let url = format!("{}/api/v1/screener/search", API_BASE);
    let res = gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(query.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}: {}", status, text));
    }
    let parsed: SearchResponse =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(parsed.rows)
}

#[cfg(feature = "web")]
pub async fn status() -> Result<ScreenerStatus, String> {
    let url = format!("{}/api/v1/screener/status", API_BASE);
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    res.json::<ScreenerStatus>().await.map_err(|e| e.to_string())
}

#[cfg(feature = "web")]
pub async fn coverage() -> Result<CoverageReport, String> {
    let url = format!("{}/api/v1/screener/coverage", API_BASE);
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    res.json::<CoverageReport>().await.map_err(|e| e.to_string())
}

#[cfg(feature = "web")]
pub async fn list_screens() -> Result<Vec<SavedScreen>, String> {
    let url = format!("{}/api/v1/screener/screens", API_BASE);
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    let parsed: SavedScreensResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.screens)
}

#[cfg(feature = "web")]
pub async fn create_screen(name: String, filters: serde_json::Value) -> Result<SavedScreen, String> {
    let url = format!("{}/api/v1/screener/screens", API_BASE);
    let body = serde_json::json!({ "name": name, "filters": filters });
    let res = gloo_net::http::Request::post(&url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    res.json::<SavedScreen>().await.map_err(|e| e.to_string())
}

#[cfg(feature = "web")]
pub async fn load_snapshot(symbol: &str) -> Result<Option<ScreenRow>, String> {
    let url = format!(
        "{}/api/v1/screener/snapshot/{}",
        API_BASE,
        urlencoding::encode(symbol)
    );
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status() == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    res.json::<ScreenRow>().await.map_err(|e| e.to_string()).map(Some)
}

#[cfg(feature = "web")]
pub async fn refresh_symbol(symbol: &str) -> Result<(), String> {
    let url = format!(
        "{}/api/v1/screener/refresh/{}",
        API_BASE,
        urlencoding::encode(symbol)
    );
    let res = gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", res.status(), text));
    }
    Ok(())
}

#[cfg(feature = "web")]
pub async fn delete_screen(id: i64) -> Result<(), String> {
    let url = format!("{}/api/v1/screener/screens/{}", API_BASE, id);
    let res = gloo_net::http::Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct BackfillResponse {
    ok: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: String,
}

#[cfg(feature = "web")]
pub async fn start_backfill() -> Result<(), String> {
    let url = format!("{}/api/v1/screener/backfill", API_BASE);
    let res = gloo_net::http::Request::post(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if status == 409 {
        let parsed: ApiErrorBody =
            serde_json::from_str(&text).unwrap_or(ApiErrorBody {
                error: "Stock data refresh is already running".to_string(),
            });
        return Err(parsed.error);
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}: {}", status, text));
    }
    let _: BackfillResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(feature = "web")]
pub async fn search_symbols(
    search: &str,
    exchange: Option<&str>,
    limit: i64,
) -> Result<Vec<SymbolListing>, String> {
    let mut url = format!(
        "{}/api/v1/screener/symbols?limit={}",
        API_BASE,
        limit
    );
    if !search.is_empty() {
        url.push_str("&search=");
        url.push_str(&urlencoding::encode(search));
    }
    if let Some(ex) = exchange.filter(|e| !e.is_empty() && !e.eq_ignore_ascii_case("all")) {
        url.push_str("&exchange=");
        url.push_str(&urlencoding::encode(ex));
    }
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    let parsed: SymbolSearchResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.rows)
}

#[cfg(feature = "web")]
pub async fn symbol_pair(symbol: &str) -> Result<Option<SymbolPair>, String> {
    let url = format!(
        "{}/api/v1/screener/symbols/{}/pair",
        API_BASE,
        urlencoding::encode(symbol)
    );
    let res = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status() == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&res.status()) {
        return Err(format!("HTTP {}", res.status()));
    }
    let parsed: SymbolPairResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(Some(parsed.listing))
}

#[cfg(feature = "web")]
pub async fn symbol_pair_from_id(id: &str, exchange: &str) -> Result<Option<SymbolPair>, String> {
    let id = id.trim();
    if id.is_empty() {
        return Ok(None);
    }
    let yahoo = if exchange.eq_ignore_ascii_case("BSE") {
        format!("{}.BO", id.to_uppercase())
    } else {
        format!("{}.NS", id.to_uppercase())
    };
    symbol_pair(&yahoo).await
}

/// Build a Yahoo ticker from id + exchange, using pair lookup when available.
#[cfg(feature = "web")]
pub async fn resolve_report_ticker(id: &str, exchange: &str) -> String {
    let id = id.trim();
    if id.is_empty() {
        return String::new();
    }
    let ex = exchange.trim().to_uppercase();
    if let Ok(Some(pair)) = symbol_pair(id).await {
        if ex == "BSE" {
            if let Some(bse) = pair.bse_id {
                return format!("{bse}.BO");
            }
        } else if let Some(nse) = pair.nse_id {
            return format!("{nse}.NS");
        }
    }
    if ex == "BSE" {
        format!("{}.BO", id.to_uppercase())
    } else {
        format!("{}.NS", id.to_uppercase())
    }
}

pub fn parse_base_id(symbol: &str) -> String {
    let s = symbol.trim();
    if s.ends_with(".NS") || s.ends_with(".BO") {
        s.rsplit_once('.').map(|(base, _)| base.to_string()).unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}

pub fn parse_exchange(symbol: &str) -> String {
    if symbol.trim().ends_with(".BO") {
        "BSE".to_string()
    } else {
        "NSE".to_string()
    }
}

// =================== DESKTOP BACKEND ===================
//
// Direct in-process calls to `stocker_screener::ScreenerService`. The service is
// initialised lazily on first access so app launch isn't blocked.

#[cfg(feature = "desktop")]
mod desktop_backend {
    use std::sync::Arc;

    use stocker_screener::{NewSavedScreen, RefreshConfig, ScreenerService};
    use tokio::sync::OnceCell;

    use super::{
        CatalogEntry, CoverageReport, CoverageSummary, CoverageTier, MetricCoverage, SavedScreen,
        ScreenRow, ScreenerStatus, SymbolListing, SymbolPair,
    };

    static SERVICE: OnceCell<Arc<ScreenerService>> = OnceCell::const_new();

    pub(super) async fn service() -> Result<Arc<ScreenerService>, String> {
        SERVICE
            .get_or_try_init(|| async {
                let path = stocker_screener::db::default_db_path();
                eprintln!("Opening screener DB at {}", path.display());
                let svc = ScreenerService::open(&path, RefreshConfig::from_env())
                    .await
                    .map_err(|e| e.to_string())?;
                svc.start();
                Ok::<Arc<ScreenerService>, String>(Arc::new(svc))
            })
            .await
            .cloned()
    }

    pub async fn list_fields() -> Result<Vec<CatalogEntry>, String> {
        let svc = service().await?;
        Ok(svc
            .catalog()
            .iter()
            .map(|s| CatalogEntry {
                id: serde_json::to_value(&s.id)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default(),
                label: s.label.to_string(),
                description: s.description.to_string(),
                category: format!("{:?}", s.category),
                category_label: s.category.label().to_string(),
                unit: format!("{:?}", s.unit),
                column: s.column.to_string(),
                needs_review: s.needs_review,
                source_kind: format!("{:?}", s.source_kind),
            })
            .collect())
    }

    pub async fn run_search(query: serde_json::Value) -> Result<Vec<ScreenRow>, String> {
        let q: stocker_screener::ScreenQuery =
            serde_json::from_value(query).map_err(|e| e.to_string())?;
        let svc = service().await?;
        let rows = svc.run_query(&q).await.map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| ScreenRow {
                symbol: r.symbol,
                short_name: r.short_name,
                sector: r.sector,
                industry: r.industry,
                exchange: r.exchange,
                currency: r.currency,
                country: r.country,
                tier: r.tier,
                face_value: r.face_value,
                last_refreshed_at: r.last_refreshed_at,
                last_refresh_status: r.last_refresh_status,
                last_refresh_error: r.last_refresh_error,
                updated_at: r.updated_at,
                metrics: r.metrics,
            })
            .collect())
    }

    pub async fn status() -> Result<ScreenerStatus, String> {
        let svc = service().await?;
        let s = svc.status().await.map_err(|e| e.to_string())?;
        Ok(ScreenerStatus {
            running: s.running,
            backfill_running: s.backfill_running,
            universe_size: s.universe_size,
            pending_count: s.pending_count,
            last_universe_sync_at: s.last_universe_sync_at,
        })
    }

    pub async fn coverage() -> Result<CoverageReport, String> {
        let svc = service().await?;
        let r = svc.coverage().await.map_err(|e| e.to_string())?;
        Ok(CoverageReport {
            snapshot_count: r.snapshot_count,
            summary: CoverageSummary {
                full: r.summary.full,
                partial: r.summary.partial,
                empty: r.summary.empty,
            },
            metrics: r
                .metrics
                .into_iter()
                .map(|m| MetricCoverage {
                    id: serde_json::to_value(&m.id)
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_default(),
                    label: m.label.to_string(),
                    description: m.description.to_string(),
                    category_label: m.category_label.to_string(),
                    column: m.column.to_string(),
                    source_kind: format!("{:?}", m.source_kind),
                    needs_review: m.needs_review,
                    filled: m.filled,
                    fill_pct: m.fill_pct,
                    tier: match m.tier {
                        stocker_screener::CoverageTier::Full => CoverageTier::Full,
                        stocker_screener::CoverageTier::Partial => CoverageTier::Partial,
                        stocker_screener::CoverageTier::Empty => CoverageTier::Empty,
                    },
                })
                .collect(),
        })
    }

    pub async fn list_screens() -> Result<Vec<SavedScreen>, String> {
        let svc = service().await?;
        let v = svc.list_screens().await.map_err(|e| e.to_string())?;
        Ok(v.into_iter()
            .map(|s| SavedScreen {
                id: s.id,
                name: s.name,
                filters: serde_json::to_value(s.filters).unwrap_or(serde_json::Value::Null),
                created_at: s.created_at,
                updated_at: s.updated_at,
            })
            .collect())
    }

    pub async fn create_screen(name: String, filters: serde_json::Value) -> Result<SavedScreen, String> {
        let svc = service().await?;
        let filters: Vec<stocker_screener::ScreenFilter> =
            serde_json::from_value(filters).map_err(|e| e.to_string())?;
        let new = NewSavedScreen { name, filters };
        let s = svc.create_screen(&new).await.map_err(|e| e.to_string())?;
        Ok(SavedScreen {
            id: s.id,
            name: s.name,
            filters: serde_json::to_value(s.filters).unwrap_or(serde_json::Value::Null),
            created_at: s.created_at,
            updated_at: s.updated_at,
        })
    }

    pub async fn delete_screen(id: i64) -> Result<(), String> {
        let svc = service().await?;
        svc.delete_screen(id).await.map_err(|e| e.to_string())
    }

    pub async fn load_snapshot(symbol: &str) -> Result<Option<ScreenRow>, String> {
        let svc = service().await?;
        let row = svc.snapshot_for(symbol).await.map_err(|e| e.to_string())?;
        Ok(row.map(|r| ScreenRow {
            symbol: r.symbol,
            short_name: r.short_name,
            sector: r.sector,
            industry: r.industry,
            exchange: r.exchange,
            currency: r.currency,
            country: r.country,
            tier: r.tier,
            face_value: r.face_value,
            last_refreshed_at: r.last_refreshed_at,
            last_refresh_status: r.last_refresh_status,
            last_refresh_error: r.last_refresh_error,
            updated_at: r.updated_at,
            metrics: r.metrics,
        }))
    }

    pub async fn start_backfill() -> Result<(), String> {
        let svc = service().await?;
        svc.try_start_backfill(RefreshConfig::from_env())
            .map_err(|e| e.to_string())
    }

    pub async fn refresh_symbol(symbol: &str) -> Result<(), String> {
        let svc = service().await?;
        svc.refresh_now(symbol).await.map_err(|e| e.to_string())
    }

    fn map_listing(r: stocker_screener::SymbolListing) -> SymbolListing {
        SymbolListing {
            symbol: r.symbol,
            id: r.id,
            short_name: r.short_name,
            exchange: r.exchange,
            sector: r.sector,
        }
    }

    fn map_pair(r: stocker_screener::SymbolPair) -> SymbolPair {
        SymbolPair {
            symbol: r.symbol,
            id: r.id,
            nse_id: r.nse_id,
            bse_id: r.bse_id,
            short_name: r.short_name,
            exchange: r.exchange,
            sector: r.sector,
        }
    }

    pub async fn search_symbols(
        search: &str,
        exchange: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SymbolListing>, String> {
        let svc = service().await?;
        let rows = svc
            .search_symbols(search, exchange, limit)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(map_listing).collect())
    }

    pub async fn symbol_pair(symbol: &str) -> Result<Option<SymbolPair>, String> {
        let svc = service().await?;
        Ok(svc
            .symbol_pair(symbol)
            .await
            .map_err(|e| e.to_string())?
            .map(map_pair))
    }

    pub async fn symbol_pair_from_id(id: &str, exchange: &str) -> Result<Option<SymbolPair>, String> {
        let svc = service().await?;
        Ok(svc
            .symbol_pair_from_id(id, exchange)
            .await
            .map_err(|e| e.to_string())?
            .map(map_pair))
    }

    pub async fn resolve_report_ticker(id: &str, exchange: &str) -> String {
        let id = id.trim();
        if id.is_empty() {
            return String::new();
        }
        if let Ok(svc) = service().await {
            if let Ok(ticker) = svc.resolve_ticker(id, exchange).await {
                if !ticker.is_empty() {
                    return ticker;
                }
            }
        }
        let ex = exchange.trim().to_uppercase();
        if ex == "BSE" {
            format!("{}.BO", id.to_uppercase())
        } else {
            format!("{}.NS", id.to_uppercase())
        }
    }
}

#[cfg(feature = "desktop")]
pub use desktop_backend::*;

/// Shared screener instance for other desktop in-process backends (e.g. portfolio).
#[cfg(all(feature = "desktop", not(feature = "web")))]
pub async fn shared_screener() -> Result<std::sync::Arc<stocker_screener::ScreenerService>, String> {
    desktop_backend::service().await
}
