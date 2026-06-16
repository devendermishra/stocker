//! Label CRUD and polymorphic attachment.

use std::collections::HashSet;

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::{DeleteLabelResult, Label, LabelEntityType, NewLabel, NewPortfolio};
use crate::portfolios;
use crate::transactions::{self, TransactionFilter};

pub async fn list(pool: &SqlitePool, user_id: i64) -> Result<Vec<Label>> {
    let rows = sqlx::query(
        "SELECT id, user_id, name, color, created_at FROM labels WHERE user_id = ? ORDER BY name ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut labels: Vec<Label> = rows.iter().map(row_to_label).collect::<Result<_>>()?;
    for label in &mut labels {
        label.holding_count = count_entity_links(pool, label.id, "holding").await? as i64;
        label.portfolio_count =
            portfolio_ids_by_name(pool, user_id, &label.name).await?.len() as i64;
        label.transaction_count =
            affected_transaction_ids(pool, user_id, label.id).await? as i64;
    }
    Ok(labels)
}

async fn count_entity_links(pool: &SqlitePool, label_id: i64, entity_type: &str) -> Result<usize> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM label_links WHERE label_id = ? AND entity_type = ?",
    )
    .bind(label_id)
    .bind(entity_type)
    .fetch_one(pool)
    .await?;
    Ok(count as usize)
}

/// Parse holding link id `"{portfolio_id}:{symbol}"` (symbol may contain `:` e.g. `MF:123`).
fn parse_holding_entity_id(entity_id: &str) -> Option<(i64, String)> {
    let (portfolio_id, symbol) = entity_id.split_once(':')?;
    Some((portfolio_id.parse().ok()?, symbol.to_string()))
}

/// Transactions removed when this label is deleted (direct tags + all txns for tagged holdings).
async fn affected_transaction_ids(
    pool: &SqlitePool,
    user_id: i64,
    label_id: i64,
) -> Result<usize> {
    Ok(collect_affected_transaction_ids(pool, user_id, label_id).await?.len())
}

async fn collect_affected_transaction_ids(
    pool: &SqlitePool,
    user_id: i64,
    label_id: i64,
) -> Result<HashSet<i64>> {
    let mut ids = HashSet::new();

    let txn_id_strs: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM label_links WHERE label_id = ? AND entity_type = 'transaction'",
    )
    .bind(label_id)
    .fetch_all(pool)
    .await?;
    for s in txn_id_strs {
        if let Ok(id) = s.parse::<i64>() {
            ids.insert(id);
        }
    }

    let holding_ids: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM label_links WHERE label_id = ? AND entity_type = 'holding'",
    )
    .bind(label_id)
    .fetch_all(pool)
    .await?;

    for entity_id in holding_ids {
        let Some((portfolio_id, symbol)) = parse_holding_entity_id(&entity_id) else {
            continue;
        };
        let txns = transactions::list(
            pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                symbol: Some(symbol),
                ..Default::default()
            },
        )
        .await?;
        for t in txns {
            ids.insert(t.id);
        }
    }

    let portfolio_ids: Vec<String> = sqlx::query_scalar(
        "SELECT entity_id FROM label_links WHERE label_id = ? AND entity_type = 'portfolio'",
    )
    .bind(label_id)
    .fetch_all(pool)
    .await?;
    for entity_id in portfolio_ids {
        let Ok(portfolio_id) = entity_id.parse::<i64>() else {
            continue;
        };
        let txns = transactions::list(
            pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
        )
        .await?;
        for t in txns {
            ids.insert(t.id);
        }
    }

    // Label and portfolio share the same name — include all transactions in matching portfolios.
    let label = get(pool, user_id, label_id).await?;
    for portfolio_id in portfolio_ids_by_name(pool, user_id, &label.name).await? {
        let txns = transactions::list(
            pool,
            user_id,
            &TransactionFilter {
                portfolio_id: Some(portfolio_id),
                ..Default::default()
            },
        )
        .await?;
        for t in txns {
            ids.insert(t.id);
        }
    }

    Ok(ids)
}

async fn portfolio_ids_by_name(
    pool: &SqlitePool,
    user_id: i64,
    name: &str,
) -> Result<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM portfolios WHERE user_id = ? AND name = ? AND status = 'active'",
    )
    .bind(user_id)
    .bind(name)
    .fetch_all(pool)
    .await?)
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

    let label = get(pool, user_id, res.last_insert_rowid()).await?;
    link_portfolios_for_label_name(pool, user_id, &label).await?;
    label_with_counts(pool, user_id, label.id).await
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

pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<DeleteLabelResult> {
    let label = get(pool, user_id, id).await?;
    delete_by_name(pool, user_id, &label.name).await
}

/// Delete every portfolio and the label that share `name` (label = portfolio).
pub async fn delete_by_name(pool: &SqlitePool, user_id: i64, name: &str) -> Result<DeleteLabelResult> {
    let txn_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions t
         INNER JOIN portfolios p ON p.id = t.portfolio_id
         WHERE t.user_id = ? AND p.user_id = ? AND p.name = ?",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?;

    let pids = portfolio_ids_by_name(pool, user_id, name).await?;
    let portfolios_deleted = pids.len();
    for pid in pids {
        portfolios::delete(pool, user_id, pid).await?;
    }

    sqlx::query("DELETE FROM labels WHERE user_id = ? AND name = ?")
        .bind(user_id)
        .bind(name)
        .execute(pool)
        .await?;

    Ok(DeleteLabelResult {
        transactions_deleted: txn_count as usize,
        portfolios_deleted,
    })
}

/// Ensure a label exists for `portfolio_id` and link them (portfolio create path).
pub async fn ensure_label_for_portfolio(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
    name: &str,
) -> Result<()> {
    let label_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM labels WHERE user_id = ? AND name = ?",
    )
    .bind(user_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;

    let label_id = if let Some(id) = label_id {
        id
    } else {
        let now = Utc::now().timestamp();
        let res = sqlx::query(
            "INSERT INTO labels (user_id, name, color, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(name)
        .bind(None::<&str>)
        .bind(now)
        .execute(pool)
        .await?;
        res.last_insert_rowid()
    };

    attach(
        pool,
        user_id,
        label_id,
        LabelEntityType::Portfolio,
        &portfolio_id.to_string(),
    )
    .await
}

async fn link_portfolios_for_label_name(
    pool: &SqlitePool,
    user_id: i64,
    label: &Label,
) -> Result<()> {
    let mut pids = portfolio_ids_by_name(pool, user_id, &label.name).await?;
    if pids.is_empty() {
        let p = portfolios::create(
            pool,
            user_id,
            &NewPortfolio {
                name: label.name.clone(),
                description: None,
                base_currency: Some("INR".into()),
                portfolio_type: Some("mixed".into()),
            },
        )
        .await?;
        pids.push(p.id);
    }
    for pid in pids {
        attach(
            pool,
            user_id,
            label.id,
            LabelEntityType::Portfolio,
            &pid.to_string(),
        )
        .await?;
    }
    Ok(())
}

async fn label_with_counts(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Label> {
    let list = list(pool, user_id).await?;
    list.into_iter()
        .find(|l| l.id == id)
        .ok_or(Error::NotFound)
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
        transaction_count: row.try_get("transaction_count").unwrap_or(0),
        holding_count: row.try_get("holding_count").unwrap_or(0),
        portfolio_count: row.try_get("portfolio_count").unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ensure_local_user;
    use crate::models::{NewPortfolio, NewTransaction, TransactionType};
    use crate::portfolios;
    use crate::transactions;

    #[test]
    fn parse_holding_entity_id_handles_mf_symbol() {
        let (pid, sym) = parse_holding_entity_id("3:MF:145552").unwrap();
        assert_eq!(pid, 3);
        assert_eq!(sym, "MF:145552");
    }

    #[tokio::test]
    async fn delete_label_removes_portfolio_transactions_by_name() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "P".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let label = create(
            &pool,
            user.id,
            &NewLabel {
                name: "P".into(),
                color: None,
            },
        )
        .await
        .unwrap();

        let txn = transactions::create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: Some(100.0),
                gross_amount: Some(1000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(1000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let result = delete(&pool, user.id, label.id).await.unwrap();
        assert_eq!(result.transactions_deleted, 1);
        assert_eq!(result.portfolios_deleted, 1);

        let labels_left = list(&pool, user.id).await.unwrap();
        assert!(labels_left.is_empty());

        let txn_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE id = ?")
                .bind(txn.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(txn_count, 0);
    }

    #[tokio::test]
    async fn delete_label_removes_transactions_for_tagged_holding() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "MF".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let label = create(
            &pool,
            user.id,
            &NewLabel {
                name: "MF".into(),
                color: None,
            },
        )
        .await
        .unwrap();

        let symbol = "MF:145552".to_string();
        let txn = transactions::create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some(symbol.clone()),
                quantity: Some(10.0),
                price: Some(100.0),
                gross_amount: Some(1000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(1000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        attach(
            &pool,
            user.id,
            label.id,
            LabelEntityType::Holding,
            &format!("{}:{symbol}", portfolio.id),
        )
        .await
        .unwrap();

        let listed = list(&pool, user.id).await.unwrap();
        assert_eq!(listed[0].transaction_count, 1);
        assert_eq!(listed[0].holding_count, 1);

        let result = delete(&pool, user.id, label.id).await.unwrap();
        assert_eq!(result.transactions_deleted, 1);

        let txn_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE id = ?")
                .bind(txn.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(txn_count, 0);
    }

    #[tokio::test]
    async fn delete_label_removes_transactions_for_tagged_portfolio() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "Shivendra".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let label = create(
            &pool,
            user.id,
            &NewLabel {
                name: "Shivendra".into(),
                color: None,
            },
        )
        .await
        .unwrap();

        let txn = transactions::create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("ITC.NS".into()),
                quantity: Some(10.0),
                price: Some(100.0),
                gross_amount: Some(1000.0),
                brokerage: None,
                taxes: None,
                net_amount: Some(1000.0),
                split_ratio_num: None,
                split_ratio_den: None,
                bonus_ratio_num: None,
                bonus_ratio_den: None,
                dividend_per_share: None,
                tds: None,
                eligible_quantity: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        attach(
            &pool,
            user.id,
            label.id,
            LabelEntityType::Portfolio,
            &portfolio.id.to_string(),
        )
        .await
        .unwrap();

        let listed = list(&pool, user.id).await.unwrap();
        assert_eq!(listed[0].portfolio_count, 1);
        assert_eq!(listed[0].transaction_count, 1);

        let result = delete(&pool, user.id, label.id).await.unwrap();
        assert_eq!(result.transactions_deleted, 1);

        let txn_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE id = ?")
                .bind(txn.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(txn_count, 0);
    }
}
