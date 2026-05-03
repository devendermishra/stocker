use crate::models::{
    FundamentalAnalysis, TechnicalAnalysis, TechnicalEntrySignal, ValuationAnalysis,
};

pub fn build_technical_entry_signal(
    technical: &TechnicalAnalysis,
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
) -> TechnicalEntrySignal {
    let price = technical.trend.price_vs_sma200_pct.unwrap_or(0.0);
    let p50 = technical.trend.price_vs_sma50_pct.unwrap_or(0.0);
    let rsi = technical.momentum.rsi_14.unwrap_or(50.0);
    let dist_hi = technical.volatility.dist_from_high_pct.unwrap_or(50.0);
    let roc1 = technical.momentum.roc_1m_pct.unwrap_or(0.0);
    let vol = technical.volatility.vol_1y_ann_pct.unwrap_or(20.0);

    let weak_fund = fundamental.profitability.interpretation.contains("Poor")
        || fundamental.growth.interpretation.contains("Weak");
    let deteriorating = fundamental.cash_flow.interpretation.contains("Weak");

    let (zone, detail, mut rationale) = if rsi < 35.0 && price < -5.0 {
        (
            "Technically oversold / value zone",
            if weak_fund && deteriorating {
                "Falling Knife Risk"
            } else if weak_fund {
                "Weak but Cheap"
            } else {
                "Possible Value Zone"
            },
            vec![
                "RSI below 35 and price below 200 DMA (heuristic).".to_string(),
                "Check fundamentals before averaging down.".to_string(),
            ],
        )
    } else if rsi > 70.0 && p50 > 8.0 && dist_hi < 8.0 {
        (
            "Technically extended",
            "Momentum Strong but Costly",
            vec![
                "RSI elevated and price stretched vs 50 DMA / 52-week high.".to_string(),
                "Better entry may come on consolidation.".to_string(),
            ],
        )
    } else if rsi >= 45.0 && rsi <= 60.0 && p50.abs() < 6.0 && dist_hi > 8.0 {
        (
            "Technically fair",
            "Neutral Zone",
            vec![
                "Price near moving averages without extreme RSI.".to_string(),
                "May wait for breakout or pullback for clearer risk/reward.".to_string(),
            ],
        )
    } else if vol > 35.0 {
        (
            "Technically fair",
            "High volatility",
            vec!["Annualized volatility is elevated — size positions conservatively.".to_string()],
        )
    } else {
        (
            "Technically fair",
            if roc1 > 8.0 {
                "Wait for Correction"
            } else {
                "Wait for Breakout"
            },
            vec!["No extreme technical signal; trend/momentum mixed.".to_string()],
        )
    };

    if weak_fund && zone.contains("oversold") {
        rationale.push("Fundamental deterioration visible — do not treat dip as cheap alone.".to_string());
    }

    let fund_lbl = valuation.valuation_label.as_str();
    let tech_lbl = detail;
    let combined = format!(
        "Fundamentally {} but Technically {} — technicals describe entry timing only, not intrinsic value.",
        fund_lbl, tech_lbl
    );

    TechnicalEntrySignal {
        zone: zone.to_string(),
        detail_label: detail.to_string(),
        rationale,
        fundamental_vs_technical: combined,
    }
}
