use dioxus::prelude::*;

use crate::portfolio::styles::{BTN_OUTLINE, BTN_PRIMARY, FORM_PANEL, INPUT};
use crate::portfolio_api::{
    apply_import, parse_import_file, preview_import, txn_type_label, ImportApplyRequest,
    ImportField, ImportResult, ImportRowPreview, ParsePreview,
};

const BTN_SEC: &str = BTN_OUTLINE;

#[component]
pub fn TransactionImport(
    portfolio_id: i64,
    on_done: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let mut step = use_signal(|| 0u8);
    let mut preview = use_signal(|| None::<ParsePreview>);
    let mut header_row = use_signal(|| 0usize);
    let mut column_mapping = use_signal(Vec::<ImportField>::new);
    let mut row_previews = use_signal(Vec::<ImportRowPreview>::new);
    let mut import_result = use_signal(|| None::<ImportResult>);
    let mut loading = use_signal(|| false);

    rsx! {
        div { style: "{FORM_PANEL}",
            h3 { style: "margin-top: 0;", "Import transactions" }
            p { style: "color: #555; font-size: 0.9rem;", "Upload a CSV, XLS, or XLSX file. Map columns to transaction fields, preview, then import." }

            if step() == 0 {
                label { style: "display: block; margin-bottom: 0.75rem;",
                    "Choose file"
                    input {
                        r#type: "file",
                        accept: ".csv,.xls,.xlsx",
                        style: "{INPUT}; display: block; margin-top: 0.35rem;",
                        onchange: move |evt| {
                            loading.set(true);
                            spawn(async move {
                                match read_uploaded_file(&evt).await {
                                    Ok((name, bytes)) => {
                                        match parse_import_file(portfolio_id, &name, &bytes).await {
                                            Ok(p) => {
                                                header_row.set(p.suggested_header_row);
                                                column_mapping.set(p.suggested_mapping.clone());
                                                preview.set(Some(p));
                                                step.set(1);
                                                import_result.set(None);
                                            }
                                            Err(e) => on_error.call(e),
                                        }
                                    }
                                    Err(e) => on_error.call(e),
                                }
                                loading.set(false);
                            });
                        },
                    }
                }
                if loading() {
                    p { "Parsing file…" }
                }
            }

            if step() >= 1 {
                if let Some(p) = preview() {
                    div { style: "margin-bottom: 1rem;",
                        label { "Header row "
                            select {
                                style: "{INPUT}",
                                value: "{header_row}",
                                onchange: move |ev| {
                                    if let Ok(v) = ev.value().parse::<usize>() {
                                        header_row.set(v);
                                        if let Some(prev) = preview() {
                                            column_mapping.set(
                                                prev.grid.rows.get(v).cloned().unwrap_or_default()
                                                    .iter()
                                                    .map(|h| suggest_field_for_header(h))
                                                    .collect(),
                                            );
                                        }
                                    }
                                },
                                for i in 0..p.grid.rows.len().min(15) {
                                    option {
                                        value: "{i}",
                                        selected: header_row() == i,
                                        "Row {i + 1}: {row_preview(&p.grid.rows[i])}"
                                    }
                                }
                            }
                        }
                    }

                    div { style: "margin-bottom: 1rem;",
                        h4 { style: "margin: 0 0 0.5rem;", "Column mapping" }
                        div { style: "display: grid; gap: 0.5rem;",
                            for (col_idx, header) in p.grid.rows.get(header_row()).into_iter().flatten().enumerate() {
                                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; align-items: center;",
                                    span { style: "font-size: 0.85rem;", "{header}" }
                                    select {
                                        style: "{INPUT}",
                                        onchange: move |ev| {
                                            let field = ImportField::parse_api_key(ev.value().as_str());
                                            column_mapping.with_mut(|m| {
                                                if col_idx >= m.len() {
                                                    m.resize(col_idx + 1, ImportField::Skip);
                                                }
                                                m[col_idx] = field;
                                            });
                                        },
                                        for f in ImportField::all_mappable() {
                                            option {
                                                value: "{f.api_key()}",
                                                selected: column_mapping().get(col_idx) == Some(f),
                                                "{import_field_label(*f)}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { style: "display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 1rem;",
                        button {
                            style: "{BTN_SEC}",
                            disabled: loading(),
                            onclick: move |_| {
                                if let Some(prev) = preview() {
                                    let req = build_apply_request(&prev, header_row(), column_mapping());
                                    loading.set(true);
                                    spawn(async move {
                                        match preview_import(portfolio_id, &req).await {
                                            Ok(rows) => {
                                                row_previews.set(rows);
                                                step.set(2);
                                            }
                                            Err(e) => on_error.call(e),
                                        }
                                        loading.set(false);
                                    });
                                }
                            },
                            "Preview"
                        }
                    }
                }
            }

            if step() >= 2 {
                if let Some(result) = import_result() {
                    p { style: "color: #0d6832;",
                        "Imported {result.imported} transactions. Skipped {result.skipped}."
                    }
                    if !result.errors.is_empty() {
                        ul { style: "color: #b00020; font-size: 0.85rem;",
                            for err in result.errors.iter().take(10) {
                                li { "Row {err.row_index + 1}: {err.message}" }
                            }
                        }
                    }
                    button {
                        style: "{BTN_PRIMARY}; margin-top: 0.5rem;",
                        onclick: move |_| on_done.call(()),
                        "Done"
                    }
                } else {
                    p {
                        "{row_previews().iter().filter(|r| r.transaction.is_some()).count()} rows ready, {row_previews().iter().filter(|r| r.error.is_some()).count()} with errors"
                    }
                    div { style: "overflow-x: auto; max-height: 280px; margin-bottom: 1rem;",
                        table { style: "width: 100%; border-collapse: collapse; font-size: 0.8rem;",
                            thead {
                                tr { style: "background: #eef2f7; text-align: left;",
                                    th { style: "padding: 0.4rem;", "Row" }
                                    th { style: "padding: 0.4rem;", "Date" }
                                    th { style: "padding: 0.4rem;", "Type" }
                                    th { style: "padding: 0.4rem;", "Symbol" }
                                    th { style: "padding: 0.4rem;", "Qty" }
                                    th { style: "padding: 0.4rem;", "Status" }
                                }
                            }
                            tbody {
                                for row in row_previews().iter().take(50) {
                                    {
                                        let qty_str = row.transaction.as_ref()
                                            .and_then(|t| t.quantity)
                                            .map(|q| format!("{q:.2}"))
                                            .unwrap_or_default();
                                        rsx! {
                                    tr { style: "border-top: 1px solid #eee;",
                                        td { style: "padding: 0.4rem;", "{row.row_index + 1}" }
                                        if let Some(txn) = &row.transaction {
                                            td { style: "padding: 0.4rem;", "{txn.trade_date}" }
                                            td { style: "padding: 0.4rem;", "{txn_type_label(&txn.txn_type)}" }
                                            td { style: "padding: 0.4rem;", "{txn.symbol.clone().unwrap_or_default()}" }
                                            td { style: "padding: 0.4rem;", "{qty_str}" }
                                            td { style: "padding: 0.4rem; color: #0d6832;", "OK" }
                                        } else {
                                            td { colspan: "4", style: "padding: 0.4rem; color: #b00020;",
                                                "{row.error.clone().unwrap_or_default()}"
                                            }
                                            td { style: "padding: 0.4rem; color: #b00020;", "Error" }
                                        }
                                    }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { style: "display: flex; gap: 0.5rem;",
                        button {
                            style: "{BTN_PRIMARY}",
                            disabled: loading(),
                            onclick: move |_| {
                                if let Some(prev) = preview() {
                                    let req = build_apply_request(&prev, header_row(), column_mapping());
                                    loading.set(true);
                                    spawn(async move {
                                        match apply_import(portfolio_id, &req).await {
                                            Ok(r) => {
                                                import_result.set(Some(r));
                                                step.set(3);
                                            }
                                            Err(e) => on_error.call(e),
                                        }
                                        loading.set(false);
                                    });
                                }
                            },
                            if loading() { "Importing…" } else { "Import valid rows" }
                        }
                        button {
                            style: "{BTN_SEC}",
                            onclick: move |_| {
                                step.set(0);
                                preview.set(None);
                                row_previews.set(vec![]);
                            },
                            "Start over"
                        }
                    }
                }
            }
        }
    }
}

fn build_apply_request(
    preview: &ParsePreview,
    header_row: usize,
    column_mapping: Vec<ImportField>,
) -> ImportApplyRequest {
    ImportApplyRequest {
        header_row,
        column_mapping,
        grid: preview.grid.clone(),
    }
}

fn row_preview(row: &[String]) -> String {
    let joined: String = row.iter().take(4).cloned().collect::<Vec<_>>().join(" | ");
    if joined.len() > 80 {
        format!("{}…", &joined[..80])
    } else {
        joined
    }
}

fn suggest_field_for_header(header: &str) -> ImportField {
    #[cfg(all(feature = "desktop", not(feature = "web")))]
    {
        return convert_import_field(stocker_portfolio::import::suggest_field(header));
    }
    #[cfg(all(feature = "web", not(feature = "desktop")))]
    {
        suggest_field_web(header)
    }
}

#[cfg(all(feature = "desktop", not(feature = "web")))]
fn convert_import_field(field: stocker_portfolio::ImportField) -> ImportField {
    serde_json::from_value(serde_json::to_value(field).expect("ImportField roundtrip"))
        .expect("ImportField roundtrip")
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn suggest_field_web(header: &str) -> ImportField {
    let h = header.trim().to_lowercase();
    if h.contains("transaction date") || h == "date" || h == "trade date" {
        ImportField::TradeDate
    } else if h.contains("transaction type") || h == "type" {
        ImportField::TxnType
    } else if h.contains("stock") || h.contains("etf name") || h.contains("security name")
        || h.contains("fund") || h.contains("scheme")
    {
        ImportField::StockName
    } else if h == "symbol" || h.contains("ticker") {
        ImportField::Symbol
    } else if h.contains("isin") {
        ImportField::Isin
    } else if h.contains("balance") && (h.contains("quantity") || h.contains("unit")) {
        ImportField::EligibleQuantity
    } else if h == "units" || h.contains("quantity") {
        ImportField::Quantity
    } else if h == "nav" || h == "price" {
        ImportField::Price
    } else if h == "amount" || h.contains("gross") {
        ImportField::GrossAmount
    } else if h.contains("brokerage") {
        ImportField::Brokerage
    } else if h.contains("tax") {
        ImportField::Taxes
    } else if h.contains("net amount") || h == "net" {
        ImportField::NetAmount
    } else if h.contains("dividend") {
        ImportField::DividendPerShare
    } else if h.contains("note") {
        ImportField::Notes
    } else {
        ImportField::Skip
    }
}

pub fn import_field_label(f: ImportField) -> &'static str {
    f.label()
}

async fn read_uploaded_file(evt: &FormEvent) -> Result<(String, Vec<u8>), String> {
    let files = evt
        .files()
        .ok_or_else(|| "no files in event".to_string())?;
    let names = files.files();
    let path_or_name = names
        .first()
        .cloned()
        .ok_or_else(|| "no file selected".to_string())?;
    let bytes = files
        .read_file(&path_or_name)
        .await
        .ok_or_else(|| "failed to read file".to_string())?;
    let filename = std::path::Path::new(&path_or_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&path_or_name)
        .to_string();
    Ok((filename, bytes))
}
