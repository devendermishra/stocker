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
    #[serde(default)]
    pub transaction_count: i64,
    #[serde(default)]
    pub holding_count: i64,
    #[serde(default)]
    pub portfolio_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteLabelResult {
    pub transactions_deleted: usize,
    #[serde(default)]
    pub portfolios_deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearTransactionsResult {
    pub transactions_deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLabel {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    OpeningBalance,
    Buy,
    MergerInvestment,
    DemergerInvestment,
    MergerRedemption,
    DemergerRedemption,
    Sell,
    Dividend,
    Split,
    Bonus,
    Rights,
    Sip,
    Swp,
}

impl TransactionType {
    pub fn from_form_value(s: &str) -> Self {
        match s {
            "opening_balance" => Self::OpeningBalance,
            "merger_investment" => Self::MergerInvestment,
            "demerger_investment" => Self::DemergerInvestment,
            "merger_redemption" => Self::MergerRedemption,
            "demerger_redemption" => Self::DemergerRedemption,
            "sell" => Self::Sell,
            "dividend" => Self::Dividend,
            "split" => Self::Split,
            "bonus" => Self::Bonus,
            "rights" => Self::Rights,
            "sip" => Self::Sip,
            "swp" => Self::Swp,
            _ => Self::Buy,
        }
    }

    pub fn is_schedule_type(self) -> bool {
        matches!(self, Self::Sip | Self::Swp)
    }

    pub fn form_value(self) -> &'static str {
        match self {
            Self::OpeningBalance => "opening_balance",
            Self::Buy => "buy",
            Self::MergerInvestment => "merger_investment",
            Self::DemergerInvestment => "demerger_investment",
            Self::MergerRedemption => "merger_redemption",
            Self::DemergerRedemption => "demerger_redemption",
            Self::Sell => "sell",
            Self::Dividend => "dividend",
            Self::Split => "split",
            Self::Bonus => "bonus",
            Self::Rights => "rights",
            Self::Sip => "sip",
            Self::Swp => "swp",
        }
    }

    pub fn requires_qty_price(self) -> bool {
        matches!(
            self,
            Self::Buy
                | Self::Sell
                | Self::OpeningBalance
                | Self::MergerInvestment
                | Self::DemergerInvestment
                | Self::MergerRedemption
                | Self::DemergerRedemption
                | Self::Rights
        )
    }
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
    #[serde(default)]
    pub schedule_id: Option<i64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleType {
    Sip,
    Swp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStatus {
    Active,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MfSchedule {
    pub id: i64,
    pub user_id: i64,
    pub portfolio_id: i64,
    pub schedule_type: ScheduleType,
    pub symbol: String,
    pub scheme_name: Option<String>,
    pub amount: f64,
    pub start_date: String,
    pub end_date: Option<String>,
    pub installment_count: Option<i32>,
    pub sip_day: i32,
    pub status: ScheduleStatus,
    pub registered_installments: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMfSchedule {
    pub schedule_type: ScheduleType,
    pub symbol: String,
    pub amount: f64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub installment_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleFailure {
    pub trade_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMfScheduleResult {
    pub schedule_id: i64,
    pub registered: Vec<i64>,
    pub materialized: Vec<i64>,
    pub skipped_months: Vec<String>,
    pub status: ScheduleStatus,
    pub failed: Vec<ScheduleFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwpRefreshFailure {
    pub swp_id: i64,
    pub symbol: Option<String>,
    pub trade_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwpRefreshResult {
    pub created: Vec<i64>,
    pub skipped: Vec<i64>,
    pub failed: Vec<SwpRefreshFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSwpMaterialization {
    pub suggestion_id: String,
    pub swp_id: i64,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedSwpInstallment {
    pub suggestion_id: String,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
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
    pub return_method: Option<String>,
    pub portfolio_weight: Option<f64>,
    pub last_transaction_date: Option<String>,
    pub short_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub exchange: Option<String>,
    #[serde(default)]
    pub asset_class: Option<String>,
    #[serde(default)]
    pub nav_date: Option<String>,
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MfSearchHit {
    pub scheme_code: i64,
    pub scheme_name: String,
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
    pub return_method: Option<String>,
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
    /// `"equity"` or `"mutual_fund"`
    pub asset_class: Option<String>,
    pub schedule_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportField {
    Skip,
    TxnType,
    TradeDate,
    Symbol,
    StockName,
    Isin,
    Quantity,
    Price,
    GrossAmount,
    Brokerage,
    Taxes,
    NetAmount,
    SplitRatioNum,
    SplitRatioDen,
    BonusRatioNum,
    BonusRatioDen,
    DividendPerShare,
    Tds,
    EligibleQuantity,
    Notes,
}

impl ImportField {
    pub fn label(self) -> &'static str {
        match self {
            ImportField::Skip => "— Skip —",
            ImportField::TxnType => "Transaction Type",
            ImportField::TradeDate => "Trade Date",
            ImportField::Symbol => "Symbol",
            ImportField::StockName => "Stock / MF / ETF Name",
            ImportField::Isin => "ISIN",
            ImportField::Quantity => "Quantity",
            ImportField::Price => "Price",
            ImportField::GrossAmount => "Amount",
            ImportField::Brokerage => "Brokerage",
            ImportField::Taxes => "Taxes",
            ImportField::NetAmount => "Net Amount",
            ImportField::SplitRatioNum => "Split Ratio (num)",
            ImportField::SplitRatioDen => "Split Ratio (den)",
            ImportField::BonusRatioNum => "Bonus Ratio (num)",
            ImportField::BonusRatioDen => "Bonus Ratio (den)",
            ImportField::DividendPerShare => "Dividend / Share",
            ImportField::Tds => "TDS",
            ImportField::EligibleQuantity => "Eligible Quantity",
            ImportField::Notes => "Notes",
        }
    }

    pub fn api_key(self) -> &'static str {
        match self {
            ImportField::Skip => "skip",
            ImportField::TxnType => "txn_type",
            ImportField::TradeDate => "trade_date",
            ImportField::Symbol => "symbol",
            ImportField::StockName => "stock_name",
            ImportField::Isin => "isin",
            ImportField::Quantity => "quantity",
            ImportField::Price => "price",
            ImportField::GrossAmount => "gross_amount",
            ImportField::Brokerage => "brokerage",
            ImportField::Taxes => "taxes",
            ImportField::NetAmount => "net_amount",
            ImportField::SplitRatioNum => "split_ratio_num",
            ImportField::SplitRatioDen => "split_ratio_den",
            ImportField::BonusRatioNum => "bonus_ratio_num",
            ImportField::BonusRatioDen => "bonus_ratio_den",
            ImportField::DividendPerShare => "dividend_per_share",
            ImportField::Tds => "tds",
            ImportField::EligibleQuantity => "eligible_quantity",
            ImportField::Notes => "notes",
        }
    }

    pub fn parse_api_key(s: &str) -> Self {
        match s {
            "txn_type" => ImportField::TxnType,
            "trade_date" => ImportField::TradeDate,
            "symbol" => ImportField::Symbol,
            "stock_name" => ImportField::StockName,
            "isin" => ImportField::Isin,
            "quantity" => ImportField::Quantity,
            "price" => ImportField::Price,
            "gross_amount" => ImportField::GrossAmount,
            "brokerage" => ImportField::Brokerage,
            "taxes" => ImportField::Taxes,
            "net_amount" => ImportField::NetAmount,
            "split_ratio_num" => ImportField::SplitRatioNum,
            "split_ratio_den" => ImportField::SplitRatioDen,
            "bonus_ratio_num" => ImportField::BonusRatioNum,
            "bonus_ratio_den" => ImportField::BonusRatioDen,
            "dividend_per_share" => ImportField::DividendPerShare,
            "tds" => ImportField::Tds,
            "eligible_quantity" => ImportField::EligibleQuantity,
            "notes" => ImportField::Notes,
            _ => ImportField::Skip,
        }
    }

    pub fn all_mappable() -> &'static [ImportField] {
        &[
            ImportField::Skip,
            ImportField::TxnType,
            ImportField::TradeDate,
            ImportField::Symbol,
            ImportField::StockName,
            ImportField::Isin,
            ImportField::Quantity,
            ImportField::Price,
            ImportField::GrossAmount,
            ImportField::Brokerage,
            ImportField::Taxes,
            ImportField::NetAmount,
            ImportField::SplitRatioNum,
            ImportField::SplitRatioDen,
            ImportField::BonusRatioNum,
            ImportField::BonusRatioDen,
            ImportField::DividendPerShare,
            ImportField::Tds,
            ImportField::EligibleQuantity,
            ImportField::Notes,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGrid {
    pub rows: Vec<Vec<String>>,
    pub sheet_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsePreview {
    pub grid: RawGrid,
    pub suggested_header_row: usize,
    pub suggested_mapping: Vec<ImportField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowPreview {
    pub row_index: usize,
    pub transaction: Option<NewTransaction>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportApplyRequest {
    pub header_row: usize,
    pub column_mapping: Vec<ImportField>,
    pub grid: RawGrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<ImportRowError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row_index: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipRefreshFailure {
    pub sip_id: i64,
    pub symbol: Option<String>,
    pub trade_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipRefreshResult {
    pub created: Vec<i64>,
    pub skipped: Vec<i64>,
    pub failed: Vec<SipRefreshFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanError {
    pub symbol: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedCorporateAction {
    pub suggestion_id: String,
    pub symbol: String,
    pub txn_type: String,
    pub trade_date: String,
    pub dividend_per_share: Option<f64>,
    pub eligible_quantity: Option<f64>,
    pub gross_amount: Option<f64>,
    pub split_ratio_num: Option<f64>,
    pub split_ratio_den: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSipMaterialization {
    pub suggestion_id: String,
    pub sip_id: i64,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedSipInstallment {
    pub suggestion_id: String,
    pub symbol: String,
    pub trade_date: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRefreshScan {
    pub corporate_actions: Vec<SuggestedCorporateAction>,
    pub sip_pending: Vec<PendingSipMaterialization>,
    pub sip_suggested: Vec<SuggestedSipInstallment>,
    pub swp_pending: Vec<PendingSwpMaterialization>,
    pub swp_suggested: Vec<SuggestedSwpInstallment>,
    pub scan_errors: Vec<ScanError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRefreshApplyResult {
    pub corporate_actions_created: usize,
    pub sip_registered: usize,
    pub sip_materialized: usize,
    pub swp_registered: usize,
    pub swp_materialized: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRefreshApplyRequest {
    pub selections: Vec<String>,
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
pub async fn search_mutual_funds(query: &str) -> Result<Vec<MfSearchHit>, String> {
    let path = format!(
        "/api/v1/portfolio/mf/search?q={}",
        urlencoding::encode(query)
    );
    let text = api_request("GET", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
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
pub async fn delete_portfolio(id: i64) -> Result<DeleteLabelResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{id}");
    let text = api_request("DELETE", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
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
pub async fn delete_label(id: i64) -> Result<DeleteLabelResult, String> {
    let path = format!("/api/v1/portfolio/labels/{id}");
    let text = api_request("DELETE", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
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
pub async fn detach_label(label_id: i64, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "label_id": label_id,
        "entity_type": entity_type,
        "entity_id": entity_id,
    });
    api_request("POST", "/api/v1/portfolio/labels/detach", Some(&body)).await?;
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
    if let Some(ref v) = filter.asset_class {
        qs.push(format!("asset_class={}", urlencoding::encode(v)));
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
    let txn: Transaction = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(txn)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn update_transaction(id: i64, input: &NewTransaction) -> Result<Transaction, String> {
    let path = format!("/api/v1/portfolio/transactions/{id}");
    let text = api_request("PUT", &path, Some(input)).await?;
    let txn: Transaction = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(txn)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn delete_transaction(id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/transactions/{id}");
    api_request("DELETE", &path, None::<&()>).await?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn clear_portfolio_transactions(portfolio_id: i64) -> Result<ClearTransactionsResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/transactions/clear");
    let text = api_request("POST", &path, None::<&()>).await?;
    let result: ClearTransactionsResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(result)
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
    crate::portfolio_data_revision::bump_portfolio_data_revision();
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn refresh_prices(portfolio_id: i64) -> Result<(), String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/refresh-prices");
    api_request("POST", &path, None::<&()>).await?;
    Ok(())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn refresh_sip_transactions(portfolio_id: i64) -> Result<SipRefreshResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/sip/refresh");
    let text = api_request("POST", &path, None::<&()>).await?;
    let result: SipRefreshResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !result.created.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn scan_portfolio_refresh(portfolio_id: i64) -> Result<PortfolioRefreshScan, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/refresh/scan");
    let text = api_request("POST", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn apply_portfolio_refresh(
    portfolio_id: i64,
    selections: &[String],
) -> Result<PortfolioRefreshApplyResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/refresh/apply");
    let body = PortfolioRefreshApplyRequest {
        selections: selections.to_vec(),
    };
    let text = api_request("POST", &path, Some(&body)).await?;
    let result: PortfolioRefreshApplyResult =
        serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if result.corporate_actions_created > 0
        || result.sip_registered > 0
        || result.sip_materialized > 0
        || result.swp_registered > 0
        || result.swp_materialized > 0
    {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn refresh_swp_transactions(portfolio_id: i64) -> Result<SwpRefreshResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/swp/refresh");
    let text = api_request("POST", &path, None::<&()>).await?;
    let result: SwpRefreshResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !result.created.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn register_mf_schedule(
    portfolio_id: i64,
    input: &RegisterMfSchedule,
) -> Result<RegisterMfScheduleResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/mf-schedule");
    let text = api_request("POST", &path, Some(input)).await?;
    let result: RegisterMfScheduleResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !result.registered.is_empty() || !result.materialized.is_empty() {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn list_mf_schedules(portfolio_id: i64) -> Result<Vec<MfSchedule>, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/mf-schedules");
    let text = api_request("GET", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn inactivate_mf_schedule(schedule_id: i64) -> Result<MfSchedule, String> {
    let path = format!("/api/v1/portfolio/mf-schedules/{schedule_id}/inactivate");
    let text = api_request("POST", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn get_mf_scheme(scheme_code: i64) -> Result<MfSearchHit, String> {
    let path = format!("/api/v1/portfolio/mf/schemes/{scheme_code}");
    let text = api_request("GET", &path, None::<&()>).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
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

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn parse_import_file(
    portfolio_id: i64,
    filename: &str,
    bytes: &[u8],
) -> Result<ParsePreview, String> {
    use base64::Engine;
    let body = serde_json::json!({
        "filename": filename,
        "data_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
    });
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/import/parse-json");
    let text = api_request("POST", &path, Some(&body)).await?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn preview_import(
    portfolio_id: i64,
    request: &ImportApplyRequest,
) -> Result<Vec<ImportRowPreview>, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/import/preview");
    let text = api_request("POST", &path, Some(request)).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    serde_json::from_value(v["rows"].clone()).map_err(|e| e.to_string())
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn apply_import(
    portfolio_id: i64,
    request: &ImportApplyRequest,
) -> Result<ImportResult, String> {
    let path = format!("/api/v1/portfolio/portfolios/{portfolio_id}/import");
    let text = api_request("POST", &path, Some(request)).await?;
    let result: ImportResult = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if result.imported > 0 {
        crate::portfolio_data_revision::bump_portfolio_data_revision();
    }
    Ok(result)
}

pub fn fmt_inr(v: f64) -> String {
    let sign = if v < 0.0 { "-" } else { "" };
    let abs = v.abs();
    if abs >= 100_000.0 {
        format!("{sign}₹{:.2}L", abs / 100_000.0)
    } else {
        format!("{sign}₹{:.0}", abs)
    }
}

pub fn fmt_pct(v: f64) -> String {
    format!("{v:.2}%")
}

pub fn fmt_return_pct(pct: Option<f64>, method: Option<&str>) -> String {
    let Some(pct) = pct else {
        return String::new();
    };
    let label = match method {
        Some("xirr") => "XIRR",
        Some("cagr") => "CAGR",
        Some("simple") => "Return",
        _ => "Return",
    };
    format!("{label} {}", fmt_pct(pct))
}

pub fn txn_type_label(t: &TransactionType) -> &'static str {
    match t {
        TransactionType::OpeningBalance => "Opening Balance",
        TransactionType::Buy => "Buy",
        TransactionType::MergerInvestment => "Merger Investment",
        TransactionType::DemergerInvestment => "Demerger Investment",
        TransactionType::MergerRedemption => "Merger Redemption",
        TransactionType::DemergerRedemption => "Demerger Redemption",
        TransactionType::Sell => "Sell",
        TransactionType::Dividend => "Dividend",
        TransactionType::Split => "Splits",
        TransactionType::Bonus => "Bonus",
        TransactionType::Rights => "Rights",
        TransactionType::Sip => "SIP Investment",
        TransactionType::Swp => "SWP Withdrawal",
    }
}

#[cfg(all(feature = "desktop", not(feature = "web")))]
#[path = "portfolio_api_desktop.rs"]
mod desktop;

#[cfg(all(feature = "desktop", not(feature = "web")))]
pub use desktop::*;
