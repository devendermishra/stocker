use dioxus::prelude::*;

use crate::components::Home;
use crate::report::Report;
use crate::screener::Screener;

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/report/:symbol")]
    Report { symbol: String },
    #[route("/screener")]
    Screener {},
}
