//! Symbol search and NSE/BSE pairing for the stock list UI.

use std::collections::HashMap;

use sqlx::{Row, SqlitePool};

use crate::error::Result;

/// One deduplicated row in the stock list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolListing {
    pub symbol: String,
    pub id: String,
    pub short_name: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,
}

/// Pair lookup for exchange switching (includes both NSE/BSE ids when linked).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolPair {
    pub symbol: String,
    pub id: String,
    pub nse_id: Option<String>,
    pub bse_id: Option<String>,
    pub short_name: Option<String>,
    pub exchange: Option<String>,
    pub sector: Option<String>,
}

#[derive(Debug, Clone)]
struct SymbolIdentityRow {
    symbol: String,
    short_name: Option<String>,
    sector: Option<String>,
    exchange: Option<String>,
    isin: Option<String>,
}

fn base_id_from_yahoo(symbol: &str) -> Option<String> {
    stocker_core::india_base_symbol(symbol).ok()
}

/// Normalize company name for dedup: lowercase, trim dots, LTD/Limited equivalence.
pub fn normalize_company_name(name: &str) -> String {
    let mut s = name.trim().to_lowercase();
    while s.ends_with('.') {
        s.pop();
    }
    s = s.replace(" ltd.", " ltd");
    s = s.replace(" limited", " ltd");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_nse(row: &SymbolIdentityRow) -> bool {
    row.exchange
        .as_deref()
        .is_some_and(|e| e.eq_ignore_ascii_case("NSE"))
        || row.symbol.ends_with(".NS")
}

fn prefer_nse(a: &SymbolIdentityRow, b: &SymbolIdentityRow) -> SymbolIdentityRow {
    if is_nse(a) {
        a.clone()
    } else if is_nse(b) {
        b.clone()
    } else {
        a.clone()
    }
}

fn identity_to_listing(row: &SymbolIdentityRow) -> SymbolListing {
    let id = base_id_from_yahoo(&row.symbol).unwrap_or_else(|| row.symbol.clone());
    let exchange = Some(
        stocker_core::india_exchange_label(&row.symbol, row.exchange.as_deref()).to_string(),
    );
    SymbolListing {
        symbol: row.symbol.clone(),
        id,
        short_name: row.short_name.clone(),
        exchange,
        sector: row.sector.clone(),
    }
}

fn build_pair(primary: &SymbolIdentityRow, counterpart: Option<&SymbolIdentityRow>) -> SymbolPair {
    let mut nse_id = None;
    let mut bse_id = None;

    if primary.symbol.ends_with(".NS") {
        nse_id = base_id_from_yahoo(&primary.symbol);
    } else if primary.symbol.ends_with(".BO") {
        bse_id = base_id_from_yahoo(&primary.symbol);
    }

    if let Some(cp) = counterpart {
        if cp.symbol.ends_with(".NS") {
            nse_id = base_id_from_yahoo(&cp.symbol).or(nse_id);
        } else if cp.symbol.ends_with(".BO") {
            bse_id = base_id_from_yahoo(&cp.symbol).or(bse_id);
        }
    }
    if nse_id.is_some() && bse_id.is_none() {
        bse_id = nse_id.clone();
    }

    let id = nse_id
        .clone()
        .or(bse_id.clone())
        .unwrap_or_else(|| base_id_from_yahoo(&primary.symbol).unwrap_or_else(|| primary.symbol.clone()));

    SymbolPair {
        symbol: primary.symbol.clone(),
        id,
        nse_id,
        bse_id,
        short_name: primary.short_name.clone(),
        exchange: Some(
            stocker_core::india_exchange_label(&primary.symbol, primary.exchange.as_deref())
                .to_string(),
        ),
        sector: primary.sector.clone(),
    }
}

fn dedupe_key_name(row: &SymbolIdentityRow) -> String {
    if let Some(ref name) = row.short_name {
        let n = normalize_company_name(name);
        if !n.is_empty() {
            return n;
        }
    }
    if let Some(id) = base_id_from_yahoo(&row.symbol) {
        return id.to_lowercase();
    }
    row.symbol.to_lowercase()
}

/// Collapse duplicate listings: ISIN groups first, then normalized name; NSE preferred.
fn dedupe_listings(
    rows: Vec<SymbolIdentityRow>,
    exchange_filter: Option<&str>,
    limit: usize,
) -> Vec<SymbolListing> {
    let mut by_isin: HashMap<String, SymbolIdentityRow> = HashMap::new();
    let mut no_isin: Vec<SymbolIdentityRow> = Vec::new();

    for row in rows {
        if let Some(ref isin) = row.isin {
            if !isin.is_empty() {
                by_isin
                    .entry(isin.clone())
                    .and_modify(|existing| {
                        *existing = prefer_nse(existing, &row);
                    })
                    .or_insert(row);
                continue;
            }
        }
        no_isin.push(row);
    }

    let isin_rows: Vec<SymbolIdentityRow> = by_isin.into_values().collect();
    let isin_names: std::collections::HashSet<String> = isin_rows
        .iter()
        .map(|r| dedupe_key_name(r))
        .collect();

    let mut by_name: HashMap<String, SymbolIdentityRow> = HashMap::new();
    for row in no_isin {
        let key = dedupe_key_name(&row);
        if isin_names.contains(&key) {
            continue;
        }
        by_name
            .entry(key)
            .and_modify(|existing| {
                *existing = prefer_nse(existing, &row);
            })
            .or_insert(row);
    }
    let mut merged = isin_rows;
    merged.extend(by_name.into_values());

    let mut listings: Vec<SymbolListing> = merged.into_iter().map(|r| identity_to_listing(&r)).collect();

    listings.sort_by(|a, b| {
        let an = a.short_name.as_deref().unwrap_or(&a.id).to_lowercase();
        let bn = b.short_name.as_deref().unwrap_or(&b.id).to_lowercase();
        an.cmp(&bn).then_with(|| a.id.cmp(&b.id))
    });

    if let Some(ex) = exchange_filter {
        if ex.eq_ignore_ascii_case("BSE") {
            listings.retain(|l| {
                l.exchange
                    .as_deref()
                    .is_some_and(|e| e.eq_ignore_ascii_case("BSE"))
            });
        } else if ex.eq_ignore_ascii_case("NSE") {
            // NSE filter shows deduped list (dual-listed already collapsed to NSE).
            listings.retain(|l| {
                l.exchange
                    .as_deref()
                    .is_some_and(|e| e.eq_ignore_ascii_case("NSE"))
            });
        }
    }

    listings.truncate(limit);
    listings
}

fn row_from_sql(row: &sqlx::sqlite::SqliteRow) -> Result<SymbolIdentityRow> {
    Ok(SymbolIdentityRow {
        symbol: row.try_get("symbol")?,
        short_name: row.try_get("short_name").ok(),
        sector: row.try_get("sector").ok(),
        exchange: row.try_get("exchange").ok(),
        isin: row.try_get("isin").ok(),
    })
}

async fn fetch_identity(pool: &SqlitePool, symbol: &str) -> Result<Option<SymbolIdentityRow>> {
    let row = sqlx::query(
        "SELECT symbol, short_name, sector, exchange, isin FROM symbols WHERE symbol = ?",
    )
    .bind(symbol)
    .fetch_optional(pool)
    .await?;

    row.map(|r| row_from_sql(&r)).transpose()
}

async fn fetch_counterpart_by_isin(
    pool: &SqlitePool,
    isin: &str,
    exclude_symbol: &str,
) -> Result<Option<SymbolIdentityRow>> {
    let row = sqlx::query(
        "SELECT symbol, short_name, sector, exchange, isin FROM symbols \
         WHERE isin = ? AND symbol != ? LIMIT 1",
    )
    .bind(isin)
    .bind(exclude_symbol)
    .fetch_optional(pool)
    .await?;

    row.map(|r| row_from_sql(&r)).transpose()
}

/// Search symbols by name or ticker base, optionally filtered by exchange.
pub async fn search_symbols(
    pool: &SqlitePool,
    search: &str,
    exchange: Option<&str>,
    limit: i64,
) -> Result<Vec<SymbolListing>> {
    let limit = limit.clamp(1, 200) as usize;
    let q = search.trim();
    let exchange_filter = exchange
        .map(|e| e.trim().to_uppercase())
        .filter(|e| !e.is_empty() && e != "ALL");

    let fetch_limit = (limit * 3).min(500) as i64;

    let rows = if q.is_empty() {
        sqlx::query(
            "SELECT symbol, short_name, sector, exchange, isin FROM symbols \
             ORDER BY short_name COLLATE NOCASE, symbol LIMIT ?",
        )
        .bind(fetch_limit)
        .fetch_all(pool)
        .await?
    } else {
        let pattern = format!("%{}%", q.to_lowercase());
        sqlx::query(
            "SELECT symbol, short_name, sector, exchange, isin FROM symbols \
             WHERE LOWER(short_name) LIKE ? OR LOWER(symbol) LIKE ? \
             ORDER BY short_name COLLATE NOCASE, symbol LIMIT ?",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(fetch_limit)
        .fetch_all(pool)
        .await?
    };

    let identities: Result<Vec<SymbolIdentityRow>> = rows.iter().map(row_from_sql).collect();
    let deduped = dedupe_listings(
        identities?,
        exchange_filter.as_deref(),
        limit,
    );
    Ok(deduped)
}

/// Resolve a Yahoo ticker (or base symbol) to a pair with both NSE/BSE IDs when linked.
pub async fn symbol_pair(pool: &SqlitePool, raw: &str) -> Result<Option<SymbolPair>> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }

    let yahoo = if s.ends_with(".NS") || s.ends_with(".BO") {
        s.to_uppercase()
    } else {
        let upper = s.to_uppercase();
        if fetch_identity(pool, &format!("{upper}.NS")).await?.is_some() {
            format!("{upper}.NS")
        } else if fetch_identity(pool, &format!("{upper}.BO")).await?.is_some() {
            format!("{upper}.BO")
        } else {
            format!("{upper}.NS")
        }
    };

    let primary = fetch_identity(pool, &yahoo).await?;
    let Some(primary) = primary else {
        let id = base_id_from_yahoo(&yahoo).unwrap_or_else(|| yahoo.clone());
        let mut pair = SymbolPair {
            symbol: yahoo.clone(),
            id,
            nse_id: None,
            bse_id: None,
            short_name: None,
            exchange: None,
            sector: None,
        };
        if yahoo.ends_with(".NS") {
            pair.nse_id = base_id_from_yahoo(&yahoo);
            pair.exchange = Some("NSE".to_string());
        } else if yahoo.ends_with(".BO") {
            pair.bse_id = base_id_from_yahoo(&yahoo);
            pair.exchange = Some("BSE".to_string());
        }
        return Ok(Some(pair));
    };

    let counterpart = if let Some(ref isin) = primary.isin {
        fetch_counterpart_by_isin(pool, isin, &primary.symbol).await?
    } else {
        None
    };

    Ok(Some(build_pair(&primary, counterpart.as_ref())))
}

/// Build Yahoo ticker from base id and exchange, using pair lookup when ids differ.
pub async fn resolve_ticker(pool: &SqlitePool, id: &str, exchange: &str) -> Result<String> {
    let id = id.trim();
    if id.is_empty() {
        return Ok(String::new());
    }
    let ex = exchange.trim().to_uppercase();
    if let Some(pair) = symbol_pair(pool, id).await? {
        if ex == "BSE" {
            if let Some(bse) = pair.bse_id {
                return Ok(format!("{bse}.BO"));
            }
        } else if let Some(nse) = pair.nse_id {
            return Ok(format!("{nse}.NS"));
        }
    }
    if ex == "BSE" {
        Ok(format!("{}.BO", id.to_uppercase()))
    } else {
        Ok(format!("{}.NS", id.to_uppercase()))
    }
}

/// Resolve a user-entered base symbol on one exchange to a pair with both IDs.
pub async fn symbol_pair_from_id(
    pool: &SqlitePool,
    id: &str,
    exchange: &str,
) -> Result<Option<SymbolPair>> {
    let id = id.trim();
    if id.is_empty() {
        return Ok(None);
    }
    let ex = exchange.trim().to_uppercase();
    let yahoo = if ex == "BSE" {
        format!("{}.BO", id.to_uppercase())
    } else {
        format!("{}.NS", id.to_uppercase())
    };
    symbol_pair(pool, &yahoo).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn insert_symbol(
        pool: &SqlitePool,
        sym: &str,
        name: &str,
        ex: &str,
        isin: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO symbols (symbol, short_name, exchange, isin, tier) VALUES (?, ?, ?, ?, 1)",
        )
        .bind(sym)
        .bind(name)
        .bind(ex)
        .bind(isin)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn search_dedup_by_isin_prefers_nse() {
        let pool = db::open_memory().await.unwrap();
        insert_symbol(
            &pool,
            "RELIANCE.NS",
            "Reliance Industries Limited",
            "NSE",
            Some("INE002A01018"),
        )
        .await;
        insert_symbol(
            &pool,
            "RELIANCE.BO",
            "Reliance Industries Ltd",
            "BSE",
            Some("INE002A01018"),
        )
        .await;

        let rows = search_symbols(&pool, "reliance", None, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "RELIANCE");
        assert_eq!(rows[0].exchange.as_deref(), Some("NSE"));
        assert_eq!(rows[0].symbol, "RELIANCE.NS");
    }

    #[tokio::test]
    async fn search_dedup_ltd_vs_limited() {
        let pool = db::open_memory().await.unwrap();
        insert_symbol(
            &pool,
            "3IINFOTECH.NS",
            "3I INFOTECH LTD.",
            "NSE",
            None,
        )
        .await;
        insert_symbol(
            &pool,
            "3IINFOTECH.BO",
            "3i Infotech Limited",
            "BSE",
            None,
        )
        .await;

        let rows = search_symbols(&pool, "3i infotech", None, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].exchange.as_deref(), Some("NSE"));
    }

    #[tokio::test]
    async fn bse_filter_excludes_dual_listed() {
        let pool = db::open_memory().await.unwrap();
        insert_symbol(
            &pool,
            "RELIANCE.NS",
            "Reliance Industries Limited",
            "NSE",
            Some("INE002A01018"),
        )
        .await;
        insert_symbol(
            &pool,
            "RELIANCE.BO",
            "Reliance Industries Ltd",
            "BSE",
            Some("INE002A01018"),
        )
        .await;
        insert_symbol(
            &pool,
            "BSEONLY.BO",
            "BSE Only Company Ltd",
            "BSE",
            None,
        )
        .await;

        let rows = search_symbols(&pool, "", Some("BSE"), 50).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "BSEONLY");
    }

    #[tokio::test]
    async fn symbol_pair_returns_both_ids() {
        let pool = db::open_memory().await.unwrap();
        insert_symbol(
            &pool,
            "RELIANCE.NS",
            "Reliance Industries Limited",
            "NSE",
            Some("INE002A01018"),
        )
        .await;
        insert_symbol(
            &pool,
            "RELIANCE.BO",
            "Reliance Industries Ltd",
            "BSE",
            Some("INE002A01018"),
        )
        .await;

        let pair = symbol_pair(&pool, "RELIANCE.NS").await.unwrap().unwrap();
        assert_eq!(pair.nse_id.as_deref(), Some("RELIANCE"));
        assert_eq!(pair.bse_id.as_deref(), Some("RELIANCE"));
    }

    #[test]
    fn normalize_ltd_limited() {
        assert_eq!(
            normalize_company_name("3I INFOTECH LTD."),
            normalize_company_name("3i Infotech Limited")
        );
        assert_eq!(
            normalize_company_name("RELIANCE INDUSTRIES LTD."),
            normalize_company_name("Reliance Industries Limited")
        );
    }

    #[tokio::test]
    async fn search_dedup_isin_row_and_name_only_bse() {
        let pool = db::open_memory().await.unwrap();
        insert_symbol(
            &pool,
            "RELIANCE.NS",
            "Reliance Industries Limited",
            "NSE",
            Some("INE002A01018"),
        )
        .await;
        insert_symbol(
            &pool,
            "RELIANCE.BO",
            "RELIANCE INDUSTRIES LTD.",
            "BSE",
            None,
        )
        .await;

        let rows = search_symbols(&pool, "reliance", None, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "RELIANCE");
        assert_eq!(rows[0].exchange.as_deref(), Some("NSE"));
    }
}
