use dioxus::prelude::*;

use crate::portfolio::layout::{AuthGuard, PortfolioNav, PortfolioTab};
use crate::portfolio::schedule_form::MfScheduleForm;
use crate::portfolio::styles::{BTN_OUTLINE, BTN_PRIMARY, CARD};
use crate::portfolio_api::{
    fmt_inr, inactivate_mf_schedule, list_mf_schedules, MfSchedule, ScheduleStatus, ScheduleType,
};
use crate::portfolio_data_revision::portfolio_data_revision;
use crate::routes::Route;

#[component]
pub fn PortfolioSchedules(id: i64) -> Element {
    let mut reload = use_signal(|| 0u32);
    let mut show_form = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut msg = use_signal(|| None::<String>);

    let schedules = use_resource(move || {
        let _ = reload();
        let _ = portfolio_data_revision();
        async move { list_mf_schedules(id).await }
    });

    rsx! {
        AuthGuard {
            div { style: "margin-bottom: 0.75rem;",
                Link { to: Route::PortfolioList {}, style: "color: #1a56db;", "← Portfolios" }
            }
            PortfolioNav { id, active: PortfolioTab::Schedules }
            div { style: "display: flex; gap: 0.75rem; align-items: center; margin-bottom: 1rem; flex-wrap: wrap;",
                h1 { style: "margin: 0; flex: 1;", "SIPs & SWPs" }
                button {
                    style: "{BTN_PRIMARY}",
                    onclick: move |_| show_form.set(!show_form()),
                    if show_form() { "Hide registration form" } else { "Register SIP / SWP" }
                }
            }
            if let Some(m) = msg() {
                p { style: "color: #0d6b2d;", "{m}" }
            }
            if let Some(e) = error() {
                p { style: "color: #b00020;", "{e}" }
            }
            if show_form() {
                MfScheduleForm {
                    portfolio_id: id,
                    on_saved: move || {
                        show_form.set(false);
                        reload.set(reload() + 1);
                        msg.set(Some("Schedule saved.".into()));
                        error.set(None);
                    },
                    on_error: move |e: String| {
                        error.set(Some(e));
                        msg.set(None);
                    },
                }
            }
            match &*schedules.read_unchecked() {
                None => rsx! { p { "Loading schedules…" } },
                Some(Err(e)) => rsx! { p { style: "color: #b00020;", "{e}" } },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { "No SIP or SWP schedules yet. Use Register SIP / SWP to add one." }
                },
                Some(Ok(list)) => rsx! {
                    div { style: "overflow-x: auto;",
                        table { style: "width: 100%; border-collapse: collapse; font-size: 0.85rem;",
                            thead {
                                tr { style: "background: #f6f8fb; text-align: left;",
                                    th { style: "padding: 0.5rem;", "Type" }
                                    th { style: "padding: 0.5rem;", "Fund" }
                                    th { style: "padding: 0.5rem;", "Amount" }
                                    th { style: "padding: 0.5rem;", "Start" }
                                    th { style: "padding: 0.5rem;", "End" }
                                    th { style: "padding: 0.5rem;", "Installments" }
                                    th { style: "padding: 0.5rem;", "Status" }
                                    th { style: "padding: 0.5rem;", "" }
                                }
                            }
                            tbody {
                                for s in list.iter().cloned() {
                                    ScheduleRow {
                                        schedule: s,
                                        on_changed: move || reload.set(reload() + 1),
                                        on_error: move |e: String| error.set(Some(e)),
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ScheduleRow(
    schedule: MfSchedule,
    on_changed: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let inactive = schedule.status == ScheduleStatus::Inactive;
    let row_style = if inactive {
        "opacity: 0.65; background: #fafafa;"
    } else {
        ""
    };
    let type_label = match schedule.schedule_type {
        ScheduleType::Sip => "SIP",
        ScheduleType::Swp => "SWP",
    };
    let fund = schedule
        .scheme_name
        .clone()
        .unwrap_or_else(|| schedule.symbol.clone());
    let end_label = schedule
        .end_date
        .clone()
        .unwrap_or_else(|| "—".into());
    let installments = match schedule.installment_count {
        Some(total) => format!("{} / {}", schedule.registered_installments, total),
        None => format!("{}", schedule.registered_installments),
    };
    let status_badge = if inactive {
        rsx! {
            span { style: "padding: 0.15rem 0.5rem; border-radius: 6px; background: #e5e7eb; color: #374151; font-size: 0.75rem;",
                "Inactive"
            }
        }
    } else {
        rsx! {
            span { style: "padding: 0.15rem 0.5rem; border-radius: 6px; background: #dcfce7; color: #166534; font-size: 0.75rem;",
                "Active"
            }
        }
    };
    let schedule_id = schedule.id;

    rsx! {
        tr { style: "{row_style}",
            td { style: "padding: 0.5rem;", "{type_label}" }
            td { style: "padding: 0.5rem;", "{fund}" }
            td { style: "padding: 0.5rem;", "{fmt_inr(schedule.amount)}" }
            td { style: "padding: 0.5rem;", "{schedule.start_date}" }
            td { style: "padding: 0.5rem;", "{end_label}" }
            td { style: "padding: 0.5rem;", "{installments}" }
            td { style: "padding: 0.5rem;", {status_badge} }
            td { style: "padding: 0.5rem; white-space: nowrap;",
                Link {
                    to: Route::PortfolioTransactions { id: schedule.portfolio_id },
                    style: "color: #1a56db; margin-right: 0.5rem;",
                    "Transactions"
                }
                if !inactive {
                    button {
                        style: "{BTN_OUTLINE}; font-size: 0.8rem; padding: 0.25rem 0.5rem;",
                        onclick: move |_| {
                            spawn(async move {
                                match inactivate_mf_schedule(schedule_id).await {
                                    Ok(_) => on_changed.call(()),
                                    Err(e) => on_error.call(e),
                                }
                            });
                        },
                        "Stop"
                    }
                }
            }
        }
    }
}
