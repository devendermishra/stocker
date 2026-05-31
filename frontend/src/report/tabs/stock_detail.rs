use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::format::{fmt_refreshed_at, fmt_screener_metric};
use crate::report::CARD;
use crate::screener_api::{CatalogEntry, ScreenRow};

#[component]
pub fn StockDetailedInformation(
    symbol: String,
    catalog: Vec<CatalogEntry>,
    snapshot: Option<ScreenRow>,
    load_error: Option<String>,
    refreshing: bool,
    refresh_error: Option<String>,
    on_refresh: EventHandler<()>,
) -> Element {
    let card = CARD;

    let mut by_category: BTreeMap<String, Vec<&CatalogEntry>> = BTreeMap::new();
    for e in catalog.iter() {
        by_category
            .entry(e.category_label.clone())
            .or_default()
            .push(e);
    }

    let last_refreshed = snapshot
        .as_ref()
        .and_then(|s| s.last_refreshed_at);

    rsx! {
        section { style: "{card}; margin-top: 0.65rem;",
            div {
                style: "display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 0.6rem; margin-bottom: 0.75rem;",
                div {
                    h3 { style: "margin: 0;", "Detailed Data" }
                    p {
                        style: "margin: 0.25rem 0 0; font-size: 0.88rem; color: #667085;",
                        "All screener parameters from the local SQLite database."
                    }
                    p {
                        style: "margin: 0.2rem 0 0; font-size: 0.85rem; color: #888;",
                        "Last refreshed: {fmt_refreshed_at(last_refreshed)}"
                    }
                }
                button {
                    style: "padding: 0.45rem 0.9rem; background: #184ad8; color: #fff; border: none; border-radius: 8px; cursor: pointer; font-weight: 600;",
                    disabled: refreshing,
                    onclick: move |_| on_refresh.call(()),
                    if refreshing { "Refreshing metrics…" } else { "Refresh metrics data" }
                }
            }

            if let Some(err) = refresh_error {
                p { style: "color: #b00020; font-size: 0.9rem; margin: 0 0 0.5rem;", "{err}" }
            }

            if let Some(err) = load_error {
                p {
                    style: "color: #b00020; background: #fdecea; border: 1px solid #f5c6cb; border-radius: 8px; padding: 0.55rem 0.75rem; font-size: 0.9rem; margin-bottom: 0.5rem;",
                    "Could not load screener data: {err}"
                }
            } else if snapshot.is_none() {
                p {
                    style: "color: #5a4300; background: #fff8e1; border: 1px solid #f0c14b; border-radius: 8px; padding: 0.55rem 0.75rem; font-size: 0.9rem;",
                    "No metrics snapshot for {symbol} yet. Click "
                    strong { "Refresh metrics data" }
                    " to fetch from Yahoo (may take a few seconds)."
                }
            } else if let Some(row) = snapshot.as_ref() {
                div {
                    style: "margin-top: 0.5rem;",
                    h4 {
                        style: "margin: 0 0 0.45rem; font-size: 0.95rem; color: #243043; border-bottom: 1px solid #eceff5; padding-bottom: 0.35rem;",
                        "Identity & metadata"
                    }
                    div {
                        style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 0.35rem 1rem;",
                        MetricLine { label: "Symbol".to_string(), value: row.symbol.clone() }
                        MetricLine { label: "Short name".to_string(), value: opt_str(row.short_name.as_deref()) }
                        MetricLine { label: "Sector".to_string(), value: opt_str(row.sector.as_deref()) }
                        MetricLine { label: "Industry".to_string(), value: opt_str(row.industry.as_deref()) }
                        MetricLine { label: "Exchange".to_string(), value: opt_str(row.exchange.as_deref()) }
                        MetricLine { label: "Currency".to_string(), value: opt_str(row.currency.as_deref()) }
                        MetricLine { label: "Country".to_string(), value: opt_str(row.country.as_deref()) }
                        MetricLine {
                            label: "Tier".to_string(),
                            value: row.tier.map(|t| t.to_string()).unwrap_or_else(|| "—".to_string())
                        }
                        MetricLine {
                            label: "Face value".to_string(),
                            value: row.face_value.map(|v| format!("{v:.2}")).unwrap_or_else(|| "—".to_string())
                        }
                        MetricLine {
                            label: "Last refreshed".to_string(),
                            value: fmt_refreshed_at(row.last_refreshed_at)
                        }
                        MetricLine {
                            label: "Refresh status".to_string(),
                            value: opt_str(row.last_refresh_status.as_deref())
                        }
                        MetricLine {
                            label: "Refresh error".to_string(),
                            value: opt_str(row.last_refresh_error.as_deref())
                        }
                        MetricLine {
                            label: "Snapshot updated".to_string(),
                            value: fmt_refreshed_at(row.updated_at)
                        }
                    }
                }

                for (category, entries) in by_category.iter() {
                    div {
                        style: "margin-top: 0.85rem;",
                        h4 {
                            style: "margin: 0 0 0.45rem; font-size: 0.95rem; color: #243043; border-bottom: 1px solid #eceff5; padding-bottom: 0.35rem;",
                            "{category}"
                        }
                        div {
                            style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 0.35rem 1rem;",
                            for entry in entries.iter() {
                                MetricLine {
                                    label: entry.label.clone(),
                                    value: metric_display(Some(row), entry),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn opt_str(v: Option<&str>) -> String {
    v.filter(|s| !s.is_empty())
        .unwrap_or("—")
        .to_string()
}

fn metric_display(snapshot: Option<&ScreenRow>, entry: &CatalogEntry) -> String {
    let Some(row) = snapshot else {
        return "—".to_string();
    };
    let raw = row
        .metrics
        .get(&entry.column)
        .and_then(|v| v.as_f64());
    fmt_screener_metric(raw, &entry.unit)
}

#[component]
fn MetricLine(label: String, value: String) -> Element {
    rsx! {
        div {
            style: "display: flex; justify-content: space-between; gap: 0.75rem; font-size: 0.88rem; padding: 0.2rem 0;",
            span { style: "color: #556074;", "{label}" }
            span { style: "font-weight: 600; font-variant-numeric: tabular-nums; text-align: right;", "{value}" }
        }
    }
}
