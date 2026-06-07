use dioxus::prelude::*;

use crate::components::Home;
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
}
