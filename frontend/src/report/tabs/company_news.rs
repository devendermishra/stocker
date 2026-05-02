use dioxus::prelude::*;

use crate::components::{news_triples, NewsList};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn company_news_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Company News" }
            NewsList { items: news_triples(&r.news) }
            h3 { "Sector News" }
            NewsList { items: news_triples(&r.sector_news) }
        }
    }
}
