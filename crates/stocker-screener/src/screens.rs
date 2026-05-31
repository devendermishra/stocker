//! Saved-screens CRUD against the `saved_screens` table.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::query::ScreenFilter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedScreen {
    pub id: i64,
    pub name: String,
    pub filters: Vec<ScreenFilter>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSavedScreen {
    pub name: String,
    pub filters: Vec<ScreenFilter>,
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<SavedScreen>> {
    let rows = sqlx::query("SELECT id, name, filters_json, created_at, updated_at FROM saved_screens ORDER BY name ASC")
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let json: String = row.try_get("filters_json")?;
        let filters: Vec<ScreenFilter> = serde_json::from_str(&json)
            .map_err(|e| Error::Other(format!("decode saved screen: {e}")))?;
        out.push(SavedScreen {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            filters,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<SavedScreen> {
    let row = sqlx::query("SELECT id, name, filters_json, created_at, updated_at FROM saved_screens WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound)?;
    let json: String = row.try_get("filters_json")?;
    let filters: Vec<ScreenFilter> = serde_json::from_str(&json)
        .map_err(|e| Error::Other(format!("decode saved screen: {e}")))?;
    Ok(SavedScreen {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        filters,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub async fn create(pool: &SqlitePool, screen: &NewSavedScreen) -> Result<SavedScreen> {
    if screen.name.trim().is_empty() {
        return Err(Error::InvalidQuery("saved screen requires a name".into()));
    }
    let now = Utc::now().timestamp();
    let json = serde_json::to_string(&screen.filters)
        .map_err(|e| Error::Other(format!("encode filters: {e}")))?;
    let res = sqlx::query("INSERT INTO saved_screens (name, filters_json, created_at, updated_at) VALUES (?, ?, ?, ?)")
        .bind(&screen.name)
        .bind(&json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    let id = res.last_insert_rowid();
    Ok(SavedScreen {
        id,
        name: screen.name.clone(),
        filters: screen.filters.clone(),
        created_at: now,
        updated_at: now,
    })
}

pub async fn update(pool: &SqlitePool, id: i64, screen: &NewSavedScreen) -> Result<SavedScreen> {
    let now = Utc::now().timestamp();
    let json = serde_json::to_string(&screen.filters)
        .map_err(|e| Error::Other(format!("encode filters: {e}")))?;
    let res = sqlx::query("UPDATE saved_screens SET name = ?, filters_json = ?, updated_at = ? WHERE id = ?")
        .bind(&screen.name)
        .bind(&json)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: i64) -> Result<()> {
    let res = sqlx::query("DELETE FROM saved_screens WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}
