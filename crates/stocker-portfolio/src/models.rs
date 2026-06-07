//! Domain models for the portfolio module.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioStatus {
    Active,
    Archived,
}

impl PortfolioStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub display_name: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelEntityType {
    Portfolio,
    Transaction,
    Holding,
}

impl LabelEntityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Portfolio => "portfolio",
            Self::Transaction => "transaction",
            Self::Holding => "holding",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "portfolio" => Some(Self::Portfolio),
            "transaction" => Some(Self::Transaction),
            "holding" => Some(Self::Holding),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    OpeningBalance,
    Buy,
    Sell,
    Dividend,
    Split,
    Bonus,
}

impl TransactionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpeningBalance => "opening_balance",
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Dividend => "dividend",
            Self::Split => "split",
            Self::Bonus => "bonus",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "opening_balance" => Some(Self::OpeningBalance),
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            "dividend" => Some(Self::Dividend),
            "split" => Some(Self::Split),
            "bonus" => Some(Self::Bonus),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealizedMatch {
    pub id: i64,
    pub portfolio_id: i64,
    pub sell_transaction_id: i64,
    pub buy_transaction_id: i64,
    pub symbol: String,
    pub quantity: f64,
    pub buy_date: String,
    pub sell_date: String,
    pub buy_cost_per_share: f64,
    pub sell_price: f64,
    pub cost_basis: f64,
    pub sell_value: f64,
    pub realized_gain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub portfolio: Portfolio,
    pub summary: PortfolioSummary,
    pub top_holdings: Vec<Holding>,
    pub recent_transactions: Vec<Transaction>,
}

pub fn holding_entity_id(portfolio_id: i64, symbol: &str) -> String {
    format!("{portfolio_id}:{symbol}")
}
