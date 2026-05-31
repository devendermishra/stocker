//! Screener UI: filter builder + results table + saved-screen panel.

use dioxus::prelude::*;

use crate::format::fmt_screener_metric;
use crate::routes::Route;
use crate::screener_api::{
    coverage, create_screen, delete_screen, list_fields, list_screens, run_search, start_backfill,
    status, CatalogEntry, CoverageReport, CoverageTier, MetricCoverage, SavedScreen, ScreenRow,
    ScreenerStatus,
};

const CARD: &str =
    "background: #fff; border: 1px solid #dfe3eb; border-radius: 12px; padding: 0.85rem;";
const LOW_COVERAGE_THRESHOLD_PCT: f64 = 10.0;

fn coverage_by_id(report: &Option<Result<CoverageReport, String>>) -> std::collections::HashMap<String, f64> {
    let Some(Ok(r)) = report else {
        return Default::default();
    };
    r.metrics.iter().map(|m| (m.id.clone(), m.fill_pct)).collect()
}

fn is_low_coverage(fill_pct: Option<f64>) -> bool {
    fill_pct.is_some_and(|p| p < LOW_COVERAGE_THRESHOLD_PCT)
}

async fn sleep_secs(secs: u64) {
    #[cfg(feature = "web")]
    {
        gloo_timers::future::TimeoutFuture::new((secs * 1000) as u32).await;
    }
    #[cfg(feature = "desktop")]
    {
        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
    }
}

fn should_poll_status(s: &ScreenerStatus) -> bool {
    s.backfill_running || s.pending_count > 0
}

fn refresh_msg_style(msg: &str) -> &'static str {
    if msg.to_lowercase().contains("already running") {
        "margin: 0.5rem 0 0; padding: 0.45rem 0.65rem; background: #fff8e1; border: 1px solid #f0c14b; border-radius: 8px; font-size: 0.9rem; color: #5a4300;"
    } else if msg.contains("started") {
        "margin: 0.5rem 0 0; padding: 0.45rem 0.65rem; background: #e8f5e9; border: 1px solid #a5d6a7; border-radius: 8px; font-size: 0.9rem; color: #1b5e20;"
    } else {
        "margin: 0.5rem 0 0; padding: 0.45rem 0.65rem; background: #fdecea; border: 1px solid #f5c6cb; border-radius: 8px; font-size: 0.9rem; color: #b00020;"
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ScreenerTab {
    Screen,
    Coverage,
}

#[derive(Clone, PartialEq, Debug)]
struct UiFilter {
    field_id: String,
    op: String,
    value: String,
    /// Cached for nicer rendering when the catalog is loaded.
    label: String,
    unit: String,
    category_label: String,
    needs_review: bool,
}

#[component]
pub fn Screener() -> Element {
    let catalog = use_resource(|| async { list_fields().await });
    let mut status_reload = use_signal(|| 0u32);
    let status_res = use_resource(move || {
        let _ = status_reload();
        async move { status().await }
    });
    let coverage_res = use_resource(|| async { coverage().await });
    let mut tab: Signal<ScreenerTab> = use_signal(|| ScreenerTab::Screen);
    let mut filters: Signal<Vec<UiFilter>> = use_signal(Vec::new);
    let mut results: Signal<Vec<ScreenRow>> = use_signal(Vec::new);
    let mut search_error: Signal<Option<String>> = use_signal(|| None);
    let mut searching = use_signal(|| false);
    let mut saved: Signal<Vec<SavedScreen>> = use_signal(Vec::new);
    let mut save_name: Signal<String> = use_signal(String::new);
    let mut refresh_msg: Signal<Option<String>> = use_signal(|| None);
    let mut backfill_busy = use_signal(|| false);
    let _saved_loader = use_resource(move || async move {
        match list_screens().await {
            Ok(v) => saved.set(v),
            Err(e) => log_warn(&format!("list_screens: {e}")),
        }
    });

    let poll_status = move |mut reload: Signal<u32>| {
        spawn(async move {
            loop {
                sleep_secs(10).await;
                if let Ok(s) = status().await {
                    reload.set(reload() + 1);
                    if !should_poll_status(&s) {
                        break;
                    }
                } else {
                    break;
                }
            }
        });
    };

    let on_refresh_data = move |_| {
        spawn(async move {
            backfill_busy.set(true);
            refresh_msg.set(None);
            match start_backfill().await {
                Ok(()) => {
                    refresh_msg.set(Some(
                        "Stock data refresh started in background.".to_string(),
                    ));
                    status_reload.set(status_reload() + 1);
                    poll_status(status_reload);
                }
                Err(e) if e.to_lowercase().contains("already running") => {
                    refresh_msg.set(Some(
                        "Stock data refresh is already running.".to_string(),
                    ));
                }
                Err(e) => refresh_msg.set(Some(e)),
            }
            backfill_busy.set(false);
        });
    };

    let catalog_ready: Vec<CatalogEntry> = match &*catalog.read() {
        Some(Ok(c)) => c.clone(),
        _ => Vec::new(),
    };

    let mut grouped: std::collections::BTreeMap<String, Vec<CatalogEntry>> = Default::default();
    for e in &catalog_ready {
        grouped.entry(e.category_label.clone()).or_default().push(e.clone());
    }

    let on_run = move |_| {
        let query = build_query(&filters.read());
        spawn(async move {
            searching.set(true);
            search_error.set(None);
            match run_search(query).await {
                Ok(rows) => results.set(rows),
                Err(e) => search_error.set(Some(e)),
            }
            searching.set(false);
        });
    };

    rsx! {
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/modern-normalize@2/modern-normalize.min.css" }
        div {
            style: "font-family: Inter, system-ui, sans-serif; max-width: 1180px; margin: 1.5rem auto; padding: 0 1rem 2rem;",
            div { style: "display: flex; align-items: baseline; gap: 1rem; flex-wrap: wrap;",
                Link { to: Route::Home {}, style: "color: #184ad8;", "← Home" }
                h1 { style: "margin: 0;", "NSE Screener" }
                if *tab.read() == ScreenerTab::Screen {
                    p { style: "color: #555; margin: 0;", "All conditions must match (AND)." }
                }
            }

            div { style: "margin-top: 0.75rem; display: flex; gap: 0.35rem; border-bottom: 1px solid #dfe3eb; padding-bottom: 0.5rem;",
                TabButton {
                    label: "Screen",
                    active: *tab.read() == ScreenerTab::Screen,
                    onclick: move |_| tab.set(ScreenerTab::Screen),
                }
                TabButton {
                    label: "Data coverage",
                    active: *tab.read() == ScreenerTab::Coverage,
                    onclick: move |_| tab.set(ScreenerTab::Coverage),
                }
            }

            div { style: "margin-top: 1rem; display: flex; flex-wrap: wrap; align-items: center; gap: 0.75rem;",
                div { style: "flex: 1 1 280px;",
                    StatusBanner { status: status_res.read().clone() }
                }
                button {
                    style: "padding: 0.45rem 0.9rem; background: #184ad8; color: #fff; border: none; border-radius: 8px; cursor: pointer; font-weight: 600; white-space: nowrap;",
                    disabled: backfill_busy(),
                    onclick: on_refresh_data,
                    if backfill_busy() { "Starting refresh…" } else { "Refresh stock data" }
                }
            }

            if let Some(msg) = refresh_msg.read().as_ref() {
                p {
                    style: refresh_msg_style(msg),
                    "{msg}"
                }
            }

            if *tab.read() == ScreenerTab::Coverage {
                CoveragePanel { report: coverage_res.read().clone() }
            } else {
                ScreenPanel {
                    catalog_ready: catalog_ready.clone(),
                    grouped: grouped.clone(),
                    coverage_map: coverage_by_id(&coverage_res.read()),
                    filters: filters,
                    results: results,
                    search_error: search_error,
                    searching: searching,
                    saved: saved,
                    save_name: save_name,
                    on_run: on_run,
                }
            }
        }
    }
}

#[component]
fn TabButton(label: &'static str, active: bool, onclick: EventHandler<()>) -> Element {
    let style = if active {
        "padding: 0.45rem 0.85rem; background: #184ad8; color: #fff; border: none; border-radius: 8px 8px 0 0; cursor: pointer; font-weight: 600;"
    } else {
        "padding: 0.45rem 0.85rem; background: #f4f6fa; color: #333; border: 1px solid #dfe3eb; border-bottom: none; border-radius: 8px 8px 0 0; cursor: pointer;"
    };
    rsx! {
        button { style: "{style}", onclick: move |_| onclick.call(()), "{label}" }
    }
}

#[component]
fn ScreenPanel(
    catalog_ready: Vec<CatalogEntry>,
    grouped: std::collections::BTreeMap<String, Vec<CatalogEntry>>,
    coverage_map: std::collections::HashMap<String, f64>,
    mut filters: Signal<Vec<UiFilter>>,
    mut results: Signal<Vec<ScreenRow>>,
    mut search_error: Signal<Option<String>>,
    mut searching: Signal<bool>,
    mut saved: Signal<Vec<SavedScreen>>,
    mut save_name: Signal<String>,
    on_run: EventHandler<()>,
) -> Element {
    let low_coverage_filters: Vec<(String, f64)> = filters
        .read()
        .iter()
        .filter_map(|f| {
            coverage_map
                .get(&f.field_id)
                .copied()
                .filter(|p| *p < LOW_COVERAGE_THRESHOLD_PCT)
                .map(|p| (f.label.clone(), p))
        })
        .collect();

    rsx! {
            // Filter builder
            div { style: "margin-top: 1rem; {CARD}",
                h2 { style: "margin: 0 0 0.6rem;", "Filters" }
                if !low_coverage_filters.is_empty() {
                    LowCoverageBanner { items: low_coverage_filters.clone() }
                }
                if filters.read().is_empty() {
                    p { style: "color: #555; margin: 0 0 0.8rem;", "No filters yet — pick a metric below to start." }
                } else {
                    div { style: "display: grid; gap: 0.4rem; margin-bottom: 0.8rem;",
                        for (idx, filter) in filters.read().iter().enumerate() {
                            FilterRow {
                                idx: idx,
                                filter: filter.clone(),
                                fill_pct: coverage_map.get(&filter.field_id).copied(),
                                catalog: catalog_ready.clone(),
                                on_change: move |f: UiFilter| {
                                    let mut current = filters.write();
                                    if let Some(slot) = current.get_mut(idx) {
                                        *slot = f;
                                    }
                                },
                                on_remove: move |_| {
                                    let mut current = filters.write();
                                    if idx < current.len() {
                                        current.remove(idx);
                                    }
                                }
                            }
                        }
                    }
                }

                // Add metric controls
                div { style: "margin-top: 0.4rem; display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center;",
                    select {
                        style: "padding: 0.45rem 0.6rem; border: 1px solid #d5dbe3; border-radius: 8px; min-width: 320px;",
                        onchange: move |ev| {
                            let id = ev.value();
                            if id.is_empty() { return; }
                            if let Some(spec) = catalog_ready.iter().find(|c| c.id == id) {
                                let mut current = filters.write();
                                current.push(UiFilter {
                                    field_id: spec.id.clone(),
                                    op: "gte".into(),
                                    value: String::new(),
                                    label: spec.label.clone(),
                                    unit: spec.unit.clone(),
                                    category_label: spec.category_label.clone(),
                                    needs_review: spec.needs_review,
                                });
                            }
                        },
                        option { value: "", "+ Add filter…" }
                        for (cat_label, specs) in grouped.iter() {
                            optgroup { label: "{cat_label}",
                                for spec in specs.iter() {
                                    {
                                        let low = is_low_coverage(coverage_map.get(&spec.id).copied());
                                        rsx! {
                                            option {
                                                value: "{spec.id}",
                                                title: "{spec.description}",
                                                "{spec.label}{review_marker(spec.needs_review)}{low_coverage_marker(low)}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    button {
                        style: button_primary(*searching.read()),
                        disabled: *searching.read(),
                        onclick: move |_| on_run.call(()),
                        if *searching.read() { "Running…" } else { "Run screen" }
                    }
                    button {
                        style: "padding: 0.5rem 1rem; border: 1px solid #cad1de; background: #fff; border-radius: 8px; cursor: pointer;",
                        onclick: move |_| filters.set(Vec::new()),
                        "Clear all"
                    }
                }
            }

            // Saved screens
            div { style: "margin-top: 1rem; {CARD}",
                h2 { style: "margin: 0 0 0.5rem;", "Saved screens" }
                div { style: "display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap;",
                    input {
                        style: "flex: 1; min-width: 200px; padding: 0.45rem 0.6rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                        placeholder: "Name this screen…",
                        value: "{save_name}",
                        oninput: move |e| save_name.set(e.value()),
                    }
                    button {
                        style: button_primary(false),
                        onclick: move |_| {
                            let name = save_name.cloned();
                            if name.trim().is_empty() { return; }
                            let payload = serde_json::Value::Array(
                                build_query(&filters.read())["filters"].as_array().cloned().unwrap_or_default(),
                            );
                            spawn(async move {
                                match create_screen(name, payload).await {
                                    Ok(_) => {
                                        if let Ok(v) = list_screens().await { saved.set(v); }
                                        save_name.set(String::new());
                                    }
                                    Err(e) => search_error.set(Some(format!("save: {e}"))),
                                }
                            });
                        },
                        "Save current"
                    }
                }
                if saved.read().is_empty() {
                    p { style: "color: #555; margin: 0.6rem 0 0;", "No saved screens yet." }
                } else {
                    div { style: "margin-top: 0.6rem; display: grid; gap: 0.4rem;",
                        for screen in saved.read().iter() {
                            {
                                let catalog_for_load = catalog_ready.clone();
                                rsx! {
                                    SavedScreenRow {
                                        screen: screen.clone(),
                                        catalog: catalog_ready.clone(),
                                        on_load: move |s: SavedScreen| {
                                            let parsed = parse_saved(&s, &catalog_for_load);
                                            filters.set(parsed);
                                        },
                                        on_delete: move |id: i64| {
                                            spawn(async move {
                                                let _ = delete_screen(id).await;
                                                if let Ok(v) = list_screens().await { saved.set(v); }
                                            });
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Results
            div { style: "margin-top: 1rem;",
                match &*search_error.read() {
                    Some(e) => rsx! { p { style: "color: #b00020;", "{e}" } },
                    None => rsx! {
                        if results.read().is_empty() {
                            p { style: "color: #555;", "No results yet — set up filters and run." }
                        } else {
                            ResultsTable {
                                rows: results.read().clone(),
                                filters: filters.read().clone(),
                                catalog: catalog_ready.clone(),
                            }
                        }
                    }
                }
            }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CoverageFilter {
    All,
    Full,
    Partial,
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CoverageSort {
    FillPctDesc,
    FillPctAsc,
    LabelAsc,
    CategoryAsc,
}

#[component]
fn CoveragePanel(report: Option<Result<CoverageReport, String>>) -> Element {
    let mut tier_filter: Signal<CoverageFilter> = use_signal(|| CoverageFilter::All);
    let mut sort: Signal<CoverageSort> = use_signal(|| CoverageSort::FillPctDesc);

    match report {
        None => rsx! { p { style: "color: #555; margin-top: 1rem;", "Loading coverage…" } },
        Some(Err(e)) => rsx! { p { style: "color: #b00020; margin-top: 1rem;", "Coverage error: {e}" } },
        Some(Ok(r)) => {
            let summary = r.summary.clone();
            let snapshot_count = r.snapshot_count;
            let filter = *tier_filter.read();
            let sort_mode = *sort.read();
            let mut rows: Vec<MetricCoverage> = r
                .metrics
                .iter()
                .filter(|m| match filter {
                    CoverageFilter::All => true,
                    CoverageFilter::Full => m.tier == CoverageTier::Full,
                    CoverageFilter::Partial => m.tier == CoverageTier::Partial,
                    CoverageFilter::Empty => m.tier == CoverageTier::Empty,
                })
                .cloned()
                .collect();
            match sort_mode {
                CoverageSort::FillPctDesc => rows.sort_by(|a, b| b.fill_pct.partial_cmp(&a.fill_pct).unwrap()),
                CoverageSort::FillPctAsc => rows.sort_by(|a, b| a.fill_pct.partial_cmp(&b.fill_pct).unwrap()),
                CoverageSort::LabelAsc => rows.sort_by(|a, b| a.label.cmp(&b.label)),
                CoverageSort::CategoryAsc => rows.sort_by(|a, b| {
                    a.category_label
                        .cmp(&b.category_label)
                        .then_with(|| a.label.cmp(&b.label))
                }),
            }
            rsx! {
                div { style: "margin-top: 1rem; {CARD}",
                    h2 { style: "margin: 0 0 0.5rem;", "Metric data coverage" }
                    p { style: "color: #555; margin: 0 0 0.75rem; font-size: 0.92rem;",
                        "Non-null values across {snapshot_count} snapshots. "
                        strong { "{summary.full}" } " full · "
                        strong { "{summary.partial}" } " partial · "
                        strong { "{summary.empty}" } " empty"
                    }
                    div { style: "display: flex; gap: 0.35rem; flex-wrap: wrap; margin-bottom: 0.75rem;",
                        CoverageChip {
                            label: "All",
                            count: r.metrics.len(),
                            active: filter == CoverageFilter::All,
                            onclick: move |_| tier_filter.set(CoverageFilter::All),
                        }
                        CoverageChip {
                            label: "Full",
                            count: summary.full,
                            active: filter == CoverageFilter::Full,
                            onclick: move |_| tier_filter.set(CoverageFilter::Full),
                        }
                        CoverageChip {
                            label: "Partial",
                            count: summary.partial,
                            active: filter == CoverageFilter::Partial,
                            onclick: move |_| tier_filter.set(CoverageFilter::Partial),
                        }
                        CoverageChip {
                            label: "Empty",
                            count: summary.empty,
                            active: filter == CoverageFilter::Empty,
                            onclick: move |_| tier_filter.set(CoverageFilter::Empty),
                        }
                        select {
                            style: "margin-left: auto; padding: 0.35rem 0.5rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                            onchange: move |ev| {
                                sort.set(match ev.value().as_str() {
                                    "fill_asc" => CoverageSort::FillPctAsc,
                                    "label" => CoverageSort::LabelAsc,
                                    "category" => CoverageSort::CategoryAsc,
                                    _ => CoverageSort::FillPctDesc,
                                });
                            },
                            option { value: "fill_desc", "Sort: fill % ↓" }
                            option { value: "fill_asc", "Sort: fill % ↑" }
                            option { value: "label", "Sort: metric name" }
                            option { value: "category", "Sort: category" }
                        }
                    }
                    div { style: "overflow-x: auto;",
                        table { style: "width: 100%; border-collapse: collapse; font-size: 0.92rem;",
                            thead {
                                tr { style: "text-align: left; border-bottom: 1px solid #dfe3eb;",
                                    th { style: "padding: 0.4rem 0.6rem;", "Metric" }
                                    th { style: "padding: 0.4rem 0.6rem;", "Category" }
                                    th { style: "padding: 0.4rem 0.6rem;", "Source" }
                                    th { style: "padding: 0.4rem 0.6rem; text-align: right;", "Fill %" }
                                    th { style: "padding: 0.4rem 0.6rem; text-align: right;", "Filled" }
                                    th { style: "padding: 0.4rem 0.6rem;", "Tier" }
                                }
                            }
                            tbody {
                                for m in rows.iter() {
                                    tr {
                                        style: coverage_row_style(m.tier.clone()),
                                        td { style: "padding: 0.4rem 0.6rem;",
                                            strong { "{m.label}" }
                                            if m.needs_review {
                                                span { style: "margin-left: 0.35rem; font-size: 0.7rem; background: #ffe7c2; color: #5a3b00; border-radius: 4px; padding: 0 0.25rem;", "review" }
                                            }
                                            div { style: "color: #666; font-size: 0.8rem; max-width: 420px;", "{m.description}" }
                                        }
                                        td { style: "padding: 0.4rem 0.6rem;", "{m.category_label}" }
                                        td { style: "padding: 0.4rem 0.6rem; font-size: 0.85rem; color: #555;", "{m.source_kind}" }
                                        td { style: "padding: 0.4rem 0.6rem; text-align: right; font-variant-numeric: tabular-nums;", "{format_fill_pct(m.fill_pct)}" }
                                        td { style: "padding: 0.4rem 0.6rem; text-align: right; font-variant-numeric: tabular-nums;", "{m.filled} / {snapshot_count}" }
                                        td { style: "padding: 0.4rem 0.6rem;",
                                            TierBadge { tier: m.tier.clone() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CoverageChip(label: &'static str, count: usize, active: bool, onclick: EventHandler<()>) -> Element {
    let style = if active {
        "padding: 0.35rem 0.65rem; background: #184ad8; color: #fff; border: none; border-radius: 999px; cursor: pointer; font-size: 0.88rem;"
    } else {
        "padding: 0.35rem 0.65rem; background: #f4f6fa; color: #333; border: 1px solid #dfe3eb; border-radius: 999px; cursor: pointer; font-size: 0.88rem;"
    };
    rsx! {
        button { style: "{style}", onclick: move |_| onclick.call(()), "{label} ({count})" }
    }
}

#[component]
fn TierBadge(tier: CoverageTier) -> Element {
    let (text, bg, fg) = match tier {
        CoverageTier::Full => ("Full", "#e6f4ea", "#137333"),
        CoverageTier::Partial => ("Partial", "#fef7e0", "#b06000"),
        CoverageTier::Empty => ("Empty", "#fce8e6", "#c5221f"),
    };
    rsx! {
        span { style: "padding: 0.15rem 0.45rem; border-radius: 6px; font-size: 0.82rem; background: {bg}; color: {fg}; font-weight: 600;", "{text}" }
    }
}

fn coverage_row_style(tier: CoverageTier) -> String {
    let bg = match tier {
        CoverageTier::Full => "#fafdfb",
        CoverageTier::Partial => "#fffcf5",
        CoverageTier::Empty => "#fffafa",
    };
    format!("border-bottom: 1px solid #f0f1f5; background: {bg};")
}

fn format_fill_pct(v: f64) -> String {
    format!("{v:.1}")
}

fn review_marker(b: bool) -> &'static str {
    if b { "  (review)" } else { "" }
}

fn low_coverage_marker(low: bool) -> &'static str {
    if low { "  (low coverage)" } else { "" }
}

fn button_primary(loading: bool) -> &'static str {
    if loading {
        "padding: 0.5rem 1rem; background: #555; color: white; border: none; border-radius: 8px; cursor: progress;"
    } else {
        "padding: 0.5rem 1rem; background: #184ad8; color: white; border: none; border-radius: 8px; cursor: pointer;"
    }
}

fn build_query(filters: &[UiFilter]) -> serde_json::Value {
    let mut out = Vec::new();
    for f in filters {
        if f.value.trim().is_empty() && f.op != "isnotnull" {
            continue;
        }
        let entry = match f.op.as_str() {
            "isnotnull" => {
                serde_json::json!({ "field": f.field_id, "op": "isnotnull" })
            }
            "between" => {
                let parts: Vec<&str> = f.value.split(',').collect();
                if parts.len() != 2 {
                    continue;
                }
                let a: f64 = match parts[0].trim().parse() { Ok(v) => v, Err(_) => continue };
                let b: f64 = match parts[1].trim().parse() { Ok(v) => v, Err(_) => continue };
                serde_json::json!({ "field": f.field_id, "op": "between", "value": [a, b] })
            }
            op => {
                let v: f64 = match f.value.trim().parse() { Ok(v) => v, Err(_) => continue };
                serde_json::json!({ "field": f.field_id, "op": op, "value": v })
            }
        };
        out.push(entry);
    }
    serde_json::json!({ "filters": out, "limit": 200 })
}

#[component]
fn FilterRow(
    idx: usize,
    filter: UiFilter,
    fill_pct: Option<f64>,
    catalog: Vec<CatalogEntry>,
    on_change: EventHandler<UiFilter>,
    on_remove: EventHandler<()>,
) -> Element {
    let _ = (idx, catalog);
    let f = filter.clone();
    let f_for_op = filter.clone();
    let f_for_val = filter.clone();
    let low = is_low_coverage(fill_pct);
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 0.25rem;",
            div { style: "display: grid; grid-template-columns: minmax(200px,2fr) 110px minmax(140px,1.5fr) auto; gap: 0.4rem; align-items: center;",
                div { style: "padding: 0.45rem 0.6rem; background: #f4f6fa; border-radius: 8px;",
                    strong { "{f.label}" }
                    span { style: "color: #555; margin-left: 0.4rem; font-size: 0.85rem;", "{f.category_label}" }
                    if f.needs_review {
                        span { style: "margin-left: 0.4rem; padding: 0 0.3rem; font-size: 0.7rem; background: #ffe7c2; color: #5a3b00; border-radius: 4px;", "review" }
                    }
                    if low {
                        span { style: "margin-left: 0.4rem; padding: 0 0.3rem; font-size: 0.7rem; background: #fce8e6; color: #c5221f; border-radius: 4px;", "low coverage" }
                    }
                }
                select {
                    style: "padding: 0.4rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                    value: "{f.op}",
                    onchange: move |ev| {
                        let mut next = f_for_op.clone();
                        next.op = ev.value();
                        on_change.call(next);
                    },
                    option { value: "gt", ">" }
                    option { value: "gte", ">=" }
                    option { value: "lt", "<" }
                    option { value: "lte", "<=" }
                    option { value: "eq", "=" }
                    option { value: "between", "between" }
                    option { value: "isnotnull", "is set" }
                }
                input {
                    style: "padding: 0.4rem 0.6rem; border: 1px solid #d5dbe3; border-radius: 8px;",
                    placeholder: input_placeholder(&f.op, &f.unit),
                    value: "{f.value}",
                    disabled: f.op == "isnotnull",
                    oninput: move |ev| {
                        let mut next = f_for_val.clone();
                        next.value = ev.value();
                        on_change.call(next);
                    },
                }
                button {
                    style: "padding: 0.4rem 0.7rem; background: #fff; border: 1px solid #cad1de; border-radius: 8px; cursor: pointer;",
                    onclick: move |_| on_remove.call(()),
                    "✕"
                }
            }
            if low {
                LowCoverageNotice {
                    label: f.label.clone(),
                    fill_pct: fill_pct.unwrap_or(0.0),
                }
            }
        }
    }
}

#[component]
fn LowCoverageBanner(items: Vec<(String, f64)>) -> Element {
    rsx! {
        div {
            style: "margin: 0 0 0.75rem; padding: 0.55rem 0.75rem; background: #fff8e1; border: 1px solid #f0c14b; border-radius: 8px; font-size: 0.9rem; color: #5a4300;",
            strong { "Low data coverage: " }
            "These filters use metrics filled for less than {LOW_COVERAGE_THRESHOLD_PCT:.0}% of stocks — your screen may return very few matches or miss valid candidates."
            ul { style: "margin: 0.35rem 0 0; padding-left: 1.2rem;",
                for (label, pct) in items.iter() {
                    li { "{label} ({format_fill_pct(*pct)}% filled)" }
                }
            }
        }
    }
}

#[component]
fn LowCoverageNotice(label: String, fill_pct: f64) -> Element {
    rsx! {
        p {
            style: "margin: 0; padding: 0.4rem 0.6rem; background: #fff8e1; border: 1px solid #f0c14b; border-radius: 6px; font-size: 0.85rem; color: #5a4300;",
            "⚠ "
            strong { "{label}" }
            " has low data coverage ({format_fill_pct(fill_pct)}% filled). Results may be incomplete."
        }
    }
}

fn input_placeholder(op: &str, unit: &str) -> String {
    match op {
        "between" => "min, max".to_string(),
        "isnotnull" => "—".to_string(),
        _ => format!("value ({unit})"),
    }
}

#[component]
fn StatusBanner(status: Option<Result<ScreenerStatus, String>>) -> Element {
    match status {
        Some(Ok(s)) => rsx! {
            div { style: "padding: 0.55rem 0.8rem; background: #f0f4ff; border: 1px solid #c8d2f0; border-radius: 8px; font-size: 0.92rem;",
                strong { "Universe " } "{s.universe_size} " strong { "Pending refresh " } "{s.pending_count} "
                strong { "Scheduler " } if s.running { "running" } else { "idle" }
                if s.backfill_running {
                    span { style: "margin-left: 0.75rem; color: #5a4300; font-weight: 600;",
                        "· Full refresh in progress"
                    }
                }
            }
        },
        Some(Err(e)) => rsx! { p { style: "color: #b00020;", "Status error: {e}" } },
        None => rsx! { p { style: "color: #555;", "Loading status…" } },
    }
}

#[component]
fn ResultsTable(rows: Vec<ScreenRow>, filters: Vec<UiFilter>, catalog: Vec<CatalogEntry>) -> Element {
    // Always show identity columns + every column referenced by a filter, in order.
    let mut metric_cols: Vec<String> = Vec::new();
    for f in &filters {
        if !metric_cols.iter().any(|c| c == &f.field_id) {
            metric_cols.push(f.field_id.clone());
        }
    }
    let labels: std::collections::BTreeMap<String, String> = filters
        .iter()
        .map(|f| (f.field_id.clone(), f.label.clone()))
        .collect();
    let units: std::collections::HashMap<String, String> = catalog
        .iter()
        .flat_map(|e| {
            [
                (e.id.clone(), e.unit.clone()),
                (e.column.clone(), e.unit.clone()),
            ]
        })
        .collect();

    rsx! {
        div { style: "{CARD} overflow-x: auto;",
            table { style: "width: 100%; border-collapse: collapse; font-size: 0.92rem;",
                thead {
                    tr { style: "text-align: left; border-bottom: 1px solid #dfe3eb;",
                        th { style: "padding: 0.4rem 0.6rem;", "Symbol" }
                        th { style: "padding: 0.4rem 0.6rem;", "Name" }
                        th { style: "padding: 0.4rem 0.6rem;", "Sector" }
                        th { style: "padding: 0.4rem 0.6rem;", "Industry" }
                        for col in metric_cols.iter() {
                            th { style: "padding: 0.4rem 0.6rem; text-align: right;", "{labels.get(col).cloned().unwrap_or_else(|| col.clone())}" }
                        }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        tr { style: "border-bottom: 1px solid #f0f1f5;",
                            td { style: "padding: 0.4rem 0.6rem;",
                                Link { to: Route::Report { symbol: r.symbol.clone() }, style: "color: #184ad8;", "{r.symbol}" }
                            }
                            td { style: "padding: 0.4rem 0.6rem;", "{r.short_name.clone().unwrap_or_default()}" }
                            td { style: "padding: 0.4rem 0.6rem;", "{r.sector.clone().unwrap_or_default()}" }
                            td { style: "padding: 0.4rem 0.6rem;", "{r.industry.clone().unwrap_or_default()}" }
                            for col in metric_cols.iter() {
                                td { style: "padding: 0.4rem 0.6rem; text-align: right; font-variant-numeric: tabular-nums;",
                                    "{format_metric_cell(r.metrics.get(col), units.get(col).map(String::as_str))}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_metric_cell(v: Option<&serde_json::Value>, unit: Option<&str>) -> String {
    let raw = v.and_then(|x| x.as_f64());
    match unit {
        Some(u) => fmt_screener_metric(raw, u),
        None => match v {
            Some(serde_json::Value::Number(n)) => n
                .as_f64()
                .map(|f| format!("{f:.2}"))
                .unwrap_or_else(|| "-".to_string()),
            _ => "-".to_string(),
        },
    }
}

#[component]
fn SavedScreenRow(
    screen: SavedScreen,
    catalog: Vec<CatalogEntry>,
    on_load: EventHandler<SavedScreen>,
    on_delete: EventHandler<i64>,
) -> Element {
    let _ = catalog;
    let s_for_load = screen.clone();
    let id = screen.id;
    let n = filter_count(&screen.filters);
    rsx! {
        div { style: "display: flex; gap: 0.4rem; align-items: center; padding: 0.4rem 0.6rem; background: #f4f6fa; border-radius: 8px;",
            strong { style: "flex: 1;", "{screen.name}" }
            span { style: "color: #555; font-size: 0.85rem;", "{n} filter(s)" }
            button {
                style: "padding: 0.32rem 0.7rem; background: #fff; border: 1px solid #cad1de; border-radius: 6px; cursor: pointer;",
                onclick: move |_| on_load.call(s_for_load.clone()),
                "Load"
            }
            button {
                style: "padding: 0.32rem 0.7rem; background: #fff; border: 1px solid #cad1de; border-radius: 6px; cursor: pointer;",
                onclick: move |_| on_delete.call(id),
                "Delete"
            }
        }
    }
}

fn filter_count(filters_json: &serde_json::Value) -> usize {
    filters_json.as_array().map(|a| a.len()).unwrap_or(0)
}

fn parse_saved(s: &SavedScreen, catalog: &[CatalogEntry]) -> Vec<UiFilter> {
    let mut out = Vec::new();
    let arr = match s.filters.as_array() {
        Some(a) => a,
        None => return out,
    };
    for item in arr {
        let field_id = match item.get("field").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => continue,
        };
        let op = item
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("gte")
            .to_string();
        let value = match item.get("value") {
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f.to_string()))
                .collect::<Vec<_>>()
                .join(","),
            _ => String::new(),
        };
        let spec = catalog.iter().find(|c| c.id == field_id);
        out.push(UiFilter {
            field_id,
            op,
            value,
            label: spec.map(|s| s.label.clone()).unwrap_or_default(),
            unit: spec.map(|s| s.unit.clone()).unwrap_or_default(),
            category_label: spec.map(|s| s.category_label.clone()).unwrap_or_default(),
            needs_review: spec.map(|s| s.needs_review).unwrap_or(false),
        });
    }
    out
}

fn log_warn(msg: &str) {
    #[cfg(feature = "web")]
    {
        web_sys_log_warn(msg);
    }
    #[cfg(not(feature = "web"))]
    {
        eprintln!("warn: {msg}");
    }
}

#[cfg(feature = "web")]
fn web_sys_log_warn(msg: &str) {
    // dioxus pulls in web-sys; falling back to console via stdlib eprintln in WASM is a no-op.
    let _ = msg;
}
