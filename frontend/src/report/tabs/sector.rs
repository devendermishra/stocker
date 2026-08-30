use dioxus::prelude::*;

use crate::components::{news_triples, NewsList};
use crate::report::CARD;
use crate::routes::Route;
use crate::sectors::SectorResearchCompact;
use crate::sectors_api::{encode_sector_path, SectorResearchProfileView};
use crate::types::ResearchReport;

fn research_view(r: &ResearchReport) -> Option<SectorResearchProfileView> {
    #[cfg(feature = "web")]
    {
        r.sector_analysis.research.clone()
    }
    #[cfg(feature = "desktop")]
    {
        r.sector_analysis.research.as_ref().and_then(|p| {
            serde_json::to_value(p)
                .ok()
                .and_then(|v| serde_json::from_value(v).ok())
        })
    }
}

pub fn sector_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let sector_name = r
        .sector_analysis
        .sector
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            r.asset_profile
                .sector
                .clone()
                .filter(|s| !s.is_empty())
        });
    let research = research_view(r);

    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Sector Context" }
            p { "{r.sector_analysis.outlook_narrative}" }
            p { "{r.sector_analysis.sector_news_summary}" }
            p { style: "color:#344155;", "{r.sector_analysis.sector_headline_themes}" }
            if let Some(name) = sector_name.clone() {
                if research.is_some() {
                    p { style: "margin: 0.75rem 0 0;",
                        Link {
                            to: Route::SectorDetailPage { sector: encode_sector_path(&name) },
                            style: "color: #184ad8; font-weight: 600; text-decoration: none;",
                            "Full sector research →"
                        }
                    }
                }
            }
            if let Some(profile) = research {
                SectorResearchCompact { profile }
            }
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
