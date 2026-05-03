use crate::models::{
    AssetProfile, CompanyOverview, Financials, FundamentalAnalysis, ResearchRating, ResearchSummary,
    RiskBuckets, StatementBundle, TechnicalAnalysis, TechnicalEntrySignal, ValuationAnalysis,
};

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
) -> CompanyOverview {
    let mut inc = bundle.income_annual.clone();
    inc.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    let mut iq = bundle.income_quarterly.clone();
    iq.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));

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

    CompanyOverview {
        company_name: profile
            .long_name
            .clone()
            .unwrap_or_else(|| symbol.to_string()),
        ticker: symbol.trim_end_matches(".NS").to_string(),
        exchange: profile.exchange.clone(),
        sector: profile.sector.clone(),
        industry: profile.industry.clone(),
        market_cap: financials.market_cap,
        current_price: price,
        business_summary_short: summary,
        website: profile.website.clone(),
        currency: profile.currency.clone(),
        country: profile.country.clone(),
        latest_fiscal_year_end: inc.first().map(|r| r.end_date_fmt.clone()),
        latest_quarter_end: iq.first().map(|r| r.end_date_fmt.clone()),
    }
}

pub fn build_research_summary(
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
    technical: &TechnicalAnalysis,
    entry: &TechnicalEntrySignal,
    rating: &ResearchRating,
    risks: &RiskBuckets,
) -> ResearchSummary {
    let business_quality = fundamental.profitability.interpretation.clone();
    let growth = fundamental.growth.interpretation.clone();
    let valuation_s = format!(
        "{} Peer read: {}",
        valuation.historical_classification, valuation.peer_value_read
    );
    let technical_position = format!(
        "{}. {}",
        technical.trend.trend_label, technical.momentum.rsi_label
    );
    let key_risks = summarize_risks(risks);

    let final_view = format!(
        "Overall score {:.0}/100 — {}. Fundamental valuation: {}. Technical entry: {}. {} This is research support only, not a recommendation.",
        rating.overall_score,
        rating.rating_label,
        rating.cheap_fair_expensive_fundamental,
        rating.technical_entry_label,
        entry.fundamental_vs_technical
    );

    let mut positives = Vec::new();
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

    let monitors = vec![
        "Revenue and margin trajectory vs guidance.".to_string(),
        "Debt, refinancing, and interest coverage.".to_string(),
        "Free cash flow vs capex and working capital.".to_string(),
    ];

    let suggested = if rating.rating_label.contains("Avoid") || valuation.valuation_label.contains("Value Trap")
    {
        "Avoid"
    } else if rating.rating_label.contains("Expensive") {
        "Wait for correction"
    } else if rating.rating_label.contains("Watchlist") {
        "Watch"
    } else if rating.rating_label.contains("Buy") {
        "Watch"
    } else if rating.rating_label.contains("Hold") {
        "Hold"
    } else {
        "Watch"
    }
    .to_string();

    if fundamental.growth.interpretation.contains("Weak") && suggested == "Watch" {
        // nudge
    }

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
        disclaimer: "Outputs are heuristic research support from Yahoo-derived data. Not investment advice. Confirm with filings and your own judgment.".to_string(),
    }
}
