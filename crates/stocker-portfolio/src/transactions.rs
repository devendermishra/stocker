//! Transaction CRUD — source of truth for the portfolio ledger.

use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::engine::rebuild;
use crate::error::{Error, Result};
use crate::models::{NewTransaction, Transaction, TransactionType};
use crate::portfolios;

const TXN_SELECT: &str = "id, user_id, portfolio_id, txn_type, trade_date, symbol, quantity, price,
         gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity,
         notes, source, corporate_action_key, created_at, updated_at";

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct TransactionFilter {
    pub portfolio_id: Option<i64>,
    pub symbol: Option<String>,
    pub txn_type: Option<TransactionType>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub label_id: Option<i64>,
    pub limit: Option<i64>,
    /// `"equity"` or `"mutual_fund"`
    pub asset_class: Option<String>,
}

pub async fn list(
    pool: &SqlitePool,
    user_id: i64,
    filter: &TransactionFilter,
) -> Result<Vec<Transaction>> {
    let mut sql = String::from(
        "SELECT t.id, t.user_id, t.portfolio_id, t.txn_type, t.trade_date, t.symbol, t.quantity,
         t.price, t.gross_amount, t.brokerage, t.taxes, t.net_amount, t.split_ratio_num,
         t.split_ratio_den, t.bonus_ratio_num, t.bonus_ratio_den, t.dividend_per_share, t.tds,
         t.eligible_quantity, t.notes, t.source, t.corporate_action_key, t.created_at, t.updated_at
         FROM transactions t WHERE t.user_id = ?",
    );
    if filter.portfolio_id.is_some() {
        sql.push_str(" AND t.portfolio_id = ?");
    }
    if filter.symbol.is_some() {
        sql.push_str(" AND t.symbol = ?");
    }
    if filter.txn_type.is_some() {
        sql.push_str(" AND t.txn_type = ?");
    }
    if filter.from_date.is_some() {
        sql.push_str(" AND t.trade_date >= ?");
    }
    if filter.to_date.is_some() {
        sql.push_str(" AND t.trade_date <= ?");
    }
    if filter.label_id.is_some() {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM label_links ll WHERE ll.entity_type = 'transaction'
             AND ll.entity_id = CAST(t.id AS TEXT) AND ll.label_id = ?)",
        );
    }
    if filter.asset_class.as_deref() == Some("mutual_fund") {
        sql.push_str(" AND t.symbol LIKE 'MF:%'");
    } else if filter.asset_class.as_deref() == Some("equity") {
        sql.push_str(" AND (t.symbol NOT LIKE 'MF:%' OR t.symbol IS NULL)");
    }
    sql.push_str(" ORDER BY t.trade_date DESC, t.id DESC");
    if filter.limit.is_some() {
        sql.push_str(" LIMIT ?");
    }

    let mut q = sqlx::query(&sql).bind(user_id);
    if let Some(v) = &filter.portfolio_id {
        q = q.bind(v);
    }
    if let Some(v) = &filter.symbol {
        q = q.bind(v);
    }
    if let Some(v) = &filter.txn_type {
        q = q.bind(v.as_str());
    }
    if let Some(v) = &filter.from_date {
        q = q.bind(v);
    }
    if let Some(v) = &filter.to_date {
        q = q.bind(v);
    }
    if let Some(v) = &filter.label_id {
        q = q.bind(v);
    }
    if let Some(v) = &filter.limit {
        q = q.bind(v);
    }

    let rows = q.fetch_all(pool).await?;
    rows.iter().map(row_to_transaction).collect()
}

pub async fn get(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Transaction> {
    let row = sqlx::query(&format!(
        "SELECT {TXN_SELECT} FROM transactions WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::NotFound)?;
    row_to_transaction(&row)
}

pub async fn create(pool: &SqlitePool, user_id: i64, input: &NewTransaction) -> Result<Transaction> {
    let mut input = input.clone();
    normalize_trade_amounts(&mut input);
    validate_new(&input)?;
    let _ = portfolios::get(pool, user_id, input.portfolio_id).await?;
    let now = Utc::now().timestamp();
    let corp_key = corporate_action_key(&input);

    let res = sqlx::query(
        "INSERT INTO transactions (user_id, portfolio_id, txn_type, trade_date, symbol, quantity,
         price, gross_amount, brokerage, taxes, net_amount, split_ratio_num, split_ratio_den,
         bonus_ratio_num, bonus_ratio_den, dividend_per_share, tds, eligible_quantity, notes,
         source, corporate_action_key, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'manual', ?, ?, ?)",
    )
    .bind(user_id)
    .bind(input.portfolio_id)
    .bind(input.txn_type.as_str())
    .bind(&input.trade_date)
    .bind(input.symbol.as_deref())
    .bind(input.quantity)
    .bind(input.price)
    .bind(input.gross_amount)
    .bind(input.brokerage)
    .bind(input.taxes)
    .bind(input.net_amount)
    .bind(input.split_ratio_num)
    .bind(input.split_ratio_den)
    .bind(input.bonus_ratio_num)
    .bind(input.bonus_ratio_den)
    .bind(input.dividend_per_share)
    .bind(input.tds)
    .bind(input.eligible_quantity)
    .bind(input.notes.as_deref())
    .bind(corp_key.as_deref())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.is_unique_violation() {
                return Error::Conflict("corporate action already applied".into());
            }
        }
        Error::from(e)
    })?;

    let txn = get(pool, user_id, res.last_insert_rowid()).await?;
    rebuild::rebuild(pool, input.portfolio_id).await?;
    Ok(txn)
}

pub async fn update(
    pool: &SqlitePool,
    user_id: i64,
    id: i64,
    input: &NewTransaction,
) -> Result<Transaction> {
    let mut input = input.clone();
    normalize_trade_amounts(&mut input);
    validate_new(&input)?;
    let existing = get(pool, user_id, id).await?;
    let now = Utc::now().timestamp();
    let corp_key = corporate_action_key(&input);

    sqlx::query(
        "UPDATE transactions SET portfolio_id = ?, txn_type = ?, trade_date = ?, symbol = ?,
         quantity = ?, price = ?, gross_amount = ?, brokerage = ?, taxes = ?, net_amount = ?,
         split_ratio_num = ?, split_ratio_den = ?, bonus_ratio_num = ?, bonus_ratio_den = ?,
         dividend_per_share = ?, tds = ?, eligible_quantity = ?, notes = ?,
         corporate_action_key = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(input.portfolio_id)
    .bind(input.txn_type.as_str())
    .bind(&input.trade_date)
    .bind(input.symbol.as_deref())
    .bind(input.quantity)
    .bind(input.price)
    .bind(input.gross_amount)
    .bind(input.brokerage)
    .bind(input.taxes)
    .bind(input.net_amount)
    .bind(input.split_ratio_num)
    .bind(input.split_ratio_den)
    .bind(input.bonus_ratio_num)
    .bind(input.bonus_ratio_den)
    .bind(input.dividend_per_share)
    .bind(input.tds)
    .bind(input.eligible_quantity)
    .bind(input.notes.as_deref())
    .bind(corp_key.as_deref())
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await?;

    let portfolio_ids: Vec<i64> = [existing.portfolio_id, input.portfolio_id]
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    for pid in portfolio_ids {
        rebuild::rebuild(pool, pid).await?;
    }
    get(pool, user_id, id).await
}

pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<()> {
    let existing = get(pool, user_id, id).await?;
    let res = sqlx::query("DELETE FROM transactions WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    rebuild::rebuild(pool, existing.portfolio_id).await?;
    Ok(())
}

/// Remove every transaction in a portfolio and rebuild FIFO state.
pub async fn delete_all_for_portfolio(
    pool: &SqlitePool,
    user_id: i64,
    portfolio_id: i64,
) -> Result<usize> {
    let _ = portfolios::get(pool, user_id, portfolio_id).await?;
    let txn_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM transactions WHERE portfolio_id = ? AND user_id = ?",
    )
    .bind(portfolio_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    for txn_id in &txn_ids {
        sqlx::query(
            "DELETE FROM label_links WHERE entity_type = 'transaction' AND entity_id = ?",
        )
        .bind(txn_id.to_string())
        .execute(pool)
        .await?;
    }
    let res = sqlx::query("DELETE FROM transactions WHERE portfolio_id = ? AND user_id = ?")
        .bind(portfolio_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    rebuild::rebuild(pool, portfolio_id).await?;
    Ok(res.rows_affected() as usize)
}

pub async fn duplicate(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Transaction> {
    let existing = get(pool, user_id, id).await?;
    create(pool, user_id, &existing.into()).await
}

fn positive_amount(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.abs() > 0.0)
}

/// Derive net/gross from quantity × price when amount fields are missing.
pub fn normalize_trade_amounts(input: &mut NewTransaction) {
    let derived = positive_amount(input.quantity).and_then(|q| {
        positive_amount(input.price).map(|p| q * p)
    });
    let amount = positive_amount(input.net_amount)
        .or_else(|| positive_amount(input.gross_amount))
        .or(derived);
    if let Some(amount) = amount {
        if positive_amount(input.net_amount).is_none() {
            input.net_amount = Some(amount);
        }
        if positive_amount(input.gross_amount).is_none() {
            input.gross_amount = Some(amount);
        }
    }
}

pub fn validate_new(input: &NewTransaction) -> Result<()> {
    if input.trade_date.trim().is_empty() {
        return Err(Error::InvalidInput("trade_date is required".into()));
    }
    if input.txn_type.requires_symbol() && input.symbol.as_deref().unwrap_or("").is_empty() {
        return Err(Error::InvalidInput("symbol is required".into()));
    }
    if input.txn_type.requires_positive_quantity() {
        let qty = input.quantity.unwrap_or(0.0);
        if qty <= 0.0 {
            return Err(Error::InvalidInput("quantity must be positive".into()));
        }
    }
    if input.txn_type == TransactionType::Sip {
        let amount = input
            .net_amount
            .or(input.gross_amount)
            .filter(|a| *a > 0.0);
        if amount.is_none() {
            return Err(Error::InvalidInput(
                "sip requires net_amount or gross_amount".into(),
            ));
        }
    }
    Ok(())
}

pub fn corporate_action_key(input: &NewTransaction) -> Option<String> {
    match input.txn_type {
        TransactionType::Split => {
            let sym = input.symbol.as_deref()?;
            let num = input.split_ratio_num.filter(|n| *n > 0.0).or(input.quantity)?;
            let den = input.split_ratio_den.filter(|d| *d > 0.0).unwrap_or(1.0);
            Some(format!("split:{sym}:{}:{num}:{den}", input.trade_date))
        }
        TransactionType::Bonus => {
            let sym = input.symbol.as_deref()?;
            let marker = input
                .bonus_ratio_num
                .map(|n| n.to_string())
                .or_else(|| input.quantity.map(|q| format!("qty:{q}")))?;
            Some(format!("bonus:{sym}:{}:{marker}", input.trade_date))
        }
        TransactionType::Dividend => {
            let sym = input.symbol.as_deref()?;
            Some(format!("dividend:{sym}:{}", input.trade_date))
        }
        _ => None,
    }
}

fn row_to_transaction(row: &sqlx::sqlite::SqliteRow) -> Result<Transaction> {
    let txn_type_str: String = row.try_get("txn_type")?;
    let txn_type = TransactionType::parse(&txn_type_str)
        .ok_or_else(|| Error::Other(format!("unknown txn_type {txn_type_str}")))?;
    Ok(Transaction {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        portfolio_id: row.try_get("portfolio_id")?,
        txn_type,
        trade_date: row.try_get("trade_date")?,
        symbol: row.try_get("symbol")?,
        quantity: row.try_get("quantity")?,
        price: row.try_get("price")?,
        gross_amount: row.try_get("gross_amount")?,
        brokerage: row.try_get("brokerage")?,
        taxes: row.try_get("taxes")?,
        net_amount: row.try_get("net_amount")?,
        split_ratio_num: row.try_get("split_ratio_num")?,
        split_ratio_den: row.try_get("split_ratio_den")?,
        bonus_ratio_num: row.try_get("bonus_ratio_num")?,
        bonus_ratio_den: row.try_get("bonus_ratio_den")?,
        dividend_per_share: row.try_get("dividend_per_share")?,
        tds: row.try_get("tds")?,
        eligible_quantity: row.try_get("eligible_quantity")?,
        notes: row.try_get("notes")?,
        source: row.try_get("source")?,
        corporate_action_key: row.try_get("corporate_action_key")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        labels: vec![],
    })
}

pub fn export_csv(transactions: &[Transaction]) -> Result<String> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    for t in transactions {
        wtr.write_record([
            t.id.to_string(),
            t.trade_date.clone(),
            t.txn_type.as_str().to_string(),
            t.symbol.clone().unwrap_or_default(),
            t.quantity.map(|q| q.to_string()).unwrap_or_default(),
            t.price.map(|p| p.to_string()).unwrap_or_default(),
            t.net_amount.map(|n| n.to_string()).unwrap_or_default(),
            t.notes.clone().unwrap_or_default(),
        ])
        .map_err(|e| Error::Other(format!("csv write: {e}")))?;
    }
    let bytes = wtr.into_inner().map_err(|e| Error::Other(format!("csv: {e}")))?;
    String::from_utf8(bytes).map_err(|e| Error::Other(format!("csv utf8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ensure_local_user;
    use crate::engine::rebuild;
    use crate::models::{NewPortfolio, TransactionType};

    #[tokio::test]
    async fn update_transaction_rebuilds_holdings() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = crate::portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "Test".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let buy = create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("TEST.NS".into()),
                quantity: Some(10.0),
                price: Some(100.0),
                gross_amount: None,
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

        let before = rebuild::rebuild(&pool, portfolio.id).await.unwrap();
        assert_eq!(before.symbols.get("TEST.NS").unwrap().quantity, 10.0);

        update(
            &pool,
            user.id,
            buy.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("TEST.NS".into()),
                quantity: Some(20.0),
                price: Some(100.0),
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: Some(2000.0),
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

        let after = rebuild::rebuild(&pool, portfolio.id).await.unwrap();
        assert_eq!(after.symbols.get("TEST.NS").unwrap().quantity, 20.0);
    }

    #[test]
    fn normalize_trade_amounts_fills_from_quantity_and_price() {
        let mut input = NewTransaction {
            portfolio_id: 1,
            txn_type: TransactionType::Buy,
            trade_date: "2024-01-01".into(),
            symbol: Some("TEST.NS".into()),
            quantity: Some(10.0),
            price: Some(100.0),
            gross_amount: None,
            brokerage: None,
            taxes: None,
            net_amount: None,
            split_ratio_num: None,
            split_ratio_den: None,
            bonus_ratio_num: None,
            bonus_ratio_den: None,
            dividend_per_share: None,
            tds: None,
            eligible_quantity: None,
            notes: None,
        };
        normalize_trade_amounts(&mut input);
        assert_eq!(input.net_amount, Some(1000.0));
        assert_eq!(input.gross_amount, Some(1000.0));
    }

    #[tokio::test]
    async fn create_fills_missing_net_amount_from_quantity_and_price() {
        let pool = crate::db::open_memory().await.unwrap();
        let user = ensure_local_user(&pool).await.unwrap();
        let portfolio = crate::portfolios::create(
            &pool,
            user.id,
            &NewPortfolio {
                name: "Test".into(),
                description: None,
                base_currency: None,
                portfolio_type: None,
            },
        )
        .await
        .unwrap();

        let buy = create(
            &pool,
            user.id,
            &NewTransaction {
                portfolio_id: portfolio.id,
                txn_type: TransactionType::Buy,
                trade_date: "2024-01-01".into(),
                symbol: Some("TEST.NS".into()),
                quantity: Some(5.0),
                price: Some(200.0),
                gross_amount: None,
                brokerage: None,
                taxes: None,
                net_amount: None,
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

        assert_eq!(buy.net_amount, Some(1000.0));
        assert_eq!(buy.gross_amount, Some(1000.0));
    }
}
