use dioxus::prelude::*;

use crate::components::KeyValue;
use crate::format::{fmt_money, fmt_opt_pct, fmt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

pub fn financials_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Financial Snapshot" }
            KeyValue { label: "Revenue", value: fmt_money(r.financials.revenue) }
            KeyValue { label: "Net Income", value: fmt_money(r.financials.net_income) }
            KeyValue { label: "EBITDA", value: fmt_money(r.financials.ebitda) }
            KeyValue { label: "Operating Cashflow", value: fmt_money(r.financials.operating_cashflow) }
            KeyValue { label: "Free Cashflow", value: fmt_money(r.financials.free_cashflow) }
            KeyValue { label: "Dividend Yield", value: fmt_pct(r.financials.dividend_yield) }
            KeyValue { label: "Payout Ratio", value: fmt_pct(r.financials.payout_ratio) }
            KeyValue { label: "Revenue Growth (YoY)", value: fmt_pct(r.financials.revenue_growth) }
            KeyValue { label: "Earnings Growth (YoY)", value: fmt_pct(r.financials.earnings_growth) }
            KeyValue { label: "Previous Close", value: format!("₹{:.2}", r.financials.previous_close) }
            KeyValue { label: "Ex-dividend Date", value: r.financials.ex_dividend_date.clone().unwrap_or_else(|| "N/A".to_string()) }
            KeyValue { label: "Insider Holding", value: fmt_pct(r.shareholders.insiders_percent) }
            KeyValue { label: "Institutional Holding", value: fmt_pct(r.shareholders.institutions_percent) }
            KeyValue { label: "Promoter Holding", value: fmt_opt_pct(r.shareholders.promoter_percent.map(|v| v * 100.0)) }
            KeyValue { label: "FII Holding", value: fmt_opt_pct(r.shareholders.fii_percent.map(|v| v * 100.0)) }
            KeyValue { label: "DII Holding", value: fmt_opt_pct(r.shareholders.dii_percent.map(|v| v * 100.0)) }
            KeyValue { label: "Mutual Fund Holding", value: fmt_opt_pct(r.shareholders.mutual_fund_percent.map(|v| v * 100.0)) }
            KeyValue { label: "Retail Holding", value: fmt_opt_pct(r.shareholders.retail_percent.map(|v| v * 100.0)) }
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
