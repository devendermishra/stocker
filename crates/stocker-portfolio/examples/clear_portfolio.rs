//! One-off: clear all transactions for a portfolio by id or name.
//! Usage: cargo run -p stocker-portfolio --example clear_portfolio -- 3
//!    or: cargo run -p stocker-portfolio --example clear_portfolio -- Shivendra

use stocker_portfolio::{auth::ensure_local_user, db, portfolios, transactions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("pass portfolio id or name");
    let path = db::default_db_path();
    eprintln!("Using DB: {}", path.display());
    let pool = db::open(&path).await?;
    let user = ensure_local_user(&pool).await?;

    let portfolio_id = if let Ok(id) = arg.parse::<i64>() {
        id
    } else {
        let list = portfolios::list(&pool, user.id, false).await?;
        list.into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(&arg))
            .map(|p| p.id)
            .unwrap_or_else(|| panic!("no portfolio named {arg}"))
    };

    let deleted =
        transactions::delete_all_for_portfolio(&pool, user.id, portfolio_id).await?;
    eprintln!("Deleted {deleted} transaction(s) from portfolio {portfolio_id}.");
    Ok(())
}
