//! Per-metric fill rates across the `snapshots` table.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::Result;
use crate::metrics::{MetricId, CATALOG};

/// Availability tier for one metric across all snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageTier {
    Full,
    Partial,
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub full: usize,
    pub partial: usize,
    pub empty: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricCoverage {
    pub id: MetricId,
    pub label: &'static str,
    pub description: &'static str,
    pub category_label: &'static str,
    pub column: &'static str,
    pub source_kind: crate::metrics::SourceKind,
    pub needs_review: bool,
    pub filled: i64,
    pub fill_pct: f64,
    pub tier: CoverageTier,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub snapshot_count: i64,
    pub summary: CoverageSummary,
    pub metrics: Vec<MetricCoverage>,
}

fn tier_for(filled: i64, total: i64) -> CoverageTier {
    if total == 0 || filled == 0 {
        CoverageTier::Empty
    } else if filled >= total {
        CoverageTier::Full
    } else {
        CoverageTier::Partial
    }
}

/// Returns true when a parent column has more than 25% fill across snapshots.
pub async fn parent_usable(pool: &SqlitePool, parent_column: &str) -> Result<bool> {
    let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(pool)
        .await?;
    if snapshot_count == 0 {
        return Ok(false);
    }
    let sql = format!("SELECT COUNT(*) FROM snapshots WHERE {parent_column} IS NOT NULL");
    let filled: i64 = sqlx::query_scalar(&sql).fetch_one(pool).await?;
    Ok((filled as f64 / snapshot_count as f64) * 100.0 > 25.0)
}

/// Count non-null values per catalog column in `snapshots`.
pub async fn coverage_report(pool: &SqlitePool) -> Result<CoverageReport> {
    let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(pool)
        .await?;

    let mut metrics = Vec::with_capacity(CATALOG.len());
    let mut full = 0usize;
    let mut partial = 0usize;
    let mut empty = 0usize;

    for spec in CATALOG {
        let col = spec.column;
        let sql = format!("SELECT COUNT(*) FROM snapshots WHERE {col} IS NOT NULL");
        let filled: i64 = sqlx::query_scalar(&sql).fetch_one(pool).await?;
        let fill_pct = if snapshot_count > 0 {
            (filled as f64 / snapshot_count as f64) * 100.0
        } else {
            0.0
        };
        let tier = tier_for(filled, snapshot_count);
        match tier {
            CoverageTier::Full => full += 1,
            CoverageTier::Partial => partial += 1,
            CoverageTier::Empty => empty += 1,
        }
        metrics.push(MetricCoverage {
            id: spec.id,
            label: spec.label,
            description: spec.description,
            category_label: spec.category.label(),
            column: col,
            source_kind: spec.source_kind,
            needs_review: spec.needs_review,
            filled,
            fill_pct,
            tier,
        });
    }

    Ok(CoverageReport {
        snapshot_count,
        summary: CoverageSummary {
            full,
            partial,
            empty,
        },
        metrics,
    })
}
