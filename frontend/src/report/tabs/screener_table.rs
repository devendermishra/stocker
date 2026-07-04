use dioxus::prelude::*;

const TABLE: &str = "width:100%; border-collapse:collapse; font-size:0.84rem; line-height:1.35;";
const TH_PERIOD: &str = "text-align:right; padding:0.5rem 0.65rem; border-bottom:2px solid #e6e9ef; color:#222; font-weight:600; white-space:nowrap; background:#fafbfc;";
const TH_CORNER: &str = "text-align:left; padding:0.5rem 0.65rem; border-bottom:2px solid #e6e9ef; background:#fafbfc; min-width:9rem;";
const TD_LABEL: &str = "text-align:left; padding:0.45rem 0.65rem; border-bottom:1px solid #eef1f5; color:#333; white-space:nowrap; background:#fafbfc; position:sticky; left:0; z-index:1;";
const TD_VAL: &str = "text-align:right; padding:0.45rem 0.65rem; border-bottom:1px solid #eef1f5; color:#222; white-space:nowrap; font-variant-numeric:tabular-nums;";
const SUBTITLE: &str = "margin:0 0 0.55rem; font-size:0.8rem; color:#6b7280;";

#[derive(Clone, PartialEq)]
pub struct ScreenerRow {
    pub label: String,
    pub values: Vec<String>,
}

#[component]
pub fn ScreenerStatementTable(
    period_labels: Vec<String>,
    rows: Vec<ScreenerRow>,
    subtitle: String,
) -> Element {
    if period_labels.is_empty() || rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        p { style: "{SUBTITLE}", "{subtitle}" }
        div { style: "overflow-x:auto; margin:0 -0.15rem;",
            table { style: "{TABLE}",
                thead {
                    tr {
                        th { style: "{TH_CORNER}", "" }
                        for label in &period_labels {
                            th { style: "{TH_PERIOD}", "{label}" }
                        }
                    }
                }
                tbody {
                    for row in &rows {
                        tr {
                            td { style: "{TD_LABEL}", "{row.label}" }
                            for val in &row.values {
                                td { style: "{TD_VAL}", "{val}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
