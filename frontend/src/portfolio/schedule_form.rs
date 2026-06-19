use dioxus::prelude::*;

use crate::portfolio::styles::{BTN_OUTLINE, BTN_PRIMARY, FORM_PANEL, INPUT};
use crate::portfolio_api::{
    register_mf_schedule, search_mutual_funds, MfSearchHit, RegisterMfSchedule, ScheduleType,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum EndConstraint {
    None,
    EndDate,
    InstallmentCount,
}

#[component]
pub fn MfScheduleForm(
    portfolio_id: i64,
    on_saved: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let mut schedule_type = use_signal(|| ScheduleType::Sip);
    let mut mf_query = use_signal(String::new);
    let mut mf_hits = use_signal(Vec::<MfSearchHit>::new);
    let mut selected_mf = use_signal(|| None::<MfSearchHit>);
    let mut amount = use_signal(|| String::new());
    let mut start_date = use_signal(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let mut end_constraint = use_signal(|| EndConstraint::None);
    let mut end_date = use_signal(String::new);
    let mut installment_count = use_signal(String::new);
    let mut saving = use_signal(|| false);

    rsx! {
        div { style: "{FORM_PANEL}",
            h3 { style: "margin-top: 0;", "Register SIP / SWP" }
            div { style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.75rem;",
                label { "Type"
                    select {
                        style: "{INPUT}",
                        onchange: move |ev| {
                            schedule_type.set(if ev.value() == "swp" {
                                ScheduleType::Swp
                            } else {
                                ScheduleType::Sip
                            });
                        },
                        option { value: "sip", selected: schedule_type() == ScheduleType::Sip, "SIP" }
                        option { value: "swp", selected: schedule_type() == ScheduleType::Swp, "SWP" }
                    }
                }
                label { style: "grid-column: 1 / -1;", "Fund name"
                    input {
                        style: "{INPUT}; width: 100%;",
                        placeholder: "Parag Parikh Flexi Cap Direct Growth",
                        value: "{mf_query}",
                        oninput: move |ev| {
                            let q = ev.value();
                            mf_query.set(q.clone());
                            selected_mf.set(None);
                            if q.trim().len() < 3 {
                                mf_hits.set(Vec::new());
                                return;
                            }
                            spawn(async move {
                                debounce_search().await;
                                match search_mutual_funds(&q).await {
                                    Ok(hits) => mf_hits.set(hits),
                                    Err(_) => mf_hits.set(Vec::new()),
                                }
                            });
                        },
                    }
                }
                if let Some(sel) = selected_mf() {
                    p { style: "grid-column: 1 / -1; margin: 0; font-size: 0.85rem; color: #166534;",
                        "Selected: {sel.scheme_name} ({sel.scheme_code})"
                    }
                } else if !mf_hits().is_empty() {
                    div { style: "grid-column: 1 / -1; border: 1px solid #dfe3eb; border-radius: 8px; max-height: 160px; overflow-y: auto; background: #fff;",
                        for hit in mf_hits().iter().cloned() {
                            {
                                let scheme_code = hit.scheme_code;
                                let scheme_name_label = hit.scheme_name.clone();
                                let scheme_name_pick = hit.scheme_name.clone();
                                let pick = hit;
                                rsx! {
                                    button {
                                        key: "{scheme_code}",
                                        style: "display: block; width: 100%; text-align: left; padding: 0.5rem 0.75rem; border: none; border-bottom: 1px solid #eee; background: #fff; cursor: pointer; font-size: 0.85rem;",
                                        onclick: move |_| {
                                            selected_mf.set(Some(pick.clone()));
                                            mf_query.set(scheme_name_pick.clone());
                                            mf_hits.set(Vec::new());
                                        },
                                        "{scheme_name_label}"
                                    }
                                }
                            }
                        }
                    }
                }
                label { "Monthly amount"
                    input {
                        style: "{INPUT}",
                        r#type: "number",
                        min: "1",
                        step: "any",
                        value: "{amount}",
                        oninput: move |ev| amount.set(ev.value()),
                    }
                }
                label { "Start transaction date"
                    input {
                        style: "{INPUT}",
                        r#type: "date",
                        value: "{start_date}",
                        oninput: move |ev| start_date.set(ev.value()),
                    }
                }
                label { style: "grid-column: 1 / -1;", "End constraint (optional)"
                    div { style: "display: flex; gap: 1rem; flex-wrap: wrap; font-size: 0.85rem;",
                        label {
                            input {
                                r#type: "radio",
                                name: "end_constraint",
                                checked: end_constraint() == EndConstraint::None,
                                onchange: move |_| end_constraint.set(EndConstraint::None),
                            }
                            " Open-ended"
                        }
                        label {
                            input {
                                r#type: "radio",
                                name: "end_constraint",
                                checked: end_constraint() == EndConstraint::EndDate,
                                onchange: move |_| end_constraint.set(EndConstraint::EndDate),
                            }
                            " End date"
                        }
                        label {
                            input {
                                r#type: "radio",
                                name: "end_constraint",
                                checked: end_constraint() == EndConstraint::InstallmentCount,
                                onchange: move |_| end_constraint.set(EndConstraint::InstallmentCount),
                            }
                            " Number of installments"
                        }
                    }
                }
                if end_constraint() == EndConstraint::EndDate {
                    label { "End date"
                        input {
                            style: "{INPUT}",
                            r#type: "date",
                            value: "{end_date}",
                            oninput: move |ev| end_date.set(ev.value()),
                        }
                    }
                }
                if end_constraint() == EndConstraint::InstallmentCount {
                    label { "Installments"
                        input {
                            style: "{INPUT}",
                            r#type: "number",
                            min: "1",
                            step: "1",
                            value: "{installment_count}",
                            oninput: move |ev| installment_count.set(ev.value()),
                        }
                    }
                }
            }
            div { style: "display: flex; gap: 0.75rem; margin-top: 0.75rem; flex-wrap: wrap;",
                button {
                    style: "{BTN_PRIMARY}",
                    disabled: saving(),
                    onclick: move |_| {
                        let Some(mf) = selected_mf() else {
                            on_error.call("Select a mutual fund from search results".into());
                            return;
                        };
                        let amt: f64 = match amount().trim().parse() {
                            Ok(v) if v > 0.0 => v,
                            _ => {
                                on_error.call("Enter a valid monthly amount".into());
                                return;
                            }
                        };
                        if start_date().trim().is_empty() {
                            on_error.call("Start transaction date is required".into());
                            return;
                        }
                        let (end_d, count) = match end_constraint() {
                            EndConstraint::None => (None, None),
                            EndConstraint::EndDate => {
                                if end_date().trim().is_empty() {
                                    on_error.call("End date is required".into());
                                    return;
                                }
                                (Some(end_date()), None)
                            }
                            EndConstraint::InstallmentCount => {
                                let n: u32 = match installment_count().trim().parse() {
                                    Ok(v) if v > 0 => v,
                                    _ => {
                                        on_error.call("Enter a valid installment count".into());
                                        return;
                                    }
                                };
                                (None, Some(n))
                            }
                        };
                        let input = RegisterMfSchedule {
                            schedule_type: schedule_type(),
                            symbol: mf.scheme_name,
                            amount: amt,
                            start_date: Some(start_date()),
                            end_date: end_d,
                            installment_count: count,
                        };
                        saving.set(true);
                        spawn(async move {
                            match register_mf_schedule(portfolio_id, &input).await {
                                Ok(result) => {
                                    if !result.failed.is_empty() {
                                        let msg = result
                                            .failed
                                            .iter()
                                            .map(|f| format!("{}: {}", f.trade_date, f.reason))
                                            .collect::<Vec<_>>()
                                            .join("; ");
                                        on_error.call(format!(
                                            "Registered {} installment(s), {} materialized; failures: {msg}",
                                            result.registered.len(),
                                            result.materialized.len()
                                        ));
                                    }
                                    on_saved.call(());
                                }
                                Err(e) => on_error.call(e),
                            }
                            saving.set(false);
                        });
                    },
                    if saving() { "Saving…" } else { "Save schedule" }
                }
                button {
                    style: "{BTN_OUTLINE}",
                    onclick: move |_| {
                        mf_query.set(String::new());
                        selected_mf.set(None);
                        amount.set(String::new());
                        end_constraint.set(EndConstraint::None);
                        end_date.set(String::new());
                        installment_count.set(String::new());
                    },
                    "Reset"
                }
            }
        }
    }
}

async fn debounce_search() {
    #[cfg(feature = "web")]
    {
        gloo_timers::future::TimeoutFuture::new(300).await;
    }
    #[cfg(all(feature = "desktop", not(feature = "web")))]
    {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
}
