use dioxus::prelude::*;

#[cfg(feature = "web")]
use crate::web_types;

#[component]
pub fn NewsList(items: Vec<(String, String, String)>) -> Element {
    rsx! {
        ul { style: "padding-left: 1.2rem;",
            for (title, link, published_at) in items.iter().take(10) {
                li { style: "margin-bottom: 0.4rem;",
                    a { href: "{link}", target: "_blank", rel: "noopener", "{title}" }
                    if !published_at.is_empty() {
                        span { style: "color:#778196; margin-left: 0.35rem; font-size: 0.85rem;", "({published_at})" }
                    }
                }
            }
        }
    }
}

pub fn news_triples(items: &[impl NewsTriplet]) -> Vec<(String, String, String)> {
    items
        .iter()
        .map(|n| (n.title().to_string(), n.link().to_string(), n.published_at().to_string()))
        .collect()
}

pub trait NewsTriplet {
    fn title(&self) -> &str;
    fn link(&self) -> &str;
    fn published_at(&self) -> &str;
}

#[cfg(feature = "web")]
impl NewsTriplet for web_types::NewsItem {
    fn title(&self) -> &str {
        &self.title
    }
    fn link(&self) -> &str {
        &self.link
    }
    fn published_at(&self) -> &str {
        &self.published_at
    }
}

#[cfg(feature = "desktop")]
impl NewsTriplet for stocker_core::NewsItem {
    fn title(&self) -> &str {
        &self.title
    }
    fn link(&self) -> &str {
        &self.link
    }
    fn published_at(&self) -> &str {
        &self.published_at
    }
}
