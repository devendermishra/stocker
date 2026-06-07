use crate::models::{
    Financials, FinancialStrengthAudit, FundamentalAnalysis, MarketSignals, PeerQuote, ResearchRating,
    RiskBuckets, ScoreExplanation, Shareholders, TechnicalAnalysis, TechnicalEntrySignal,
    ValuationAnalysis,
};

fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 100.0)
}

pub fn build_research_rating(
    financials: &Financials,
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
    technical: &TechnicalAnalysis,
    technical_entry: &TechnicalEntrySignal,
    peers: &[PeerQuote],
    shareholders: &Shareholders,
    risk_buckets: &RiskBuckets,
    audit: &FinancialStrengthAudit,
    market: &MarketSignals,
) -> ResearchRating {
    let w = (0.20_f64, 0.25_f64, 0.25_f64, 0.15_f64, 0.15_f64);
    let mut explain = Vec::new();

    // Growth 0-100
    let mut growth = 50.0_f64;
    let rg = financials.revenue_growth * 100.0;
    let eg = financials.earnings_growth * 100.0;
    if rg > 15.0 {
        growth += 15.0;
        explain.push(ScoreExplanation {
            factor: "Revenue growth".to_string(),
            impact: "Strong YoY revenue growth (Yahoo/statement mix)".to_string(),
            points: 15.0,
        });
    } else if rg > 5.0 {
        growth += 8.0;
    } else if rg < 0.0 {
        growth -= 12.0;
    }
    if eg > 15.0 {
        growth += 15.0;
    } else if eg > 5.0 {
        growth += 8.0;
    } else if eg < 0.0 {
        growth -= 12.0;
    }
    if fundamental.growth.interpretation.contains("Strong") {
        growth += 10.0;
    } else if fundamental.growth.interpretation.contains("Weak") {
        growth -= 10.0;
    }
    growth = clamp01(growth);

    // Quality 0-100 — blend heuristic fundamentals with financial strength audit.
    // For banks, audit already uses GNPA/NNPA/PCR; CFO/PAT-style metrics are not used.
    let mut quality = audit.overall_strength_score * 0.40 + 27.0;
    let roe = financials.return_on_equity * 100.0;
    if roe > 18.0 {
        quality += 18.0;
    } else if roe > 12.0 {
        quality += 10.0;
    } else if roe < 5.0 {
        quality -= 12.0;
    }
    if financials.profit_margins > 0.12 {
        quality += 10.0;
    } else if financials.profit_margins < 0.03 {
        quality -= 10.0;
    }
    if fundamental.cash_flow.interpretation.contains("Good") {
        quality += 12.0;
    } else if fundamental.cash_flow.interpretation.contains("Weak") {
        quality -= 12.0;
    }
    if fundamental.balance_sheet.interpretation.contains("Net cash") {
        quality += 10.0;
    } else     if fundamental.balance_sheet.interpretation.contains("Risky") {
        quality -= 15.0;
    }
    if audit.earnings_quality_score < 40.0 {
        quality -= 15.0;
    } else if audit.checklist.iter().any(|i| {
        i.metric.contains("Cumulative CFO / PAT (3Y)") && i.value.map(|v| v >= 1.0).unwrap_or(false)
    }) {
        quality += 12.0;
    }
    if audit.balance_sheet_score < 40.0 {
        quality -= 12.0;
    }
    if audit
        .checklist
        .iter()
        .any(|i| i.metric == "Interest coverage" && i.value.map(|v| v < 2.0).unwrap_or(false))
    {
        quality -= 10.0;
    }
    if financials
        .return_on_capital_employed
        .map(|r| r * 100.0 > 18.0)
        .unwrap_or(false)
    {
        quality += 8.0;
    }
    if market.analyst.net_bullish_score > 3 {
        quality += 4.0;
    } else if market.analyst.net_bullish_score < -3 {
        quality -= 4.0;
    }
    let insider_net: f64 = market.insider_transactions.iter().map(|t| t.shares).sum();
    if insider_net > 0.0 {
        quality += 3.0;
    } else if insider_net < 0.0 {
        quality -= 3.0;
    }
    quality = clamp01(quality);

    // Valuation: higher score when cheaper (heuristic)
    let mut val = 50.0_f64;
    match valuation.valuation_label.as_str() {
        "Very Cheap" => val += 28.0,
        "Cheap" => val += 18.0,
        "Fairly Valued" => val += 5.0,
        "Expensive" => val -= 15.0,
        "Very Expensive" => val -= 25.0,
        "Avoid / Possible Value Trap" => val -= 35.0,
        _ => {}
    }
    if valuation
        .historical_classification
        .contains("Cheap")
    {
        val += 8.0;
    }
    if valuation
        .historical_classification
        .contains("expensive")
    {
        val -= 8.0;
    }
    val = clamp01(val);

    // Technical: balanced trend, not extreme
    let mut tech = 50.0_f64;
    if technical.trend.trend_label.contains("Strong uptrend") {
        tech += 12.0;
    } else if technical.trend.trend_label.contains("Weak") {
        tech -= 10.0;
    }
    let rsi = technical.momentum.rsi_14.unwrap_or(50.0);
    if rsi > 75.0 {
        tech -= 12.0;
    } else if rsi < 30.0 {
        tech -= 5.0;
    } else if rsi >= 45.0 && rsi <= 65.0 {
        tech += 8.0;
    }
    if technical.volume.volume_breakout {
        tech += 5.0;
    }
    tech = clamp01(tech);

    // Risk: start 100, subtract penalties (then invert for "risk score" display as 0-100 higher=worse)
    let mut risk_penalties = 0.0_f64;
    if financials.debt_to_equity > 1.20 {
        risk_penalties += 18.0;
    } else if financials.debt_to_equity > 0.80 {
        risk_penalties += 10.0;
    }
    if shareholders.pledge_percent.unwrap_or(0.0) > 0.0 {
        risk_penalties += 15.0;
    }
    if financials.free_cashflow < 0.0 && financials.net_income > 0.0 {
        risk_penalties += 12.0;
    }
    let avg_vol = peers
        .iter()
        .map(|p| p.average_volume_10_day)
        .filter(|v| *v > 0.0)
        .fold((0.0_f64, 0_usize), |acc, v| (acc.0 + v, acc.1 + 1));
    let peer_avg = if avg_vol.1 > 0 {
        avg_vol.0 / avg_vol.1 as f64
    } else {
        0.0
    };
    if peer_avg > 0.0 && financials.average_volume_10_day > 0.0 && financials.average_volume_10_day < peer_avg * 0.2 {
        risk_penalties += 10.0;
    }
    if valuation.valuation_label.contains("Expensive") {
        risk_penalties += 8.0;
    }
    let vol = technical.volatility.vol_1y_ann_pct.unwrap_or(0.0);
    if vol > 40.0 {
        risk_penalties += 10.0;
    }
    let high_sev = risk_buckets
        .financial_risks
        .iter()
        .chain(risk_buckets.management_risks.iter())
        .chain(risk_buckets.valuation_risks.iter())
        .filter(|r| r.severity == "High")
        .count() as f64;
    risk_penalties += high_sev * 4.0;
    risk_penalties = risk_penalties.clamp(0.0, 80.0);
    let risk_score = clamp01(risk_penalties * 1.15);

    let overall = growth * w.0 + quality * w.1 + val * w.2 + tech * w.3 + (100.0 - risk_score) * w.4;
    let overall = clamp01(overall);

    let rating_label = if overall >= 78.0 && !valuation.valuation_label.contains("Expensive") {
        "Strong Buy Candidate"
    } else if overall >= 68.0 {
        "Buy Candidate"
    } else if overall >= 58.0 {
        "Watchlist"
    } else if overall >= 48.0 {
        "Hold"
    } else if risk_score > 70.0 {
        "High Risk"
    } else if valuation.valuation_label.contains("Expensive") {
        "Expensive / Wait"
    } else {
        "Avoid"
    }
    .to_string();

    let fund_r = if quality >= 68.0 && growth >= 55.0 {
        "Strong"
    } else if quality >= 55.0 {
        "Adequate"
    } else {
        "Weak"
    }
    .to_string();

    let val_r = match valuation.valuation_label.as_str() {
        "Very Cheap" | "Cheap" => "Attractive",
        "Fairly Valued" => "Neutral",
        "Expensive" | "Very Expensive" => "Rich",
        "Avoid / Possible Value Trap" => "Unattractive",
        _ => "Unclear",
    }
    .to_string();

    let tech_r = if technical.trend.trend_label.contains("Strong uptrend") {
        "Supportive"
    } else if technical.momentum.rsi_14.unwrap_or(50.0) > 70.0 {
        "Overbought"
    } else if technical.momentum.rsi_14.unwrap_or(50.0) < 35.0 {
        "Weak / Oversold"
    } else {
        "Neutral"
    }
    .to_string();

    let risk_r = if risk_score > 65.0 {
        "High"
    } else if risk_score > 40.0 {
        "Medium"
    } else {
        "Moderate"
    }
    .to_string();

    explain.push(ScoreExplanation {
        factor: "Composite".to_string(),
        impact: format!(
            "Weighted {:.0}% growth, {:.0}% quality, {:.0}% valuation, {:.0}% technical, {:.0}% risk-adjusted.",
            w.0 * 100.0,
            w.1 * 100.0,
            w.2 * 100.0,
            w.3 * 100.0,
            w.4 * 100.0
        ),
        points: overall,
    });

    ResearchRating {
        growth_score: growth,
        quality_score: quality,
        valuation_score: val,
        technical_score: tech,
        risk_score,
        overall_score: overall,
        rating_label,
        fundamental_rating: fund_r,
        valuation_rating: val_r,
        technical_rating: tech_r,
        risk_rating: risk_r,
        weights: w,
        explain,
        cheap_fair_expensive_fundamental: valuation.valuation_label.clone(),
        technical_entry_label: technical_entry.detail_label.clone(),
    }
}
