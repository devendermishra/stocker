//! Portfolio CRUD.

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::{NewPortfolio, Portfolio, PortfolioStatus, UpdatePortfolio};

pub async fn list(
    pool: &SqlitePool,
    user_id: i64,
    include_archived: bool,
) -> Result<Vec<Portfolio>> {
    let rows = if include_archived {
        sqlx::query(
            "SELECT id, user_id, name, description, base_currency, portfolio_type, status,
             created_at, updated_at FROM portfolios WHERE user_id = ? ORDER BY name ASC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, user_id, name, description, base_currency, portfolio_type, status,
             created_at, updated_at FROM portfolios WHERE user_id = ? AND status = 'active'
             ORDER BY name ASC",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?
    };

    rows.iter().map(row_to_portfolio).collect()
}

pub async fn get(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Portfolio> {
    let row = sqlx::query(
        "SELECT id, user_id, name, description, base_currency, portfolio_type, status,
         created_at, updated_at FROM portfolios WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::NotFound)?;
    row_to_portfolio(&row)
}

pub async fn create(pool: &SqlitePool, user_id: i64, input: &NewPortfolio) -> Result<Portfolio> {
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput("portfolio name is required".into()));
    }
    let now = Utc::now().timestamp();
    let base_currency = input
        .base_currency
        .as_deref()
        .unwrap_or("INR")
        .to_string();
    let portfolio_type = input
        .portfolio_type
        .as_deref()
        .unwrap_or("mixed")
        .to_string();

    let res = sqlx::query(
        "INSERT INTO portfolios (user_id, name, description, base_currency, portfolio_type,
         status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(user_id)
    .bind(input.name.trim())
    .bind(input.description.as_deref())
    .bind(&base_currency)
    .bind(&portfolio_type)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    get(pool, user_id, res.last_insert_rowid()).await
}

pub async fn update(
    pool: &SqlitePool,
    user_id: i64,
    id: i64,
    input: &UpdatePortfolio,
) -> Result<Portfolio> {
    let existing = get(pool, user_id, id).await?;
    let now = Utc::now().timestamp();
    let name = input.name.as_deref().unwrap_or(&existing.name);
    let description = input.description.as_ref().or(existing.description.as_ref());
    let portfolio_type = input
        .portfolio_type
        .as_deref()
        .unwrap_or(&existing.portfolio_type);
    let status = input.status.unwrap_or(existing.status);

    sqlx::query(
        "UPDATE portfolios SET name = ?, description = ?, portfolio_type = ?, status = ?,
         updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(portfolio_type)
    .bind(status.as_str())
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    get(pool, user_id, id).await
}

pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<()> {
    let res = sqlx::query("DELETE FROM portfolios WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}

fn row_to_portfolio(row: &sqlx::sqlite::SqliteRow) -> Result<Portfolio> {
    let status_str: String = row.try_get("status")?;
    let status = PortfolioStatus::parse(&status_str)
        .ok_or_else(|| Error::Other(format!("invalid portfolio status {status_str}")))?;
    Ok(Portfolio {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        base_currency: row.try_get("base_currency")?,
        portfolio_type: row.try_get("portfolio_type")?,
        status,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
