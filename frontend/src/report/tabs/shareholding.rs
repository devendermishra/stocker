use dioxus::prelude::*;

use crate::format::{fmt_money, fmt_opt_pct};
use crate::report::CARD;
use crate::types::ResearchReport;

fn pct_cell(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.2}%", x * 100.0),
        _ => "—".to_string(),
    }
}

pub fn shareholding_tab(r: &ResearchReport) -> Element {
    let card = CARD;
    let sh = &r.shareholders;
    rsx! {
        section { style: "{card}",
            h3 { style: "margin-top:0;", "Current shareholding (Yahoo snapshot)" }
            table { style: "width:100%; border-collapse: collapse; font-size: 0.9rem;",
                tbody {
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "Yahoo insiders % held" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5; font-weight:600;",
                            {fmt_opt_pct(Some(sh.insiders_percent * 100.0))}
                        }
                    }
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "Yahoo institutions % held" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5; font-weight:600;",
                            {fmt_opt_pct(Some(sh.institutions_percent * 100.0))}
                        }
                    }
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "FII / promoter / MF (Indian filings)" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;",
                            "Not mapped from Yahoo — use NSE/BSE shareholding pattern"
                        }
                    }
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "Mutual funds (Yahoo — not Indian MF category)" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;",
                            "—"
                        }
                    }
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "Retail" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;",
                            "Not inferred from Yahoo insiders %"
                        }
                    }
                    tr {
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5;", "Promoter pledge" }
                        td { style: "padding:0.35rem 0; border-top:1px solid #eceff5; color:#b42318;",
                            {fmt_opt_pct(sh.pledge_percent.map(|v| v * 100.0))}
                        }
                    }
                }
            }
            if let Some(note) = &sh.insider_activity_note {
                p { style: "margin: 0.5rem 0 0; font-size: 0.86rem; color:#273447;", "{note}" }
            }
        }

        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Shareholding pattern — last 8 quarters" }
            p { style: "margin: 0 0 0.5rem; font-size: 0.84rem; color:#666; line-height: 1.45;",
                "Quarterly history is aggregated from Yahoo institutional and mutual-fund filing dates. \
                 Promoter/FII/DII breakdown from Indian exchange filings is not available via Yahoo — \
                 verify promoter and pledge data in BSE/NSE shareholding pattern."
            }
            if r.shareholding_quarterly.is_empty() {
                p { style: "color:#666; font-size: 0.88rem;", "No quarterly shareholding history available for this symbol." }
            } else {
                table { style: "width:100%; border-collapse: collapse; font-size: 0.85rem;",
                    thead {
                        tr {
                            th { style: "text-align:left; padding:0.4rem 0.25rem; border-bottom:1px solid #dce3ef;", "Period end" }
                            th { style: "text-align:right; padding:0.4rem 0.25rem; border-bottom:1px solid #dce3ef;", "Promoter %" }
                            th { style: "text-align:right; padding:0.4rem 0.25rem; border-bottom:1px solid #dce3ef;", "Institutional %" }
                            th { style: "text-align:right; padding:0.4rem 0.25rem; border-bottom:1px solid #dce3ef;", "MF %" }
                            th { style: "text-align:right; padding:0.4rem 0.25rem; border-bottom:1px solid #dce3ef;", "Public %" }
                        }
                    }
                    tbody {
                        for row in &r.shareholding_quarterly {
                            tr {
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6;", "{row.period_end}" }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; text-align:right;",
                                    {pct_cell(row.promoter_pct)}
                                }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; text-align:right;",
                                    {pct_cell(row.institutional_pct)}
                                }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; text-align:right;",
                                    {pct_cell(row.mutual_fund_pct)}
                                }
                                td { style: "padding:0.35rem 0.25rem; border-top:1px solid #f0f2f6; text-align:right;",
                                    {pct_cell(row.public_pct)}
                                }
                            }
                        }
                    }
                }
                if let Some(src) = r.shareholding_quarterly.first().map(|r| r.source.as_str()) {
                    p { style: "margin: 0.45rem 0 0; font-size: 0.78rem; color:#888;", "Source: {src}" }
                }
            }
        }

        section { style: "{card}; margin-top: 0.65rem;",
            h3 { style: "margin-top:0;", "Interpretation guide" }
            ul { style: "margin:0; padding-left: 1.1rem; font-size: 0.88rem; line-height: 1.5;",
                li { "Promoter increasing stake: often positive signal" }
                li { "Mutual funds entering: can improve liquidity and re-rating" }
                li { "FIIs exiting heavily: investigate macro/sector reasons" }
                li { "Promoter pledge increasing: serious red flag" }
            }
        }

        if !r.market_signals.institutional_holders.is_empty() {
            section { style: "{card}; margin-top: 0.65rem;",
                h3 { style: "margin-top:0;", "Top institutional holders" }
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
                h3 { style: "margin-top:0;", "Insider transactions" }
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
