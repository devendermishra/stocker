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
    /// Number of transactions deleted if this label is removed (list endpoint only).
    #[serde(default)]
    pub transaction_count: i64,
    /// Holdings this label is attached to (list endpoint only).
    #[serde(default)]
    pub holding_count: i64,
    /// Whole portfolios this label is attached to (list endpoint only).
    #[serde(default)]
    pub portfolio_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearTransactionsResult {
    pub transactions_deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteLabelResult {
    pub transactions_deleted: usize,
    pub portfolios_deleted: usize,
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
}

impl TransactionType {
    pub fn as_str(self) -> &'static str {
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
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "opening_balance" => Some(Self::OpeningBalance),
            "buy" => Some(Self::Buy),
            "merger_investment" => Some(Self::MergerInvestment),
            "demerger_investment" => Some(Self::DemergerInvestment),
            "merger_redemption" => Some(Self::MergerRedemption),
            "demerger_redemption" => Some(Self::DemergerRedemption),
            "sell" => Some(Self::Sell),
            "dividend" => Some(Self::Dividend),
            "split" => Some(Self::Split),
            "bonus" => Some(Self::Bonus),
            "rights" => Some(Self::Rights),
            "sip" => Some(Self::Sip),
            _ => None,
        }
    }

    pub fn is_buy_like(self) -> bool {
        matches!(
            self,
            Self::OpeningBalance
                | Self::Buy
                | Self::MergerInvestment
                | Self::DemergerInvestment
                | Self::Rights
        )
    }

    pub fn is_sell_like(self) -> bool {
        matches!(
            self,
            Self::Sell | Self::MergerRedemption | Self::DemergerRedemption
        )
    }

    pub fn requires_symbol(self) -> bool {
        !matches!(self, Self::Sip)
    }

    pub fn requires_positive_quantity(self) -> bool {
        self.is_buy_like() || self.is_sell_like()
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

impl From<Transaction> for NewTransaction {
    fn from(t: Transaction) -> Self {
        Self {
            portfolio_id: t.portfolio_id,
            txn_type: t.txn_type,
            trade_date: t.trade_date,
            symbol: t.symbol,
            quantity: t.quantity,
            price: t.price,
            gross_amount: t.gross_amount,
            brokerage: t.brokerage,
            taxes: t.taxes,
            net_amount: t.net_amount,
            split_ratio_num: t.split_ratio_num,
            split_ratio_den: t.split_ratio_den,
            bonus_ratio_num: t.bonus_ratio_num,
            bonus_ratio_den: t.bonus_ratio_den,
            dividend_per_share: t.dividend_per_share,
            tds: t.tds,
            eligible_quantity: t.eligible_quantity,
            notes: t.notes,
        }
    }
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
    /// `xirr` or `cagr` — how `total_return_pct` was computed.
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
    pub return_method: Option<String>,
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
