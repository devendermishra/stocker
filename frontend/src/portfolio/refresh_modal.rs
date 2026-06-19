use std::collections::HashSet;

use dioxus::prelude::*;

use crate::portfolio::styles::{BTN_OUTLINE, BTN_PRIMARY, CARD};
use crate::portfolio_api::{
    apply_portfolio_refresh, fmt_inr, scan_portfolio_refresh, PortfolioRefreshApplyResult,
    PortfolioRefreshScan,
};

const OVERLAY: &str = "position: fixed; inset: 0; background: rgba(0,0,0,0.35); z-index: 1000; display: flex; align-items: center; justify-content: center; padding: 1rem;";
const MODAL: &str = "background: #fff; border-radius: 12px; max-width: 900px; width: 100%; max-height: 90vh; overflow: auto; padding: 1.25rem; box-shadow: 0 8px 32px rgba(0,0,0,0.15);";

#[component]
pub fn PortfolioRefreshModal(
    portfolio_id: i64,
    on_close: EventHandler<()>,
    on_applied: EventHandler<String>,
) -> Element {
    let mut loading = use_signal(|| true);
    let mut applying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut scan = use_signal(|| None::<PortfolioRefreshScan>);
    let mut selected = use_signal(HashSet::<String>::new);

    use_effect(move || {
        spawn(async move {
            loading.set(true);
            error.set(None);
            match scan_portfolio_refresh(portfolio_id).await {
                Ok(result) => {
                    let mut ids = HashSet::new();
                    for item in &result.corporate_actions {
                        ids.insert(item.suggestion_id.clone());
                    }
                    for item in &result.sip_pending {
                        ids.insert(item.suggestion_id.clone());
                    }
                    for item in &result.sip_suggested {
                        ids.insert(item.suggestion_id.clone());
                    }
                    for item in &result.swp_pending {
                        ids.insert(item.suggestion_id.clone());
                    }
                    for item in &result.swp_suggested {
                        ids.insert(item.suggestion_id.clone());
                    }
                    selected.set(ids);
                    scan.set(Some(result));
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let mut toggle = move |id: String| {
        selected.with_mut(|set| {
            if set.contains(&id) {
                set.remove(&id);
            } else {
                set.insert(id);
            }
        });
    };

    rsx! {
        div { style: "{OVERLAY}",
            div { style: "{MODAL}",
                h2 { style: "margin-top: 0;", "Refresh portfolio" }
                p { style: "color: #666; font-size: 0.9rem; margin-bottom: 1rem;",
                    "Review missing corporate actions and SIP installments before applying."
                }

                if loading() {
                    p { "Scanning portfolio…" }
                } else if let Some(err) = error() {
                    p { style: "color: #b00020;", "{err}" }
                } else if let Some(data) = scan() {
                    if !data.scan_errors.is_empty() {
                        div { style: "margin-bottom: 1rem; padding: 0.75rem; background: #fff8e1; border: 1px solid #ffe082; border-radius: 8px; font-size: 0.85rem;",
                            strong { "Scan warnings" }
                            ul { style: "margin: 0.5rem 0 0 1.25rem;",
                                for w in data.scan_errors.iter() {
                                    li { "{w.symbol}: {w.reason}" }
                                }
                            }
                        }
                    }

                    RefreshSection {
                        title: "Corporate actions (dividends & splits)".to_string(),
                        note: Some("Bonus shares cannot be auto-detected; add them manually via Add transaction.".to_string()),
                        empty: "No missing dividends or splits detected.".to_string(),
                        has_items: !data.corporate_actions.is_empty(),
                        children: rsx! {
                            div { style: "overflow-x: auto;",
                                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                                    thead {
                                        tr { style: "background: #f6f8fb; text-align: left;",
                                            th { style: "padding: 0.4rem;", "" }
                                            th { style: "padding: 0.4rem;", "Symbol" }
                                            th { style: "padding: 0.4rem;", "Type" }
                                            th { style: "padding: 0.4rem;", "Date" }
                                            th { style: "padding: 0.4rem;", "Details" }
                                        }
                                    }
                                    tbody {
                                        for item in data.corporate_actions.iter() {
                                            tr { style: "border-top: 1px solid #eee;",
                                                td { style: "padding: 0.4rem;",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected().contains(&item.suggestion_id),
                                                        onchange: {
                                                            let id = item.suggestion_id.clone();
                                                            move |_| toggle(id.clone())
                                                        },
                                                    }
                                                }
                                                td { style: "padding: 0.4rem;", "{item.symbol}" }
                                                td { style: "padding: 0.4rem;", "{item.txn_type}" }
                                                td { style: "padding: 0.4rem;", "{item.trade_date}" }
                                                td { style: "padding: 0.4rem;",
                                                    if item.txn_type == "dividend" {
                                                        {
                                                            let dps = item.dividend_per_share.unwrap_or(0.0);
                                                            let qty = item.eligible_quantity.unwrap_or(0.0);
                                                            let gross = item.gross_amount.unwrap_or(0.0);
                                                            format!(
                                                                "DPS {dps:.2} × {qty:.2} = {}",
                                                                fmt_inr(gross)
                                                            )
                                                        }
                                                    } else {
                                                        {
                                                            let num = item.split_ratio_num.unwrap_or(0.0);
                                                            let den = item.split_ratio_den.unwrap_or(1.0);
                                                            format!("Ratio {num}:{den}")
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }

                    RefreshSection {
                        title: "SIPs to materialize".to_string(),
                        note: None,
                        empty: "No pending SIP rows need materialization.".to_string(),
                        has_items: !data.sip_pending.is_empty(),
                        children: rsx! {
                            div { style: "overflow-x: auto;",
                                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                                    thead {
                                        tr { style: "background: #f6f8fb; text-align: left;",
                                            th { style: "padding: 0.4rem;", "" }
                                            th { style: "padding: 0.4rem;", "Symbol" }
                                            th { style: "padding: 0.4rem;", "SIP date" }
                                            th { style: "padding: 0.4rem;", "Amount" }
                                        }
                                    }
                                    tbody {
                                        for item in data.sip_pending.iter() {
                                            tr { style: "border-top: 1px solid #eee;",
                                                td { style: "padding: 0.4rem;",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected().contains(&item.suggestion_id),
                                                        onchange: {
                                                            let id = item.suggestion_id.clone();
                                                            move |_| toggle(id.clone())
                                                        },
                                                    }
                                                }
                                                td { style: "padding: 0.4rem;", "{item.symbol}" }
                                                td { style: "padding: 0.4rem;", "{item.trade_date}" }
                                                td { style: "padding: 0.4rem;", "{fmt_inr(item.amount)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }

                    RefreshSection {
                        title: "Missing SIP installments".to_string(),
                        note: Some("Selecting an installment registers the SIP and materializes it into a buy.".to_string()),
                        empty: "No missing monthly SIP installments detected.".to_string(),
                        has_items: !data.sip_suggested.is_empty(),
                        children: rsx! {
                            div { style: "overflow-x: auto;",
                                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                                    thead {
                                        tr { style: "background: #f6f8fb; text-align: left;",
                                            th { style: "padding: 0.4rem;", "" }
                                            th { style: "padding: 0.4rem;", "Symbol" }
                                            th { style: "padding: 0.4rem;", "Suggested date" }
                                            th { style: "padding: 0.4rem;", "Amount" }
                                        }
                                    }
                                    tbody {
                                        for item in data.sip_suggested.iter() {
                                            tr { style: "border-top: 1px solid #eee;",
                                                td { style: "padding: 0.4rem;",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected().contains(&item.suggestion_id),
                                                        onchange: {
                                                            let id = item.suggestion_id.clone();
                                                            move |_| toggle(id.clone())
                                                        },
                                                    }
                                                }
                                                td { style: "padding: 0.4rem;", "{item.symbol}" }
                                                td { style: "padding: 0.4rem;", "{item.trade_date}" }
                                                td { style: "padding: 0.4rem;", "{fmt_inr(item.amount)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }

                    RefreshSection {
                        title: "SWPs to materialize".to_string(),
                        note: None,
                        empty: "No pending SWP rows need materialization.".to_string(),
                        has_items: !data.swp_pending.is_empty(),
                        children: rsx! {
                            div { style: "overflow-x: auto;",
                                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                                    thead {
                                        tr { style: "background: #f6f8fb; text-align: left;",
                                            th { style: "padding: 0.4rem;", "" }
                                            th { style: "padding: 0.4rem;", "Symbol" }
                                            th { style: "padding: 0.4rem;", "SWP date" }
                                            th { style: "padding: 0.4rem;", "Amount" }
                                        }
                                    }
                                    tbody {
                                        for item in data.swp_pending.iter() {
                                            tr { style: "border-top: 1px solid #eee;",
                                                td { style: "padding: 0.4rem;",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected().contains(&item.suggestion_id),
                                                        onchange: {
                                                            let id = item.suggestion_id.clone();
                                                            move |_| toggle(id.clone())
                                                        },
                                                    }
                                                }
                                                td { style: "padding: 0.4rem;", "{item.symbol}" }
                                                td { style: "padding: 0.4rem;", "{item.trade_date}" }
                                                td { style: "padding: 0.4rem;", "{fmt_inr(item.amount)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }

                    RefreshSection {
                        title: "Missing SWP installments".to_string(),
                        note: Some("Selecting an installment registers the SWP and materializes it into a sell.".to_string()),
                        empty: "No missing monthly SWP installments detected.".to_string(),
                        has_items: !data.swp_suggested.is_empty(),
                        children: rsx! {
                            div { style: "overflow-x: auto;",
                                table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                                    thead {
                                        tr { style: "background: #f6f8fb; text-align: left;",
                                            th { style: "padding: 0.4rem;", "" }
                                            th { style: "padding: 0.4rem;", "Symbol" }
                                            th { style: "padding: 0.4rem;", "Suggested date" }
                                            th { style: "padding: 0.4rem;", "Amount" }
                                        }
                                    }
                                    tbody {
                                        for item in data.swp_suggested.iter() {
                                            tr { style: "border-top: 1px solid #eee;",
                                                td { style: "padding: 0.4rem;",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: selected().contains(&item.suggestion_id),
                                                        onchange: {
                                                            let id = item.suggestion_id.clone();
                                                            move |_| toggle(id.clone())
                                                        },
                                                    }
                                                }
                                                td { style: "padding: 0.4rem;", "{item.symbol}" }
                                                td { style: "padding: 0.4rem;", "{item.trade_date}" }
                                                td { style: "padding: 0.4rem;", "{fmt_inr(item.amount)}" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }

                div { style: "display: flex; gap: 0.5rem; margin-top: 1.25rem; flex-wrap: wrap;",
                    button {
                        style: "{BTN_PRIMARY}",
                        disabled: loading() || applying() || scan().is_none(),
                        onclick: move |_| {
                            let selections: Vec<String> = selected().into_iter().collect();
                            if selections.is_empty() {
                                return;
                            }
                            spawn(async move {
                                applying.set(true);
                                match apply_portfolio_refresh(portfolio_id, &selections).await {
                                    Ok(result) => {
                                        on_applied.call(format_apply_summary(&result));
                                        on_close.call(());
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                                applying.set(false);
                            });
                        },
                        if applying() { "Applying…" } else { "Apply selected" }
                    }
                    button {
                        style: "{BTN_OUTLINE}",
                        disabled: applying(),
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

fn format_apply_summary(result: &PortfolioRefreshApplyResult) -> String {
    let mut parts = Vec::new();
    if result.corporate_actions_created > 0 {
        parts.push(format!(
            "{} corporate action(s)",
            result.corporate_actions_created
        ));
    }
    if result.sip_registered > 0 {
        parts.push(format!("{} SIP(s) registered", result.sip_registered));
    }
    if result.sip_materialized > 0 {
        parts.push(format!("{} SIP(s) materialized", result.sip_materialized));
    }
    if result.swp_registered > 0 {
        parts.push(format!("{} SWP(s) registered", result.swp_registered));
    }
    if result.swp_materialized > 0 {
        parts.push(format!("{} SWP(s) materialized", result.swp_materialized));
    }
    if parts.is_empty() && result.failed.is_empty() {
        return "Nothing applied.".into();
    }
    let mut msg = if parts.is_empty() {
        String::new()
    } else {
        format!("Applied {}.", parts.join(", "))
    };
    if !result.failed.is_empty() {
        if !msg.is_empty() {
            msg.push(' ');
        }
        msg.push_str(&format!("{} failed.", result.failed.len()));
    }
    msg
}

#[component]
fn RefreshSection(
    title: String,
    note: Option<String>,
    empty: String,
    has_items: bool,
    children: Element,
) -> Element {
    rsx! {
        div { style: "{CARD}",
            h3 { style: "margin-top: 0; font-size: 1rem;", "{title}" }
            if let Some(n) = note {
                p { style: "color: #666; font-size: 0.85rem; margin: 0 0 0.75rem 0;", "{n}" }
            }
            if has_items {
                {children}
            } else {
                p { style: "color: #888; font-size: 0.85rem; margin: 0;", "{empty}" }
            }
        }
    }
}
