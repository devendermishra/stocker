//! Screener.in consolidated financial tables (quarterly + annual).
//!
//! Used for display-quality Indian financials (8+ quarters). Yahoo FTS only exposes ~5
//! quarterly points for most NSE tickers.

use crate::fetcher::http_client;
use crate::http_policy::screener_get;
use crate::models::{FinancialTable, FinancialTableRow, BankingMetrics, ScreenerFinancials};

fn strip_html(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&#8377;", "₹")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_end_matches('+')
        .trim()
        .to_string()
}

fn find_section_table<'a>(html: &'a str, section_id: &str) -> Option<&'a str> {
    let marker = format!("id=\"{section_id}\"");
    let start = html.find(&marker)?;
    let section = &html[start..];
    let table_marker = "<table class=\"data-table";
    let table_start = section.find(table_marker)?;
    let table_slice = &section[table_start..];
    let end = table_slice.find("</table>")? + "</table>".len();
    Some(&table_slice[..end])
}

fn parse_period_dates(table: &str) -> Vec<String> {
    let mut dates = Vec::new();
    let mut search = table;
    while let Some(i) = search.find("data-date-key=\"") {
        let rest = &search[i + 15..];
        if let Some(end) = rest.find('"') {
            dates.push(rest[..end].to_string());
            search = &rest[end..];
        } else {
            break;
        }
    }
    dates
}

fn parse_table_rows(table: &str, n_periods: usize) -> Vec<FinancialTableRow> {
    let mut rows = Vec::new();
    let mut search = table;
    while let Some(i) = search.find("<tr") {
        let row_start = &search[i..];
        let Some(row_end) = row_start.find("</tr>") else {
            break;
        };
        let row = &row_start[..row_end];
        search = &row_start[row_end + 5..];

        let Some(label_start) = row.find("<td class=\"text\">") else {
            continue;
        };
        let label_html = &row[label_start + 17..];
        let Some(label_end) = label_html.find("</td>") else {
            continue;
        };
        let label = strip_html(&label_html[..label_end]);
        if label.is_empty() {
            continue;
        }

        let values_part = &label_html[label_end + 5..];
        let mut values = Vec::new();
        let mut vp = values_part;
        while let Some(i) = vp.find("<td") {
            let cell = &vp[i..];
            let Some(gt) = cell.find('>') else {
                break;
            };
            let inner = &cell[gt + 1..];
            let Some(end) = inner.find("</td>") else {
                break;
            };
            values.push(strip_html(&inner[..end]));
            vp = &inner[end..];
        }
        if values.len() < n_periods {
            values.resize(n_periods, "—".to_string());
        } else if values.len() > n_periods {
            values.truncate(n_periods);
        }
        let is_pct = values.iter().any(|v| v.ends_with('%')) || label.contains('%');
        rows.push(FinancialTableRow {
            label,
            values,
            is_pct,
        });
    }
    rows
}

fn parse_section(html: &str, section_id: &str, source: &str) -> FinancialTable {
    let Some(table) = find_section_table(html, section_id) else {
        return FinancialTable {
            source: source.to_string(),
            ..FinancialTable::default()
        };
    };
    let period_dates = parse_period_dates(table);
    let rows = parse_table_rows(table, period_dates.len());
    FinancialTable {
        period_dates,
        rows,
        unit: "Rs. Crores".to_string(),
        source: source.to_string(),
    }
}

fn parse_pct_cell(raw: &str) -> Option<f64> {
    let t = raw.trim().trim_end_matches('%').replace(',', "");
    if t.is_empty() || t == "—" || t == "-" {
        return None;
    }
    t.parse().ok()
}

fn latest_pct_from_row(row: &FinancialTableRow) -> Option<f64> {
    row.values.iter().rev().find_map(|v| parse_pct_cell(v))
}

/// GNPA / NNPA from Screener.in quarterly results (NBFCs and some banks).
pub fn bank_metrics_from_quarterly(quarterly: &FinancialTable) -> Option<BankingMetrics> {
    if quarterly.rows.is_empty() {
        return None;
    }
    let mut gnpa = None;
    let mut nnpa = None;
    for row in &quarterly.rows {
        let label = row.label.to_lowercase();
        if label.contains("gross") && label.contains("npa") {
            gnpa = latest_pct_from_row(row);
        } else if label.contains("net") && label.contains("npa") {
            nnpa = latest_pct_from_row(row);
        } else if label.starts_with("gnpa") {
            gnpa = latest_pct_from_row(row);
        } else if label.starts_with("nnpa") {
            nnpa = latest_pct_from_row(row);
        }
    }
    if gnpa.is_none() && nnpa.is_none() {
        return None;
    }
    Some(BankingMetrics {
        gnpa_pct: gnpa,
        nnpa_pct: nnpa,
        as_of_date: quarterly.period_dates.last().cloned(),
        source: Some(quarterly.source.clone()),
        ..BankingMetrics::default()
    })
}

async fn fetch_company_html(base_symbol: &str) -> Option<String> {
    let client = http_client();
    for path in [
        format!("https://www.screener.in/company/{base_symbol}/consolidated/"),
        format!("https://www.screener.in/company/{base_symbol}/"),
    ] {
        match screener_get(client, &path).await {
            Ok(res) if res.status().is_success() => {
                if let Ok(text) = res.text().await {
                    if text.contains("data-table") {
                        return Some(text);
                    }
                }
            }
            Ok(res) => {
                log::debug!("screener.in {path} status {}", res.status());
            }
            Err(e) => {
                log::warn!("screener.in fetch failed for {base_symbol} ({path}): {e}");
            }
        }
    }
    None
}

/// Fetch quarterly + annual financial tables from Screener.in.
pub async fn fetch_screener_financials(base_symbol: &str) -> ScreenerFinancials {
    let base = base_symbol.trim().to_uppercase();
    if base.is_empty() {
        return ScreenerFinancials::default();
    }
    let Some(html) = fetch_company_html(&base).await else {
        return ScreenerFinancials::default();
    };
    let src = format!("Screener.in /company/{base}/consolidated");
    ScreenerFinancials {
        quarterly: parse_section(&html, "quarters", &src),
        profit_loss: parse_section(&html, "profit-loss", &src),
        balance_sheet: parse_section(&html, "balance-sheet", &src),
        cash_flow: parse_section(&html, "cash-flow", &src),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PFC_SNIPPET: &str = r#"
<section id="quarters" class="card">
<table class="data-table responsive-text-nowrap">
<thead><tr>
<th class="text"></th>
<th data-date-key="2023-03-31">Mar 2023</th>
<th data-date-key="2023-06-30">Jun 2023</th>
<th data-date-key="2023-09-30">Sep 2023</th>
</tr></thead>
<tbody>
<tr><td class="text">Revenue&nbsp;+</td><td>20,061</td><td>21,009</td><td>22,391</td></tr>
<tr><td class="text">Net Profit&nbsp;+</td><td>6,129</td><td>5,982</td><td>6,628</td></tr>
</tbody></table></section>
"#;

    #[test]
    fn parses_quarters_table() {
        let t = parse_section(PFC_SNIPPET, "quarters", "test");
        assert_eq!(t.period_dates.len(), 3);
        assert_eq!(t.period_dates[0], "2023-03-31");
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].label, "Revenue");
        assert_eq!(t.rows[0].values[0], "20,061");
    }

    #[test]
    fn bank_metrics_from_quarterly_npa_rows() {
        let mut q = FinancialTable::default();
        q.period_dates = vec!["2024-03-31".to_string(), "2024-06-30".to_string()];
        q.rows = vec![
            FinancialTableRow {
                label: "Gross NPA %".to_string(),
                values: vec!["2.1%".to_string(), "1.9%".to_string()],
                is_pct: true,
            },
            FinancialTableRow {
                label: "Net NPA %".to_string(),
                values: vec!["0.8%".to_string(), "0.6%".to_string()],
                is_pct: true,
            },
        ];
        let m = bank_metrics_from_quarterly(&q).unwrap();
        assert_eq!(m.gnpa_pct, Some(1.9));
        assert_eq!(m.nnpa_pct, Some(0.6));
    }
}
