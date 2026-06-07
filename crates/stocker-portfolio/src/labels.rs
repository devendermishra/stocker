//! Label CRUD and polymorphic attachment.

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::{Label, LabelEntityType, NewLabel};

pub async fn list(pool: &SqlitePool, user_id: i64) -> Result<Vec<Label>> {
    let rows = sqlx::query(
        "SELECT id, user_id, name, color, created_at FROM labels WHERE user_id = ? ORDER BY name ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_to_label).collect()
}

pub async fn get(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Label> {
    let row = sqlx::query(
        "SELECT id, user_id, name, color, created_at FROM labels WHERE id = ? AND user_id = ?",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::NotFound)?;
    row_to_label(&row)
}

pub async fn create(pool: &SqlitePool, user_id: i64, input: &NewLabel) -> Result<Label> {
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput("label name is required".into()));
    }
    let now = Utc::now().timestamp();
    let res = sqlx::query(
        "INSERT INTO labels (user_id, name, color, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(input.name.trim())
    .bind(input.color.as_deref())
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return Error::Conflict("label already exists".into());
            }
        }
        Error::from(e)
    })?;

    get(pool, user_id, res.last_insert_rowid()).await
}

pub async fn update(
    pool: &SqlitePool,
    user_id: i64,
    id: i64,
    input: &NewLabel,
) -> Result<Label> {
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput("label name is required".into()));
    }
    let res = sqlx::query(
        "UPDATE labels SET name = ?, color = ? WHERE id = ? AND user_id = ?",
    )
    .bind(input.name.trim())
    .bind(input.color.as_deref())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    get(pool, user_id, id).await
}

pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<()> {
    let _ = get(pool, user_id, id).await?;
    sqlx::query("DELETE FROM labels WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn attach(
    pool: &SqlitePool,
    user_id: i64,
    label_id: i64,
    entity_type: LabelEntityType,
    entity_id: &str,
) -> Result<()> {
    let _ = get(pool, user_id, label_id).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO label_links (label_id, entity_type, entity_id) VALUES (?, ?, ?)",
    )
    .bind(label_id)
    .bind(entity_type.as_str())
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn detach(
    pool: &SqlitePool,
    user_id: i64,
    label_id: i64,
    entity_type: LabelEntityType,
    entity_id: &str,
) -> Result<()> {
    let _ = get(pool, user_id, label_id).await?;
    sqlx::query(
        "DELETE FROM label_links WHERE label_id = ? AND entity_type = ? AND entity_id = ?",
    )
    .bind(label_id)
    .bind(entity_type.as_str())
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn labels_for_entity(
    pool: &SqlitePool,
    user_id: i64,
    entity_type: LabelEntityType,
    entity_id: &str,
) -> Result<Vec<Label>> {
    let rows = sqlx::query(
        "SELECT l.id, l.user_id, l.name, l.color, l.created_at
         FROM labels l
         JOIN label_links ll ON ll.label_id = l.id
         WHERE l.user_id = ? AND ll.entity_type = ? AND ll.entity_id = ?",
    )
    .bind(user_id)
    .bind(entity_type.as_str())
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_to_label).collect()
}

pub async fn labels_for_entities(
    pool: &SqlitePool,
    user_id: i64,
    entity_type: LabelEntityType,
    entity_ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<Label>>> {
    let mut out = std::collections::HashMap::new();
    if entity_ids.is_empty() {
        return Ok(out);
    }
    for id in entity_ids {
        let labels = labels_for_entity(pool, user_id, entity_type, id).await?;
        out.insert(id.clone(), labels);
    }
    Ok(out)
}

fn row_to_label(row: &sqlx::sqlite::SqliteRow) -> Result<Label> {
    Ok(Label {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        color: row.try_get("color")?,
        created_at: row.try_get("created_at")?,
    })
}
