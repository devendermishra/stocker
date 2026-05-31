//! Backfill composite snapshot columns from parent columns already stored in SQLite.
//!
//! This avoids a Yahoo re-fetch when parent metrics were populated on a prior refresh
//! but composite formulas were tightened or added later.

use sqlx::SqlitePool;

use crate::error::Result;

/// Number of snapshot rows updated (one row may receive multiple column updates).
#[derive(Debug, Clone, Copy, Default)]
pub struct RecomputeStats {
    pub rows_touched: u64,
}

/// Recompute composite metrics from existing parent columns in `snapshots`.
pub async fn recompute_composites(pool: &SqlitePool) -> Result<RecomputeStats> {
    let mut rows_touched: u64 = 0;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET pb_x_pe = pe_ratio * price_to_book
        WHERE pe_ratio IS NOT NULL AND price_to_book IS NOT NULL AND pe_ratio > 0 AND price_to_book > 0
        "#,
    )
    .await?;

    rows_touched += recompute_graham_numbers(pool).await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET mcap_to_sales = market_cap / revenue_ttm
        WHERE market_cap IS NOT NULL AND revenue_ttm IS NOT NULL AND market_cap > 0 AND revenue_ttm > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET mcap_to_cfo = market_cap / operating_cashflow_ttm
        WHERE market_cap IS NOT NULL AND operating_cashflow_ttm IS NOT NULL
          AND market_cap > 0 AND ABS(operating_cashflow_ttm) > 1e-3
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET price_to_fcf = current_price / ((free_cashflow_3y_sum / 3.0) / shares_outstanding)
        WHERE current_price IS NOT NULL AND shares_outstanding IS NOT NULL AND free_cashflow_3y_sum IS NOT NULL
          AND current_price > 0 AND shares_outstanding > 0 AND free_cashflow_3y_sum > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET from_52w_high_pct = ((fifty_two_week_high - current_price) / fifty_two_week_high) * 100.0
        WHERE fifty_two_week_high IS NOT NULL AND current_price IS NOT NULL
          AND fifty_two_week_high > 0 AND current_price > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET up_from_52w_low_pct = ((current_price - fifty_two_week_low) / fifty_two_week_low) * 100.0
        WHERE fifty_two_week_low IS NOT NULL AND current_price IS NOT NULL
          AND fifty_two_week_low > 0 AND current_price > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET peg_ratio = pe_ratio / profit_growth_3y_cagr_pct
        WHERE pe_ratio IS NOT NULL AND profit_growth_3y_cagr_pct IS NOT NULL
          AND pe_ratio > 0 AND profit_growth_3y_cagr_pct > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET debt_capacity = (ebitda * 5.0 - total_debt) / net_worth
        WHERE ebitda IS NOT NULL AND total_debt IS NOT NULL AND net_worth IS NOT NULL
          AND ebitda > 0 AND net_worth > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET mcap_to_debt_capacity = market_cap / (ebitda * 5.0)
        WHERE market_cap IS NOT NULL AND ebitda IS NOT NULL AND market_cap > 0 AND ebitda > 0
        "#,
    )
    .await?;

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET price_to_quarterly_earning = current_price / ((profit_after_tax_latest_quarter / shares_outstanding) * 4.0)
        WHERE current_price IS NOT NULL AND shares_outstanding IS NOT NULL
          AND profit_after_tax_latest_quarter IS NOT NULL
          AND current_price > 0 AND shares_outstanding > 0
          AND profit_after_tax_latest_quarter > 0
        "#,
    )
    .await?;

    // earnings_yield_pct (EBIT/EV) and financial_leverage (2-year avg assets/NW) are
    // computed from statements during refresh; skip SQL recompute here.

    rows_touched += run_update(
        pool,
        r#"
        UPDATE snapshots SET face_value = (
            SELECT face_value FROM symbols WHERE symbols.symbol = snapshots.symbol
        )
        WHERE EXISTS (
            SELECT 1 FROM symbols
            WHERE symbols.symbol = snapshots.symbol
              AND symbols.face_value IS NOT NULL AND symbols.face_value > 0
        )
        "#,
    )
    .await?;

    Ok(RecomputeStats { rows_touched })
}

async fn run_update(pool: &SqlitePool, sql: &str) -> Result<u64> {
    let result = sqlx::query(sql).execute(pool).await?;
    Ok(result.rows_affected())
}

async fn recompute_graham_numbers(pool: &SqlitePool) -> Result<u64> {
    let rows: Vec<(String, f64, f64)> = sqlx::query_as(
        r#"
        SELECT symbol, eps_ttm, book_value FROM snapshots
        WHERE eps_ttm IS NOT NULL AND book_value IS NOT NULL AND eps_ttm > 0 AND book_value > 0
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut n = 0u64;
    for (symbol, eps, bv) in rows {
        let graham = (22.5 * eps * bv).sqrt();
        if !graham.is_finite() || graham <= 0.0 {
            continue;
        }
        let r = sqlx::query("UPDATE snapshots SET graham_number = ?1 WHERE symbol = ?2")
            .bind(graham)
            .bind(&symbol)
            .execute(pool)
            .await?;
        n += r.rows_affected();
    }
    Ok(n)
}
