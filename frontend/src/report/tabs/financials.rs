use dioxus::prelude::*;

use crate::components::KeyValue;
use crate::format::{fmt_money, fmt_opt_money, fmt_opt_num, fmt_opt_pct, fmt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn financials_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Financial Snapshot" }
            KeyValue { label: "Yahoo totalRevenue (quote)", value: fmt_money(r.financials.revenue) }
            KeyValue { label: "Net Income (Yahoo TTM)", value: fmt_opt_money(r.financials.net_income) }
            KeyValue { label: "EBITDA", value: if !r.financials.industrial_yahoo_fields_analysis_applicable { "Raw Yahoo (not used for NBFC analysis)".to_string() } else if r.canonical.industrial_metrics_suppressed { "N/A — not meaningful for lending companies".to_string() } else { fmt_money(r.financials.ebitda) } }
            KeyValue { label: "Operating Cashflow (Yahoo)", value: fmt_opt_money(r.financials.operating_cashflow) }
            KeyValue { label: "Free Cashflow (Yahoo snapshot, if any)", value: fmt_opt_money(r.financials.free_cashflow) }
            KeyValue { label: "Dividend Yield", value: fmt_pct(r.financials.dividend_yield) }
            KeyValue { label: "Payout Ratio", value: fmt_pct(r.financials.payout_ratio) }
            if !r.canonical.industrial_metrics_suppressed {
                KeyValue { label: "Yahoo current revenueGrowth (not FY)", value: fmt_opt_pct(r.financials.revenue_growth.map(|g| g * 100.0)) }
            }
            KeyValue { label: "Yahoo current earningsGrowth (not FY PAT)", value: fmt_opt_pct(r.financials.earnings_growth.map(|g| g * 100.0)) }
            KeyValue { label: "Previous Close", value: format!("₹{:.2}", r.financials.previous_close) }
            if !r.financials.industrial_yahoo_fields_analysis_applicable {
                p { style: "font-size:0.8rem; color:#8a4a00; margin:0.35rem 0 0;",
                    "Yahoo EBITDA, gross/operating margins, P/S, and enterprise value are stored as raw fields (analysis_applicable = false) and are not used for this lender."
                }
            }
        }
        if let Some(app) = &r.financials_applicable {
            section { style: "{card}; margin-top: 0.65rem;",
                h3 { style: "margin-top:0;", "Financials applicable (valuation)" }
                KeyValue { label: "P/E", value: if app.pe_ratio > 0.0 { format!("{:.2}", app.pe_ratio) } else { "N/A".to_string() } }
                KeyValue { label: "P/B", value: if app.price_to_book > 0.0 { format!("{:.2}", app.price_to_book) } else { "N/A".to_string() } }
                KeyValue { label: "EPS", value: if app.trailing_eps.abs() > 0.0 { format!("{:.2}", app.trailing_eps) } else { "N/A".to_string() } }
                KeyValue { label: "Book value / share", value: if app.book_value.abs() > 0.0 { format!("{:.2}", app.book_value) } else { "N/A".to_string() } }
                KeyValue { label: "Dividend yield", value: fmt_pct(app.dividend_yield) }
                KeyValue { label: "Market cap", value: fmt_money(app.market_cap) }
                KeyValue { label: "Beta", value: if app.beta > 0.0 { format!("{:.2}", app.beta) } else { "N/A".to_string() } }
            }
        }
        if let Some(raw) = &r.financials_raw_yahoo {
            if !raw.analysis_applicable {
                section { style: "{card}; margin-top: 0.65rem;",
                    h3 { style: "margin-top:0;", "Financials raw Yahoo (not used for analysis)" }
                    p { style: "font-size:0.8rem; color:#5a6578; margin:0 0 0.35rem;", "{raw.note}" }
                    KeyValue { label: "Revenue", value: fmt_money(raw.revenue) }
                    KeyValue { label: "Revenue growth", value: fmt_opt_pct(raw.revenue_growth.map(|g| g * 100.0)) }
                    KeyValue { label: "EBITDA", value: fmt_money(raw.ebitda) }
                    KeyValue { label: "Gross margin", value: fmt_pct(raw.gross_margins) }
                    KeyValue { label: "Operating margin", value: fmt_pct(raw.operating_margins) }
                    KeyValue { label: "P/S", value: if raw.price_to_sales > 0.0 { format!("{:.2}", raw.price_to_sales) } else { "N/A".to_string() } }
                    KeyValue { label: "Enterprise value", value: fmt_opt_money(raw.enterprise_value) }
                }
            }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Canonical statement metrics" }
            p { style: "font-size:0.84rem; color:#5a6578; line-height:1.45;",
                "Scoring and cash-flow quality read this object, not Yahoo quoteSummary zeros."
            }
            KeyValue { label: "PAT (selected)", value: fmt_opt_money(r.canonical.pat) }
            KeyValue { label: "PAT period", value: if r.canonical.pat_period.is_empty() { "N/A".to_string() } else { r.canonical.pat_period.clone() } }
            KeyValue { label: "FY PAT (annual statement)", value: fmt_opt_money(r.canonical.fy_pat) }
            KeyValue { label: "TTM PAT (four quarters)", value: fmt_opt_money(r.canonical.ttm_pat) }
            KeyValue { label: "Latest Yahoo quarter PAT", value: fmt_opt_money(r.canonical.latest_yahoo_quarter_pat) }
            KeyValue { label: "Latest Yahoo quarter PAT period", value: if r.canonical.latest_yahoo_quarter_pat_period.is_empty() { "N/A".to_string() } else { r.canonical.latest_yahoo_quarter_pat_period.clone() } }
            KeyValue { label: "Latest Yahoo quarter PAT source column", value: if r.canonical.latest_yahoo_quarter_pat_source_column.is_empty() { "N/A".to_string() } else { r.canonical.latest_yahoo_quarter_pat_source_column.clone() } }
            KeyValue { label: "Latest reported quarter (filings)", value: r.canonical.latest_reported_quarter_end.clone().unwrap_or_else(|| "N/A — Yahoo-only".to_string()) }
            KeyValue { label: "Yahoo quarterly series stale", value: if r.canonical.quarterly_statement_stale { r.canonical.quarterly_statement_age_days.map(|d| format!("Yes · {d} days")).unwrap_or_else(|| "Yes".to_string()) } else { "No".to_string() } }
            KeyValue { label: "PAT scope", value: if r.canonical.pat_scope.is_empty() { "N/A".to_string() } else { r.canonical.pat_scope.clone() } }
            KeyValue { label: "FCF (CFO − capex)", value: if r.canonical.industrial_metrics_suppressed { "N/A — not used for lenders".to_string() } else { fmt_opt_money(r.canonical.fcf) } }
            KeyValue { label: "CFO", value: if r.canonical.industrial_metrics_suppressed { "N/A — not used for lenders".to_string() } else { fmt_opt_money(r.canonical.cfo) } }
            KeyValue { label: "ROCE", value: if r.canonical.industrial_metrics_suppressed { "N/A — not meaningful for lending companies".to_string() } else { r.canonical.roce.map(|v| format!("{:.1}%", v * 100.0)).unwrap_or_else(|| "N/A".to_string()) } }
            KeyValue { label: "Current ratio", value: if r.canonical.industrial_metrics_suppressed { "N/A — not a primary metric".to_string() } else { fmt_opt_num(r.canonical.current_ratio) } }
            KeyValue { label: "Interest coverage", value: if r.canonical.industrial_metrics_suppressed { "N/A — not meaningful for lending companies".to_string() } else { r.canonical.interest_coverage.map(|v| format!("{:.2}x", v)).unwrap_or_else(|| "N/A".to_string()) } }
            KeyValue { label: "Yahoo totalRevenue (unverified; not total income)", value: fmt_opt_money(r.canonical.yahoo_revenue_field) }
            KeyValue { label: "Interest income", value: fmt_opt_money(r.canonical.interest_income) }
            KeyValue { label: "Net interest income (preferred)", value: fmt_opt_money(r.canonical.net_interest_income) }
            KeyValue { label: "Yahoo-reported NII (statement row)", value: fmt_opt_money(r.canonical.yahoo_reported_nii) }
            KeyValue { label: "Calculated NII (II − IE)", value: fmt_opt_money(r.canonical.calculated_nii) }
            KeyValue { label: "Canonical NII", value: fmt_opt_money(if r.canonical.canonical_nii.is_some() { r.canonical.canonical_nii } else { r.canonical.net_interest_income }) }
            KeyValue { label: "Canonical NII source", value: if r.canonical.canonical_nii_source.is_empty() { r.canonical.nii_definition.clone() } else { r.canonical.canonical_nii_source.clone() } }
            KeyValue { label: "NII reconciliation (calculated − Yahoo)", value: fmt_opt_money(r.canonical.nii_reconciliation_difference) }
            KeyValue { label: "NII definition", value: if r.canonical.nii_definition.is_empty() { "N/A".to_string() } else { r.canonical.nii_definition.clone() } }
            KeyValue { label: "Loan book (NBFC proxy)", value: if r.canonical.loan_book.is_some() { fmt_opt_money(r.canonical.loan_book) } else { "N/A — banks use Yahoo loans/receivables below, not this field".to_string() } }
            KeyValue { label: "Yahoo loans/receivables", value: fmt_opt_money(r.canonical.yahoo_loan_book_field) }
            KeyValue { label: "Yahoo loan row", value: if r.canonical.yahoo_loan_book_row.is_empty() { "N/A".to_string() } else { r.canonical.yahoo_loan_book_row.clone() } }
            KeyValue { label: "Yahoo loans YoY", value: fmt_opt_pct(r.canonical.yahoo_loan_book_growth_yoy_pct) }
            KeyValue { label: "Canonical advances (gross)", value: r.canonical.canonical_advances.map(|_| fmt_opt_money(r.canonical.canonical_advances)).unwrap_or_else(|| "N/A — Yahoo row not verified as gross advances".to_string()) }
            KeyValue { label: "Loan book growth YoY (canonical)", value: if r.canonical.loan_book_growth_yoy_pct.is_some() { fmt_opt_pct(r.canonical.loan_book_growth_yoy_pct) } else if r.canonical.industrial_metrics_suppressed { "N/A for banks unless filing-verified".to_string() } else { fmt_opt_pct(r.canonical.loan_book_growth_yoy_pct) } }
            KeyValue { label: "Cash & cash equivalents", value: fmt_opt_money(r.canonical.cash_and_cash_equivalents) }
            KeyValue { label: "Short-term investments", value: fmt_opt_money(r.canonical.short_term_investments) }
            KeyValue { label: "Gross cash + liquid", value: fmt_opt_money(r.canonical.gross_cash_and_liquid_investments) }
            if r.canonical.industrial_metrics_suppressed {
                p { style: "font-size:0.8rem; color:#8a4a00; margin:0.35rem 0 0;",
                    "Cash is statement cash only — not a liquidity-strength signal. Industrial net-debt is omitted from canonical for lenders."
                }
                if let Some(raw) = &r.canonical.raw_balance_sheet {
                    KeyValue { label: "Raw net debt vs CCE (not scored)", value: fmt_opt_money(raw.net_debt_vs_cash_equivalents) }
                    KeyValue { label: "Raw net debt vs liquid (not scored)", value: fmt_opt_money(raw.net_debt_vs_liquid) }
                    if !raw.note.is_empty() {
                        p { style: "font-size:0.8rem; color:#5a6578; margin:0.2rem 0 0;", "{raw.note}" }
                    }
                }
            } else {
                KeyValue { label: "Net debt vs CCE", value: fmt_opt_money(r.canonical.net_debt_vs_cash_equivalents) }
                KeyValue { label: "Net cash (CCE > debt)", value: if r.canonical.is_net_cash_equivalents { "Yes".to_string() } else { "No".to_string() } }
            }
            KeyValue { label: "Interest income YoY", value: fmt_opt_pct(r.canonical.interest_income_yoy_pct) }
            KeyValue { label: "NII YoY", value: fmt_opt_pct(r.canonical.nii_yoy_pct) }
            KeyValue { label: "FY PAT YoY (annual)", value: fmt_opt_pct(r.canonical.fy_pat_yoy_pct) }
            if !r.canonical.industrial_metrics_suppressed {
                KeyValue { label: "FY revenue YoY (annual)", value: fmt_opt_pct(r.canonical.fy_revenue_yoy_pct) }
                KeyValue { label: "Statement 3Y revenue CAGR", value: fmt_opt_pct(r.canonical.revenue_cagr_3y_pct) }
            }
            KeyValue { label: "Statement 3Y PAT CAGR", value: fmt_opt_pct(r.canonical.pat_cagr_3y_pct) }
            for n in &r.canonical.notes {
                p { style: "font-size:0.8rem; color:#5a6578; margin:0.2rem 0 0;", "{n}" }
            }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Ownership snapshot" }
            KeyValue { label: "Yahoo insiders % held", value: fmt_pct(r.shareholders.insiders_percent) }
            KeyValue { label: "Yahoo institutions % held", value: fmt_pct(r.shareholders.institutions_percent) }
            KeyValue { label: "Promoter (NSE/BSE — not Yahoo)", value: "N/A until sourced from exchange filings".to_string() }
            KeyValue { label: "FII (NSE/BSE — not Yahoo)", value: "N/A until sourced from exchange filings".to_string() }
            KeyValue { label: "DII Holding", value: fmt_opt_pct(r.shareholders.dii_percent.map(|v| v * 100.0)) }
            KeyValue { label: "Mutual Fund (NSE/BSE — not Yahoo)", value: "N/A until sourced from exchange filings".to_string() }
            KeyValue { label: "Retail Holding", value: "N/A (not Yahoo 1 − insiders)".to_string() }
            KeyValue { label: "Promoter Pledge", value: fmt_opt_pct(r.shareholders.pledge_percent.map(|v| v * 100.0)) }
        }
        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Important Ownership Changes (Interpretation Guide)" }
            ul {
                li { "Promoter increasing stake: often positive" }
                li { "Mutual funds entering: can improve liquidity/re-rating" }
                li { "FIIs exiting heavily: investigate reason" }
                li { "Promoter pledge increasing: serious red flag" }
            }
            if let Some(note) = &r.shareholders.insider_activity_note {
                p { style: "color:#273447;", "{note}" }
            }
        }
        if !r.market_signals.institutional_holders.is_empty() {
            section { style: "{card}; margin-top: 0.65rem;",
                h3 { style: "margin-top:0;", "Institutional holders (Yahoo)" }
                table { style: "width:100%; border-collapse: collapse; font-size: 0.88rem;",
                    thead { tr { th { "Organization" } th { "% held" } th { "Value" } th { "Report date" } } }
                    tbody {
                        for h in &r.market_signals.institutional_holders {
                            tr {
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{h.organization}" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{h.pct_held * 100.0:.2}%" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{fmt_money(h.value)}" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{h.report_date}" }
                            }
                        }
                    }
                }
            }
        }
        if !r.market_signals.insider_transactions.is_empty() {
            section { style: "{card}; margin-top: 0.65rem;",
                h3 { style: "margin-top:0;", "Insider transactions (Yahoo)" }
                table { style: "width:100%; border-collapse: collapse; font-size: 0.88rem;",
                    thead { tr { th { "Filer" } th { "Transaction" } th { "Shares" } th { "Date" } } }
                    tbody {
                        for t in &r.market_signals.insider_transactions {
                            tr {
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{t.filer_name}" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{t.transaction_text}" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{t.shares:.0}" }
                                td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "{t.start_date}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
