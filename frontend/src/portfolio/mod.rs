//! Portfolio UI module.

mod confirm_dialog;
mod dashboard;
mod holdings;
mod import;
mod labels;
mod layout;
mod list;
mod overview;
mod stock_detail;
mod styles;
mod transactions;

pub use dashboard::PortfolioDashboard;
pub use holdings::PortfolioHoldings;
pub use labels::PortfolioLabels;
pub use import::TransactionImport;
pub use list::PortfolioList;
pub use overview::PortfolioOverview;
pub use stock_detail::PortfolioStockDetail;
pub use transactions::PortfolioTransactions;
