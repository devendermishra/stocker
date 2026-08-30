//! Sector catalog queries: distinct Yahoo sectors + cohort metrics for research profiles.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use stocker_core::{
    compute_sector_research_profile, sector_inputs_from_aggregates, SectorCohortRow,
    SectorResearchProfile,
};

use crate::error::{Error, Result};

pub const UNCLASSIFIED: &str = "Unclassified";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorListItem {
    pub sector: String,
    pub company_count: i64,
    pub with_snapshot_count: i64,
    pub total_market_cap: Option<f64>,
    pub lifecycle: String,
    pub sector_type: String,
    pub attractiveness: f64,
    pub growth_prospects: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorMember {
    pub symbol: String,
    pub short_name: Option<String>,
    pub market_cap: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDetail {
    pub sector: String,
    pub company_count: i64,
    pub with_snapshot_count: i64,
    pub total_market_cap: Option<f64>,
    pub research: SectorResearchProfile,
    pub members: Vec<SectorMember>,
}

fn phase_str(p: stocker_core::SectorLifecyclePhase) -> String {
    match p {
        stocker_core::SectorLifecyclePhase::Startup => "startup".into(),
        stocker_core::SectorLifecyclePhase::Growth => "growth".into(),
        stocker_core::SectorLifecyclePhase::Consolidation => "consolidation".into(),
        stocker_core::SectorLifecyclePhase::MaturityOrDecline => "maturity_or_decline".into(),
    }
}

fn type_str(t: stocker_core::SectorTypeKind) -> String {
    match t {
        stocker_core::SectorTypeKind::Growth => "growth".into(),
        stocker_core::SectorTypeKind::Cyclical => "cyclical".into(),
        stocker_core::SectorTypeKind::Defensive => "defensive".into(),
        stocker_core::SectorTypeKind::CyclicalGrowth => "cyclical_growth".into(),
    }
}

fn growth_str(g: stocker_core::GrowthProspectsLevel) -> String {
    match g {
        stocker_core::GrowthProspectsLevel::Strong => "strong".into(),
        stocker_core::GrowthProspectsLevel::Moderate => "moderate".into(),
        stocker_core::GrowthProspectsLevel::Weak => "weak".into(),
        stocker_core::GrowthProspectsLevel::Contracting => "contracting".into(),
    }
}

/// Resolve a path/query sector name to the DB label (exact case-insensitive match).
pub async fn resolve_sector_name(pool: &SqlitePool, raw: &str) -> Result<String> {
    let decoded = urlencoding::decode(raw)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let trimmed = decoded.trim();
    if trimmed.eq_ignore_ascii_case(UNCLASSIFIED) || trimmed.is_empty() {
        return Ok(UNCLASSIFIED.to_string());
    }
    let row = sqlx::query(
        "SELECT sector FROM symbols WHERE sector IS NOT NULL AND TRIM(sector) != '' \
         AND LOWER(sector) = LOWER(?) LIMIT 1",
    )
    .bind(trimmed)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => Ok(r.try_get::<String, _>("sector")?),
        None => Err(Error::NotFound),
    }
}

pub async fn list_sectors(pool: &SqlitePool) -> Result<Vec<SectorListItem>> {
    let rows = sqlx::query(
        r#"
        SELECT
          CASE WHEN sym.sector IS NULL OR TRIM(sym.sector) = '' THEN ? ELSE sym.sector END AS sector_name,
          COUNT(*) AS company_count,
          SUM(CASE WHEN s.symbol IS NOT NULL THEN 1 ELSE 0 END) AS with_snapshot_count,
          SUM(s.market_cap) AS total_market_cap
        FROM symbols sym
        LEFT JOIN snapshots s ON s.symbol = sym.symbol
        GROUP BY sector_name
        ORDER BY company_count DESC, sector_name ASC
        "#,
    )
    .bind(UNCLASSIFIED)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let sector: String = row.try_get("sector_name")?;
        let company_count: i64 = row.try_get("company_count")?;
        let with_snapshot_count: i64 = row.try_get("with_snapshot_count")?;
        let total_market_cap: Option<f64> = row.try_get("total_market_cap")?;

        let cohort = fetch_cohort_rows(pool, &sector).await?;
        let inputs = sector_inputs_from_aggregates(&sector, company_count as usize, &cohort);
        let profile = compute_sector_research_profile(&inputs);

        out.push(SectorListItem {
            sector,
            company_count,
            with_snapshot_count,
            total_market_cap,
            lifecycle: phase_str(profile.lifecycle.phase),
            sector_type: type_str(profile.sector_type.sector_type),
            attractiveness: profile.porter.attractiveness,
            growth_prospects: growth_str(profile.growth_prospects.level),
        });
    }
    Ok(out)
}

async fn fetch_cohort_rows(pool: &SqlitePool, sector: &str) -> Result<Vec<SectorCohortRow>> {
    let unclassified = sector.eq_ignore_ascii_case(UNCLASSIFIED);
    let sql = if unclassified {
        r#"
        SELECT sym.symbol, sym.short_name,
               s.market_cap, s.gross_margins, s.ebitda_margins, s.opm_pct,
               s.npm_last_year_pct, s.return_on_equity, s.debt_to_equity,
               s.sales_growth_ttm_pct, s.sales_growth_3y_cagr_pct, s.sales_growth_5y_cagr_pct,
               s.profit_growth_3y_cagr_pct, s.profit_after_tax_ttm,
               s.npm_latest_quarter_pct, s.npm_preceding_quarter_pct
        FROM symbols sym
        LEFT JOIN snapshots s ON s.symbol = sym.symbol
        WHERE sym.sector IS NULL OR TRIM(sym.sector) = ''
        ORDER BY s.market_cap DESC NULLS LAST
        "#
    } else {
        r#"
        SELECT sym.symbol, sym.short_name,
               s.market_cap, s.gross_margins, s.ebitda_margins, s.opm_pct,
               s.npm_last_year_pct, s.return_on_equity, s.debt_to_equity,
               s.sales_growth_ttm_pct, s.sales_growth_3y_cagr_pct, s.sales_growth_5y_cagr_pct,
               s.profit_growth_3y_cagr_pct, s.profit_after_tax_ttm,
               s.npm_latest_quarter_pct, s.npm_preceding_quarter_pct
        FROM symbols sym
        LEFT JOIN snapshots s ON s.symbol = sym.symbol
        WHERE LOWER(sym.sector) = LOWER(?)
        ORDER BY s.market_cap DESC NULLS LAST
        "#
    };

    let rows = if unclassified {
        sqlx::query(sql).fetch_all(pool).await?
    } else {
        sqlx::query(sql).bind(sector).fetch_all(pool).await?
    };

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Skip pure symbol rows with no usable metrics for aggregation medians —
        // still count them via company_count; for inputs we only need snapshot-backed rows.
        let market_cap: Option<f64> = row.try_get("market_cap")?;
        let has_any = market_cap.is_some()
            || row
                .try_get::<Option<f64>, _>("return_on_equity")?
                .is_some()
            || row
                .try_get::<Option<f64>, _>("sales_growth_3y_cagr_pct")?
                .is_some();
        if !has_any {
            continue;
        }
        out.push(SectorCohortRow {
            symbol: row.try_get("symbol")?,
            short_name: row.try_get("short_name")?,
            market_cap,
            gross_margins: row.try_get("gross_margins")?,
            ebitda_margins: row.try_get("ebitda_margins")?,
            op_margin: row.try_get("opm_pct")?,
            net_margin: row.try_get("npm_last_year_pct")?,
            return_on_equity: row.try_get("return_on_equity")?,
            debt_to_equity: row.try_get("debt_to_equity")?,
            sales_growth_ttm_pct: row.try_get("sales_growth_ttm_pct")?,
            sales_growth_3y_pct: row.try_get("sales_growth_3y_cagr_pct")?,
            sales_growth_5y_pct: row.try_get("sales_growth_5y_cagr_pct")?,
            profit_growth_3y_pct: row.try_get("profit_growth_3y_cagr_pct")?,
            profit_after_tax: row.try_get("profit_after_tax_ttm")?,
            npm_latest: row.try_get("npm_latest_quarter_pct")?,
            npm_preceding: row.try_get("npm_preceding_quarter_pct")?,
        });
    }
    Ok(out)
}

pub async fn sector_detail(pool: &SqlitePool, sector_raw: &str) -> Result<SectorDetail> {
    let sector = resolve_sector_name(pool, sector_raw).await?;

    let count_row = if sector.eq_ignore_ascii_case(UNCLASSIFIED) {
        sqlx::query(
            r#"
            SELECT COUNT(*) AS company_count,
                   SUM(CASE WHEN s.symbol IS NOT NULL THEN 1 ELSE 0 END) AS with_snapshot_count,
                   SUM(s.market_cap) AS total_market_cap
            FROM symbols sym
            LEFT JOIN snapshots s ON s.symbol = sym.symbol
            WHERE sym.sector IS NULL OR TRIM(sym.sector) = ''
            "#,
        )
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT COUNT(*) AS company_count,
                   SUM(CASE WHEN s.symbol IS NOT NULL THEN 1 ELSE 0 END) AS with_snapshot_count,
                   SUM(s.market_cap) AS total_market_cap
            FROM symbols sym
            LEFT JOIN snapshots s ON s.symbol = sym.symbol
            WHERE LOWER(sym.sector) = LOWER(?)
            "#,
        )
        .bind(&sector)
        .fetch_one(pool)
        .await?
    };

    let company_count: i64 = count_row.try_get("company_count")?;
    if company_count == 0 {
        return Err(Error::NotFound);
    }
    let with_snapshot_count: i64 = count_row.try_get("with_snapshot_count")?;
    let total_market_cap: Option<f64> = count_row.try_get("total_market_cap")?;

    let cohort = fetch_cohort_rows(pool, &sector).await?;
    let inputs = sector_inputs_from_aggregates(&sector, company_count as usize, &cohort);
    let research = compute_sector_research_profile(&inputs);

    let members: Vec<SectorMember> = cohort
        .into_iter()
        .take(50)
        .map(|r| SectorMember {
            symbol: r.symbol,
            short_name: r.short_name,
            market_cap: r.market_cap,
        })
        .collect();

    Ok(SectorDetail {
        sector,
        company_count,
        with_snapshot_count,
        total_market_cap,
        research,
        members,
    })
}
