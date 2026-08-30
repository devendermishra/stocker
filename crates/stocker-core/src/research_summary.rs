use crate::models::{
    AssetProfile, CompanyOverview, FinancialCompanyType, Financials, FinancialStrengthAudit,
    FundamentalAnalysis, MarketSignals, ResearchRating, ResearchSummary, RiskBuckets,
    StatementBundle, TechnicalAnalysis, TechnicalEntrySignal, ValuationAnalysis,
};
use crate::financial_company::{company_type_label, NA_FILING, NA_YAHOO, NA_YAHOO_FILINGS_MAY_EXIST};
use crate::financial_strength_audit::build_action_guidance;

fn technical_position_from_state(technical: &TechnicalAnalysis) -> String {
    let st = &technical.state;
    let mut parts = Vec::new();
    match (st.above_dma50, st.above_dma200) {
        (Some(true), Some(true)) => parts.push("Price is above both 50 and 200 DMA".to_string()),
        (Some(true), Some(false)) => {
            parts.push("Price is above 50 DMA and below 200 DMA".to_string())
        }
        (Some(false), Some(true)) => {
            parts.push("Price is below 50 DMA and above 200 DMA".to_string())
        }
        (Some(false), Some(false)) => parts.push("Price is below both 50 and 200 DMA".to_string()),
        (Some(true), None) => parts.push("Price is above 50 DMA".to_string()),
        (Some(false), None) => parts.push("Price is below 50 DMA".to_string()),
        (None, Some(true)) => parts.push("Price is above 200 DMA".to_string()),
        (None, Some(false)) => parts.push("Price is below 200 DMA".to_string()),
        _ => {}
    }
    if st.rsi_oversold == Some(true) {
        parts.push("RSI oversold (<30)".to_string());
    } else if st.rsi_below_35 == Some(true) {
        parts.push("RSI below 35".to_string());
    } else if st.rsi_weak == Some(true) {
        parts.push("RSI weak (<40)".to_string());
    } else if technical.momentum.rsi_14.is_some() {
        parts.push(format!("RSI {}", technical.momentum.rsi_label));
    }
    if st.macd_bullish == Some(true) {
        parts.push("MACD bullish".to_string());
    } else if st.macd_bullish == Some(false) {
        parts.push("MACD bearish".to_string());
    }
    if st.price_stretched_vs_50 == Some(true) {
        parts.push("price stretched vs 50 DMA".to_string());
    }
    if parts.is_empty() {
        "Technical state incomplete (missing SMA/RSI).".to_string()
    } else {
        format!("{}.", parts.join("; "))
    }
}

fn summarize_risks(risks: &RiskBuckets) -> String {
    let mut parts = Vec::new();
    for r in risks.financial_risks.iter().take(2) {
        parts.push(format!("{} ({})", r.risk, r.severity));
    }
    for r in risks.valuation_risks.iter().take(1) {
        parts.push(format!("{} ({})", r.risk, r.severity));
    }
    if parts.is_empty() {
        "No automated high-severity flags beyond generic sector risks.".to_string()
    } else {
        parts.join("; ")
    }
}

pub fn build_company_overview(
    symbol: &str,
    profile: &AssetProfile,
    financials: &Financials,
    price: f64,
    bundle: &StatementBundle,
    company_type: FinancialCompanyType,
) -> CompanyOverview {
    let summary = profile
        .long_business_summary
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(480)
        .collect::<String>();
    let summary = if summary.len() >= 480 {
        format!("{}…", summary)
    } else {
        summary
    };

    let yahoo_q = crate::statements::latest_yahoo_quarter_end(bundle);
    let freshness = crate::statements::yahoo_quarter_freshness(
        yahoo_q.as_deref(),
        chrono::Utc::now().date_naive(),
    );

    CompanyOverview {
        company_name: profile
            .long_name
            .clone()
            .unwrap_or_else(|| symbol.to_string()),
        ticker: crate::symbol::india_display_ticker(symbol),
        exchange: profile.exchange.clone(),
        sector: profile.sector.clone(),
        industry: profile.industry.clone(),
        market_cap: financials.market_cap,
        current_price: price,
        business_summary_short: summary,
        website: profile.website.clone(),
        currency: profile.currency.clone(),
        country: profile.country.clone(),
        latest_fiscal_year_end: crate::statements::latest_fiscal_year_end(bundle),
        latest_yahoo_quarter_end: yahoo_q,
        latest_reported_quarter_end: None,
        quarterly_statement_stale: freshness.stale,
        quarterly_statement_age_days: freshness.age_days,
        financial_company_type: company_type,
        financial_company_type_label: company_type_label(company_type).to_string(),
    }
}

pub fn build_research_summary(
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
    technical: &TechnicalAnalysis,
    entry: &TechnicalEntrySignal,
    rating: &ResearchRating,
    risks: &RiskBuckets,
    audit: &FinancialStrengthAudit,
    market: &MarketSignals,
) -> ResearchSummary {
    build_research_summary_for(
        fundamental,
        valuation,
        technical,
        entry,
        rating,
        risks,
        audit,
        market,
        FinancialCompanyType::Industrial,
        &Financials::default(),
        &crate::models::CanonicalMetrics::default(),
        &CompanyOverview::default(),
    )
}

pub fn build_research_summary_for(
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
    technical: &TechnicalAnalysis,
    entry: &TechnicalEntrySignal,
    rating: &ResearchRating,
    risks: &RiskBuckets,
    audit: &FinancialStrengthAudit,
    market: &MarketSignals,
    company_type: FinancialCompanyType,
    financials: &Financials,
    canonical: &crate::models::CanonicalMetrics,
    overview: &CompanyOverview,
) -> ResearchSummary {
    let business_quality = fundamental.profitability.interpretation.clone();
    let growth = fundamental.growth.interpretation.clone();
    let valuation_s = format!(
        "{} Peer read: {}",
        valuation.historical_classification, valuation.peer_value_read
    );
    let technical_position = technical_position_from_state(technical);
    let key_risks = summarize_risks(risks);

    let action_guidance = build_action_guidance(audit, rating, valuation, market, company_type);

    let mut positives = Vec::new();
    for s in audit.strengths.iter().take(2) {
        positives.push(s.clone());
    }
    if fundamental.profitability.interpretation.contains("High-quality") {
        positives.push("Solid profitability vs heuristic thresholds.".to_string());
    }
    if fundamental.growth.interpretation.contains("Strong") {
        positives.push("Growth metrics trending positively.".to_string());
    }
    if valuation.valuation_label == "Cheap" || valuation.valuation_label == "Very Cheap" {
        positives.push("Valuation screen vs history/peers is undemanding.".to_string());
    }
    if positives.len() < 3 {
        positives.push("Review competitive position and capital allocation in filings.".to_string());
    }
    positives.truncate(3);

    let mut negatives = Vec::new();
    for f in audit.red_flags.iter().take(2) {
        negatives.push(f.clone());
    }
    if fundamental.balance_sheet.interpretation.contains("Risky") {
        negatives.push("Balance sheet / liquidity screen is tight.".to_string());
    }
    if fundamental.cash_flow.interpretation.contains("Weak") {
        negatives.push("Cash conversion weaker than earnings.".to_string());
    }
    if valuation.valuation_label.contains("Expensive") {
        negatives.push("Multiples are rich vs history or peers.".to_string());
    }
    if negatives.len() < 3 {
        negatives.push("Data gaps may hide one-off items — verify in annual report.".to_string());
    }
    negatives.truncate(3);

    let suggested = match action_guidance.if_considering_entry.as_str() {
        "Incomplete — verify filings" => "Incomplete — verify filings",
        "Avoid" => "Avoid",
        "Wait" => "Wait for fundamental verification / technical confirmation",
        "Watch / stagger only" => "Watch / stagger only",
        "Buy" => "Buy",
        _ => "Watch",
    }
    .to_string();

    let monitors = if company_type.is_bank() {
        vec![
            "Credit growth vs deposit growth".to_string(),
            "CASA / cost of deposits".to_string(),
            "NIM".to_string(),
            "GNPA / NNPA / credit cost".to_string(),
            "CET1 / CRAR".to_string(),
            "LDR".to_string(),
            "ROA / ROE".to_string(),
        ]
    } else if company_type.is_lender() {
        vec![
            "GNPA / Stage 3, NNPA, PCR from filings".to_string(),
            "CRAR / Tier-I and gearing vs asset quality".to_string(),
            "NIM, spread, cost of funds".to_string(),
            "Loan book and disbursement growth".to_string(),
        ]
    } else {
        let mut m = action_guidance.wait_for_events.clone();
        if m.len() < 3 {
            m.push("Revenue and margin trajectory vs guidance.".to_string());
            m.push("Debt, refinancing, and interest coverage.".to_string());
            m.push("Free cash flow vs capex and working capital.".to_string());
        }
        m.truncate(5);
        m
    };

    if company_type.is_lender() && audit.data_coverage.recommendation_gated {
        negatives.retain(|n| !n.contains("Cash conversion") && !n.contains("Balance sheet / liquidity"));
        if negatives.iter().all(|n| !n.contains("asset-quality")) {
            negatives.insert(
                0,
                "Yahoo lacks GNPA/NNPA/PCR, CRAR and NIM — do not treat as a value trap from debt/CFO.".to_string(),
            );
        }
        negatives.truncate(3);
    }

    let company_type_headline = format!(
        "{} — {}",
        overview.company_name,
        company_type_label(company_type)
    );
    let fmt_x = |v: f64, suf: &str| {
        if v > 0.0 {
            format!("{:.1}{suf}", v)
        } else {
            "N/A".to_string()
        }
    };
    let executive_blocks = if company_type.is_lender() {
        let gnpa = audit
            .checklist
            .iter()
            .find(|i| i.metric.contains("GNPA"))
            .map(|i| i.value_display.clone())
            .unwrap_or_else(|| NA_YAHOO.to_string());
        let crar = audit
            .checklist
            .iter()
            .find(|i| i.metric == "CRAR")
            .map(|i| i.value_display.clone())
            .unwrap_or_else(|| NA_FILING.to_string());
        let nim = audit
            .checklist
            .iter()
            .find(|i| i.metric == "NIM")
            .map(|i| i.value_display.clone())
            .unwrap_or_else(|| NA_FILING.to_string());
        vec![
            (
                "Valuation".to_string(),
                format!(
                    "P/E: {} · Forward P/E: {} · P/B: {} · Dividend yield: {:.1}% · Signal: {}",
                    fmt_x(valuation.pe, "x"),
                    valuation.forward_pe.map(|x| format!("{:.1}x", x)).unwrap_or_else(|| "N/A".into()),
                    if valuation.price_to_book > 0.0 { format!("{:.2}x", valuation.price_to_book) } else { "N/A".into() },
                    valuation.dividend_yield * 100.0,
                    valuation.valuation_label
                ),
            ),
            (
                "Earnings".to_string(),
                format!(
                    "FY PAT growth: {} · 3Y PAT CAGR: {} · Yahoo earnings growth: {}",
                    canonical.fy_pat_yoy_pct.map(|x| format!("{:+.1}%", x)).unwrap_or_else(|| "N/A".into()),
                    canonical.pat_cagr_3y_pct.map(|x| format!("{:+.1}%", x)).unwrap_or_else(|| "N/A".into()),
                    financials.earnings_growth.map(|x| format!("{:+.1}%", x * 100.0)).unwrap_or_else(|| "N/A".into())
                ),
            ),
            (
                "Asset quality".to_string(),
                format!("GNPA/Stage 3: {gnpa} · Status: Filing verification required"),
            ),
            (
                "Capital".to_string(),
                format!("CRAR: {crar} · Status: Filing verification required"),
            ),
            (
                "Lending economics".to_string(),
                format!("NIM: {nim} · Spread/cost of funds: {NA_FILING}"),
            ),
            (
                "Loan growth".to_string(),
                format!(
                    "Yahoo loans YoY: {} (row: {}) · Canonical advances: {}",
                    canonical
                        .yahoo_loan_book_growth_yoy_pct
                        .map(|x| format!("{:.1}%", x))
                        .unwrap_or_else(|| NA_YAHOO_FILINGS_MAY_EXIST.to_string()),
                    if canonical.yahoo_loan_book_row.is_empty() {
                        "unspecified".to_string()
                    } else {
                        canonical.yahoo_loan_book_row.clone()
                    },
                    canonical
                        .canonical_advances
                        .map(|x| format!("{:.0}", x))
                        .unwrap_or_else(|| "N/A — not verified".to_string())
                ),
            ),
            ("Technical".to_string(), technical_position.clone()),
            (
                "Data coverage".to_string(),
                format!(
                    "Overall {:.0}% · Critical {:.0}% ({}/{}) · Confidence: {}",
                    audit.data_coverage.overall_pct,
                    audit.data_coverage.critical_pct,
                    audit.data_coverage.critical_present,
                    audit.data_coverage.critical_total,
                    audit.data_coverage.confidence
                ),
            ),
        ]
    } else {
        vec![]
    };

    let final_view = if company_type.is_lender() && audit.data_coverage.recommendation_gated {
        let kind = if company_type.is_bank() {
            "critical bank metrics"
        } else {
            "critical lender metrics"
        };
        format!(
            "Yahoo valuation must be read with incomplete filing coverage: Yahoo lacks the {kind} needed to judge asset quality and solvency. Do not classify as a value trap solely from debt/CFO metrics. Coverage {:.0}%. {}",
            audit.data_coverage.overall_pct,
            action_guidance.headline
        )
    } else {
        format!(
            "{} Overall score {} — {}. Financial strength {} (confidence: {}). {}",
            action_guidance.headline,
            rating
                .overall_score
                .map(|s| format!("{:.0}/100", s))
                .unwrap_or_else(|| "N/A".to_string()),
            rating.rating_label,
            if audit.scores_provisional {
                "N/A".to_string()
            } else {
                audit
                    .overall_strength_score
                    .map(|s| format!("{:.0}/100", s))
                    .unwrap_or_else(|| "N/A".to_string())
            },
            audit.confidence,
            entry.fundamental_vs_technical
        )
    };

    ResearchSummary {
        business_quality,
        growth,
        valuation: valuation_s,
        technical_position,
        key_risks,
        final_view,
        key_positives: positives,
        key_negatives: negatives,
        key_monitorables: monitors,
        suggested_action: suggested,
        action_guidance,
        disclaimer: "Outputs are heuristic research support from Yahoo-derived data. Contingent liabilities, related-party loans, and auditor footnotes require manual annual report review. Not investment advice.".to_string(),
        company_type_headline,
        executive_blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TechnicalMomentum, TechnicalState, TechnicalTrend};

    #[test]
    fn summary_does_not_claim_rsi_below_35_when_flag_false() {
        let technical = TechnicalAnalysis {
            trend: TechnicalTrend {
                trend_label: "Recovery (above 50 DMA, below 200 DMA)".into(),
                ..Default::default()
            },
            momentum: TechnicalMomentum {
                rsi_14: Some(49.04),
                rsi_label: "Neutral".into(),
                ..Default::default()
            },
            state: TechnicalState {
                above_dma50: Some(true),
                above_dma200: Some(false),
                rsi_oversold: Some(false),
                rsi_weak: Some(false),
                rsi_below_35: Some(false),
                macd_bullish: Some(true),
                price_stretched_vs_50: Some(false),
            },
            ..Default::default()
        };
        let s = technical_position_from_state(&technical);
        assert!(!s.contains("RSI below 35"));
        assert!(!s.to_lowercase().contains("oversold"));
        assert!(s.contains("above 50 DMA"));
        assert!(s.contains("below 200 DMA"));
    }

    #[test]
    fn company_overview_quarter_end_follows_statement_dates() {
        use crate::models::IncomeStatementRow;
        let bundle = StatementBundle {
            income_annual: vec![IncomeStatementRow {
                end_date_fmt: "2026-03-31".into(),
                end_ts: Some(10),
                ..Default::default()
            }],
            income_quarterly: vec![
                IncomeStatementRow {
                    end_date_fmt: "2025-06-30".into(),
                    end_ts: Some(99),
                    ..Default::default()
                },
                IncomeStatementRow {
                    end_date_fmt: "2026-06-30".into(),
                    end_ts: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let o = build_company_overview(
            "HDFCBANK.NS",
            &AssetProfile::default(),
            &Financials::default(),
            720.0,
            &bundle,
            FinancialCompanyType::Bank,
        );
        assert_eq!(o.latest_fiscal_year_end.as_deref(), Some("2026-03-31"));
        assert_eq!(o.latest_yahoo_quarter_end.as_deref(), Some("2026-06-30"));
    }

    #[test]
    fn bank_key_monitorables_are_not_industrial() {
        let s = build_research_summary_for(
            &FundamentalAnalysis::default(),
            &ValuationAnalysis {
                valuation_label: "Fairly Valued".into(),
                historical_classification: "Fair vs history".into(),
                ..Default::default()
            },
            &TechnicalAnalysis::default(),
            &TechnicalEntrySignal::default(),
            &ResearchRating::default(),
            &RiskBuckets {
                business_risks: vec![],
                financial_risks: vec![],
                management_risks: vec![],
                valuation_risks: vec![],
                regulatory_risks: vec![],
            },
            &FinancialStrengthAudit::default(),
            &MarketSignals::default(),
            FinancialCompanyType::Bank,
            &Financials::default(),
            &crate::models::CanonicalMetrics::default(),
            &CompanyOverview {
                company_name: "HDFC Bank".into(),
                ..Default::default()
            },
        );
        assert!(s.key_monitorables.iter().any(|m| m.contains("CASA")));
        assert!(s.key_monitorables.iter().any(|m| m.contains("GNPA")));
        assert!(s.key_monitorables.iter().any(|m| m.contains("LDR")));
        assert!(!s.key_monitorables.iter().any(|m| {
            m.contains("Revenue and margin") || m.contains("Free cash flow vs capex")
        }));
        assert_ne!(s.suggested_action, "Wait for correction");
    }
}
