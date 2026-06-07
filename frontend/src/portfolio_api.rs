//! Portfolio API client (HTTP to stocker-api on web; in-process on desktop).

use serde::{Deserialize, Serialize};

#[cfg(all(feature = "web", not(feature = "desktop")))]
use crate::api::API_BASE;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Portfolio {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub base_currency: String,
    pub portfolio_type: String,
    pub status: PortfolioStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPortfolio {
    pub name: String,
    pub description: Option<String>,
    pub base_currency: Option<String>,
    pub portfolio_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePortfolio {
    pub name: Option<String>,
    pub description: Option<String>,
    pub portfolio_type: Option<String>,
    pub status: Option<PortfolioStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Label {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLabel {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    OpeningBalance,
    Buy,
    Sell,
    Dividend,
    Split,
    Bonus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub id: i64,
    pub user_id: i64,
    pub portfolio_id: i64,
    pub txn_type: TransactionType,
    pub trade_date: String,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub gross_amount: Option<f64>,
    pub brokerage: Option<f64>,
    pub taxes: Option<f64>,
    pub net_amount: Option<f64>,
    pub split_ratio_num: Option<f64>,
    pub split_ratio_den: Option<f64>,
    pub bonus_ratio_num: Option<f64>,
    pub bonus_ratio_den: Option<f64>,
    pub dividend_per_share: Option<f64>,
    pub tds: Option<f64>,
    pub eligible_quantity: Option<f64>,
    pub notes: Option<String>,
    pub source: String,
    pub corporate_action_key: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTransaction {
    pub portfolio_id: i64,
    pub txn_type: TransactionType,
    pub trade_date: String,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub gross_amount: Option<f64>,
    pub brokerage: Option<f64>,
    pub taxes: Option<f64>,
    pub net_amount: Option<f64>,
    pub split_ratio_num: Option<f64>,
    pub split_ratio_den: Option<f64>,
    pub bonus_ratio_num: Option<f64>,
    pub bonus_ratio_den: Option<f64>,
    pub dividend_per_share: Option<f64>,
    pub tds: Option<f64>,
    pub eligible_quantity: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Holding {
    pub symbol: String,
    pub quantity: f64,
    pub average_cost: f64,
    pub total_cost: f64,
    pub current_price: Option<f64>,
    pub current_value: Option<f64>,
    pub unrealized_gain: Option<f64>,
    pub unrealized_gain_pct: Option<f64>,
    pub realized_gain: f64,
    pub dividend_received: f64,
    pub total_return: Option<f64>,
    pub total_return_pct: Option<f64>,
    pub portfolio_weight: Option<f64>,
    pub last_transaction_date: Option<String>,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortfolioSummary {
    pub portfolio_id: i64,
    pub invested_amount: f64,
    pub current_market_value: f64,
    pub unrealized_gain: f64,
    pub unrealized_gain_pct: f64,
    pub realized_gain: f64,
    pub dividend_received: f64,
    pub total_return: f64,
    pub total_return_pct: f64,
    pub holdings_count: usize,
    pub rebuilt_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AllocationRow {
    pub key: String,
    pub label: String,
    pub current_value: f64,
    pub invested_amount: f64,
    pub weight_pct: f64,
    pub unrealized_gain: f64,
    pub realized_gain: f64,
    pub dividend_received: f64,
    pub total_return: f64,
    pub holdings_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dashboard {
    pub portfolio: Portfolio,
    pub summary: PortfolioSummary,
    pub top_holdings: Vec<Holding>,
    pub recent_transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FifoLot {
    pub id: i64,
    pub portfolio_id: i64,
    pub symbol: String,
    pub source_transaction_id: i64,
    pub acquired_date: String,
    pub original_quantity: f64,
    pub remaining_quantity: f64,
    pub total_cost: f64,
    pub cost_per_share: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionFilter {
    pub portfolio_id: Option<i64>,
    pub symbol: Option<String>,
    pub txn_type: Option<TransactionType>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub label_id: Option<i64>,
    pub limit: Option<i64>,
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
async fn api_request(
    method: &str,
    path: &str,
    body: Option<&impl Serialize>,
) -> Result<String, String> {
    let url = format!("{}{}", API_BASE, path);
    let builder = match method {
        "GET" => gloo_net::http::Request::get(&url),
        "POST" => gloo_net::http::Request::post(&url),
        "PUT" => gloo_net::http::Request::put(&url),
        "DELETE" => gloo_net::http::Request::delete(&url),
        _ => return Err(format!("unsupported method {method}")),
    };
    let resp = if let Some(b) = body {
        builder
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(b).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
    } else {
        builder.send().await
    }
    .map_err(|e| e.to_string())?;
    parse_http_response(resp.status(), resp.text().await.map_err(|e| e.to_string())?)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn parse_http_response(status: u16, text: String) -> Result<String, String> {
    if !(200..300).contains(&status) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                return Err(err.to_string());
            }
        }
        return Err(format!("HTTP {status}: {text}"));
    }
    Ok(text)
}
#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn list_portfolios(include_archived: bool) -> Result<Vec<Portfolio>, String> {
    let path = format!(
        "/api/v1/portfolio/portfolios?include_archived={}",
        include_archived
    );
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["portfolios"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn create_portfolio(input: &NewPortfolio) -> Result<Portfolio, String> {
    let text = api_request("POST", "/api/v1/portfolio/portfolios", Some(input)).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn update_portfolio(id: i64, input: &UpdatePortfolio) -> Result<Portfolio, String> {
    let path = format!("/api/v1/portfolio/portfolios/{id}");
    let text = api_request("PUT", &path, Some(input)).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn delete_portfolio(id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/portfolios/{id}");
    api_request("DELETE", &path, None::<&()>).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn list_labels() -> Result<Vec<Label>, String> {
    let text = api_request("GET", "/api/v1/portfolio/labels", None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["labels"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn create_label(input: &NewLabel) -> Result<Label, String> {
    let text = api_request("POST", "/api/v1/portfolio/labels", Some(input)).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn delete_label(id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/labels/{id}");
    api_request("DELETE", &path, None::<&()>).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn attach_label(label_id: i64, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "label_id": label_id,
        "entity_type": entity_type,
        "entity_id": entity_id,
    });
    api_request("POST", "/api/v1/portfolio/labels/attach", Some(&body)).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn list_transactions(filter: &TransactionFilter) -> Result<Vec<Transaction>, String> {
    let mut qs = vec![];
    if let Some(v) = filter.portfolio_id {
        qs.push(format!("portfolio_id={v}"));
    }
    if let Some(ref v) = filter.symbol {
        qs.push(format!("symbol={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = filter.txn_type {
        let t = serde_json::to_string(v).unwrap_or_default().trim_matches('"').to_string();
        qs.push(format!("txn_type={t}"));
    }
    if let Some(ref v) = filter.from_date {
        qs.push(format!("from_date={v}"));
    }
    if let Some(ref v) = filter.to_date {
        qs.push(format!("to_date={v}"));
    }
    if let Some(v) = filter.limit {
        qs.push(format!("limit={v}"));
    }
    let path = if qs.is_empty() {
        "/api/v1/portfolio/transactions".to_string()
    } else {
        format!("/api/v1/portfolio/transactions?{}", qs.join("&"))
    };
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["transactions"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn create_transaction(input: &NewTransaction) -> Result<Transaction, String> {
    let text = api_request("POST", "/api/v1/portfolio/transactions", Some(input)).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn delete_transaction(id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/transactions/{id}");
    api_request("DELETE", &path, None::<&()>).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn dashboard(portfolio_id: i64) -> Result<Dashboard, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/dashboard");
    let text = api_request("GET", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn holdings(portfolio_id: i64) -> Result<Vec<Holding>, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/holdings");
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["holdings"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn allocation_stock(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/allocation/stock");
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["allocation"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn allocation_label(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/allocation/label");
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["allocation"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn rebuild_portfolio(portfolio_id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/rebuild");
    api_request("POST", &path, None::<&()>).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn fifo_lots(portfolio_id: i64, symbol: &str) -> Result<Vec<FifoLot>, String> {
    let path = format!(
        "/api/v1/portfolio/portfolios/{portfolio_id}/stock/{}/lots",
        urlencoding::encode(symbol)
    );
    let text = api_request("GET", &path, None::<&()>).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["lots"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub fn export_holdings_url(portfolio_id: i64) -> String {
    format!(
        "{}/api/v1/portfolio/portfolios/{portfolio_id}/export/holdings.csv",
        API_BASE
    )
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub fn export_transactions_url(portfolio_id: i64) -> String {
    format!(
        "{}/api/v1/portfolio/portfolios/{portfolio_id}/export/transactions.csv",
        API_BASE
    )
}

pub fn fmt_inr(v: f64) -> String {
    if v.abs() >= 100_000.0 {
        format!("₹{:.2}L", v / 100_000.0)
    } else {
        format!("₹{:.0}", v)
    }
}

pub fn fmt_pct(v: f64) -> String {
    format!("{v:.2}%")
}

pub fn txn_type_label(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::OpeningBalance => "Opening Balance",
        TransactionType::Buy => "Buy",
        TransactionType::Sell => "Sell",
        TransactionType::Dividend => "Dividend",
        TransactionType::Split => "Split",
        TransactionType::Bonus => "Bonus",
    }
}

#[cfg(all(feature = "desktop", not(feature = "web")))]
#[path = "portfolio_api_desktop.rs"]
mod desktop;

#[cfg(all(feature = "desktop", not(feature = "web")))]
pub use desktop::*;
