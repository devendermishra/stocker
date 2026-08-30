use dioxus::prelude::*;

use crate::components::Home;
use crate::portfolio::{
    PortfolioDashboard, PortfolioHoldings, PortfolioLabels, PortfolioList, PortfolioOverview,
    PortfolioStockDetail, PortfolioTransactions, PortfolioSchedules,
};
use crate::report::Report;
use crate::screener::Screener;
use crate::sectors::{SectorDetailPage, SectorsList};
use crate::stocks::Stocks;
use crate::sync::DriveSync;
use crate::sync_portfolio::{
    SyncPortfolioDashboard, SyncPortfolioHoldings, SyncPortfolioOverview, SyncPortfolioStockDetail,
    SyncPortfolioTransactions,
};

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
    #[route("/sectors")]
    SectorsList {},
    #[route("/sectors/:sector")]
    SectorDetailPage { sector: String },
    #[route("/sync")]
    DriveSync {},
    #[route("/sync/portfolio/:id")]
    SyncPortfolioOverview { id: i64 },
    #[route("/sync/portfolio/:id/holdings")]
    SyncPortfolioHoldings { id: i64 },
    #[route("/sync/portfolio/:id/transactions")]
    SyncPortfolioTransactions { id: i64 },
    #[route("/sync/portfolio/:id/dashboard")]
    SyncPortfolioDashboard { id: i64 },
    #[route("/sync/portfolio/:id/stock/:symbol")]
    SyncPortfolioStockDetail { id: i64, symbol: String },
    #[route("/portfolio")]
    PortfolioList {},
    #[route("/portfolio/labels")]
    PortfolioLabels {},
    #[route("/portfolio/:id")]
    PortfolioOverview { id: i64 },
    #[route("/portfolio/:id/dashboard")]
    PortfolioDashboard { id: i64 },
    #[route("/portfolio/:id/holdings")]
    PortfolioHoldings { id: i64 },
    #[route("/portfolio/:id/transactions")]
    PortfolioTransactions { id: i64 },
    #[route("/portfolio/:id/sips")]
    PortfolioSchedules { id: i64 },
    #[route("/portfolio/:id/stock/:symbol")]
    PortfolioStockDetail { id: i64, symbol: String },
}
