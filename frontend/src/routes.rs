use dioxus::prelude::*;

use crate::components::Home;
use crate::portfolio::{
    PortfolioDashboard, PortfolioHoldings, PortfolioLabels, PortfolioList,
    PortfolioStockDetail, PortfolioTransactions,
};
use crate::report::Report;
use crate::screener::Screener;
use crate::stocks::Stocks;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/?:id&:exchange")]
    Home { id: String, exchange: String },
    #[route("/report/:symbol")]
    Report { symbol: String },
    #[route("/screener")]
    Screener {},
    #[route("/stocks")]
    Stocks {},
    #[route("/portfolio")]
    PortfolioList {},
    #[route("/portfolio/labels")]
    PortfolioLabels {},
    #[route("/portfolio/:id")]
    PortfolioDashboard { id: i64 },
    #[route("/portfolio/:id/holdings")]
    PortfolioHoldings { id: i64 },
    #[route("/portfolio/:id/transactions")]
    PortfolioTransactions { id: i64 },
    #[route("/portfolio/:id/stock/:symbol")]
    PortfolioStockDetail { id: i64, symbol: String },
}
