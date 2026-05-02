use dioxus::prelude::*;

use crate::components::{news_triples, NewsList};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn sector_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Sector Context" }
            p { "{r.sector_analysis.outlook_narrative}" }
            p { "{r.sector_analysis.sector_news_summary}" }
            p { style: "color:#344155;", "{r.sector_analysis.sector_headline_themes}" }
            if !r.sector_analysis.sample_headlines.is_empty() {
                h4 { "Sample Sector Headlines" }
                ul { for h in &r.sector_analysis.sample_headlines { li { "{h}" } } }
            }
            if !r.sector_news.is_empty() {
                h4 { "Sector News Feed" }
                NewsList { items: news_triples(&r.sector_news) }
            }
        }
    }
}
