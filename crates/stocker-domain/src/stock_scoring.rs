use crate::models::{
    FinancialCompanyType, Financials, FinancialStrengthAudit, FundamentalAnalysis, MarketSignals, PeerQuote, ResearchRating,
    RiskBuckets, ScoreExplanation, Shareholders, TechnicalAnalysis, TechnicalEntrySignal,
    ValuationAnalysis, CanonicalMetrics,
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
    _market: &MarketSignals,
    canonical: &CanonicalMetrics,
) -> ResearchRating {
    build_research_rating_for(
        financials,
        fundamental,
        valuation,
        technical,
        technical_entry,
        peers,
        shareholders,
        risk_buckets,
        audit,
        _market,
        canonical,
        FinancialCompanyType::Industrial,
    )
}

pub fn build_research_rating_for(
    financials: &Financials,
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
    technical: &TechnicalAnalysis,
    technical_entry: &TechnicalEntrySignal,
    peers: &[PeerQuote],
    shareholders: &Shareholders,
    risk_buckets: &RiskBuckets,
    audit: &FinancialStrengthAudit,
    _market: &MarketSignals,
    canonical: &CanonicalMetrics,
    company_type: FinancialCompanyType,
) -> ResearchRating {
    let w = (0.20_f64, 0.25_f64, 0.25_f64, 0.15_f64, 0.15_f64);
    let mut explain = Vec::new();

    // Growth 0-100
    let mut growth = 50.0_f64;
    if company_type.is_lender() {
        if let Some(g) = canonical.fy_pat_yoy_pct {
            if g > 15.0 {
                growth += 15.0;
            } else if g > 5.0 {
                growth += 8.0;
            } else if g < 0.0 {
                growth -= 12.0;
            }
        }
        if let Some(g) = canonical.interest_income_yoy_pct {
            if g > 10.0 {
                growth += 8.0;
            } else if g < 0.0 {
                growth -= 6.0;
            }
        }
        if let Some(g) = canonical.nii_yoy_pct {
            if g > 8.0 {
                growth += 6.0;
            } else if g < 0.0 {
                growth -= 6.0;
            }
        }
    } else if let Some(rg) = financials.revenue_growth.map(|x| x * 100.0) {
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
    }
    if !company_type.is_lender() {
        if let Some(eg) = financials.earnings_growth.map(|x| x * 100.0) {
            if eg > 15.0 {
                growth += 15.0;
            } else if eg > 5.0 {
                growth += 8.0;
            } else if eg < 0.0 {
                growth -= 12.0;
            }
        }
    }
    if fundamental.growth.interpretation.contains("Strong") {
        growth += 10.0;
    } else if fundamental.growth.interpretation.contains("Weak") {
        growth -= 10.0;
    }
    growth = clamp01(growth);

    // Quality tracks the financial-strength audit, with a small bounded adjustment — not a stacked 98/100.
    let mut q_adj = 0.0_f64;
    if let Some(roe) = financials.return_on_equity.map(|r| r * 100.0) {
        if roe > 18.0 {
            q_adj += 6.0;
        } else if roe < 5.0 {
            q_adj -= 6.0;
        }
    }
    if !company_type.is_lender() {
        if financials.profit_margins > 0.12 {
            q_adj += 4.0;
        } else if financials.profit_margins < 0.03 {
            q_adj -= 4.0;
        }
    }
    if company_type.is_lender() {
        if fundamental.cash_flow.interpretation.contains("Weak") {
            // ignored — loan-book section must not penalize as CFO
        }
    } else if fundamental.cash_flow.interpretation.contains("Good") {
        q_adj += 4.0;
    } else if fundamental.cash_flow.interpretation.contains("Weak") {
        q_adj -= 6.0;
    }
    if !company_type.is_lender() {
        if canonical.is_net_cash_equivalents {
            q_adj += 4.0;
        } else if fundamental.balance_sheet.interpretation.contains("Risky") {
            q_adj -= 8.0;
        }
    }
    if !audit.scores_provisional {
        if audit.earnings_quality_score.map(|s| s < 40.0).unwrap_or(false) {
            q_adj -= 8.0;
        }
        if audit.balance_sheet_score.map(|s| s < 40.0).unwrap_or(false) {
            q_adj -= 6.0;
        }
    }
    if !company_type.is_lender()
        && audit
            .checklist
            .iter()
            .any(|i| i.metric == "Interest coverage" && i.value.map(|v| v < 2.0).unwrap_or(false))
    {
        q_adj -= 6.0;
    }
    let financial_quality = if audit.scores_provisional && company_type.is_lender() {
        None
    } else {
        audit
            .overall_strength_score
            .map(|s| clamp01(s + q_adj.clamp(-12.0, 12.0)))
    };

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
    match (technical.state.rsi_oversold, technical.momentum.rsi_14) {
        (Some(true), _) => tech -= 5.0,
        (_, Some(r)) if r > 75.0 => tech -= 12.0,
        (_, Some(r)) if (45.0..=65.0).contains(&r) => tech += 8.0,
        _ => {}
    }
    if technical.volume.volume_breakout {
        tech += 5.0;
    }
    tech = clamp01(tech);

    // Risk: start 100, subtract penalties (then invert for "risk score" display as 0-100 higher=worse)
    let mut risk_penalties = 0.0_f64;
    if !company_type.is_lender() {
        if let Some(de) = financials.debt_to_equity {
            if de > 1.20 {
                risk_penalties += 18.0;
            } else if de > 0.80 {
                risk_penalties += 10.0;
            }
        }
        if financials.free_cashflow.map(|f| f < 0.0).unwrap_or(false)
            && financials.net_income.map(|n| n > 0.0).unwrap_or(false)
        {
            risk_penalties += 12.0;
        }
    }
    if shareholders.pledge_percent.unwrap_or(0.0) > 0.0 {
        risk_penalties += 15.0;
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
    let gated = audit.data_coverage.recommendation_gated;
    let high_sev = risk_buckets
        .financial_risks
        .iter()
        .chain(risk_buckets.management_risks.iter())
        .chain(risk_buckets.valuation_risks.iter())
        .filter(|r| r.severity == "High")
        .count() as f64;
    if !(gated && company_type.is_lender()) {
        risk_penalties += high_sev * 4.0;
    }
    risk_penalties = risk_penalties.clamp(0.0, 80.0);
    let credit_unassessed =
        crate::financial_company::lender_fundamental_risk_unassessed(&audit.data_coverage, company_type);
    let risk_penalty = if (gated && company_type.is_lender()) || credit_unassessed {
        None
    } else {
        Some(clamp01(risk_penalties * 1.15))
    };

    let overall = if gated && company_type.is_lender() {
        let wsum = w.0 + w.2 + w.3;
        clamp01((growth * w.0 + val * w.2 + tech * w.3) / wsum.max(1e-9))
    } else if credit_unassessed {
        let q = financial_quality.unwrap_or(50.0);
        let wsum = w.0 + w.1 + w.2 + w.3;
        clamp01((growth * w.0 + q * w.1 + val * w.2 + tech * w.3) / wsum.max(1e-9))
    } else {
        let q = financial_quality.unwrap_or(50.0);
        let rp = risk_penalty.unwrap_or(0.0);
        clamp01(growth * w.0 + q * w.1 + val * w.2 + tech * w.3 + (100.0 - rp) * w.4)
    };

    let expensive_label = valuation.valuation_label.contains("Expensive");
    let rating_label = if gated && company_type.is_lender() {
        "Incomplete — verify filings".to_string()
    } else if overall >= 80.0 && !expensive_label {
        "Strong Buy candidate".to_string()
    } else if overall >= 70.0 {
        "Buy / Accumulate".to_string()
    } else if overall >= 60.0 {
        "Watchlist".to_string()
    } else if overall >= 50.0 {
        "Caution".to_string()
    } else if risk_penalty.unwrap_or(0.0) > 70.0 {
        "High Risk".to_string()
    } else if expensive_label {
        "Expensive / Wait".to_string()
    } else {
        "Avoid".to_string()
    };

    let fund_r = if gated && company_type.is_lender() {
        "Insufficient data".to_string()
    } else if financial_quality.unwrap_or(0.0) >= 68.0 && growth >= 55.0 {
        "Strong".to_string()
    } else if financial_quality.unwrap_or(0.0) >= 55.0 {
        "Adequate".to_string()
    } else {
        "Weak".to_string()
    };

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
    } else if technical.state.rsi_oversold == Some(true) {
        "Weak / Oversold"
    } else if technical.state.rsi_below_35 == Some(true)
        || technical.momentum.rsi_14.map(|r| r < 35.0).unwrap_or(false)
    {
        "Weak / Near oversold"
    } else if technical.momentum.rsi_14.map(|r| r > 70.0).unwrap_or(false) {
        "Overbought"
    } else {
        "Neutral"
    }
    .to_string();

    let risk_r = if credit_unassessed {
        "Unassessed / insufficient data".to_string()
    } else if gated && company_type.is_lender() {
        "Unassessed / insufficient data".to_string()
    } else if risk_penalty.unwrap_or(0.0) > 65.0 {
        "High".to_string()
    } else if risk_penalty.unwrap_or(0.0) > 40.0 {
        "Elevated".to_string()
    } else if risk_penalty.unwrap_or(0.0) > 15.0 {
        "Moderate".to_string()
    } else {
        "Low".to_string()
    };
    let market_beta_risk = crate::financial_company::market_beta_risk_label(financials.beta);

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

    let data_quality_score = crate::analysis::evaluate_quality_for(financials, company_type);
    let screening_quality_score = if gated && company_type.is_lender() {
        None
    } else {
        financial_quality
    };
    let overall_score_provisional = gated && company_type.is_lender();

    ResearchRating {
        growth_score: growth,
        financial_quality_score: financial_quality,
        valuation_score: val,
        technical_score: tech,
        risk_penalty,
        overall_score: if overall_score_provisional { None } else { Some(overall) },
        overall_score_provisional,
        provisional_screening_score: if overall_score_provisional { Some(overall) } else { None },
        data_quality_score,
        screening_quality_score,
        rating_label,
        fundamental_rating: fund_r,
        valuation_rating: val_r,
        technical_rating: tech_r,
        risk_rating: risk_r.clone(),
        fundamental_risk_rating: risk_r,
        market_beta_risk,
        weights: w,
        explain,
        cheap_fair_expensive_fundamental: valuation.valuation_label.clone(),
        technical_entry_label: technical_entry.detail_label.clone(),
    }
}
