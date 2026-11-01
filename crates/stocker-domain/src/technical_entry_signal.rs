use crate::models::{
    FundamentalAnalysis, TechnicalAnalysis, TechnicalEntrySignal, ValuationAnalysis,
};

pub fn build_technical_entry_signal(
    technical: &TechnicalAnalysis,
    fundamental: &FundamentalAnalysis,
    valuation: &ValuationAnalysis,
) -> TechnicalEntrySignal {
    let st = &technical.state;
    let dist_hi = technical.volatility.dist_from_high_pct;
    let roc1 = technical.momentum.roc_1m_pct;
    let vol = technical.volatility.vol_1y_ann_pct;

    let weak_fund = fundamental.profitability.interpretation.contains("Poor")
        || fundamental.growth.interpretation.contains("Weak");
    let deteriorating = fundamental.cash_flow.interpretation.contains("Weak");

    let below_200 = st.above_dma200 == Some(false);
    let rsi_below_35 = st.rsi_below_35 == Some(true);
    let rsi_overbought = technical.momentum.rsi_14.map(|r| r > 70.0).unwrap_or(false);
    let stretched_50 = st.price_stretched_vs_50 == Some(true);
    let near_high = dist_hi.map(|d| d < 8.0).unwrap_or(false);
    let rsi_neutral = technical
        .momentum
        .rsi_14
        .map(|r| (45.0..=60.0).contains(&r))
        .unwrap_or(false);
    let near_50 = technical
        .trend
        .price_vs_sma50_pct
        .map(|p| p.abs() < 6.0)
        .unwrap_or(false);
    let room_from_high = dist_hi.map(|d| d > 8.0).unwrap_or(false);

    let (zone, detail, mut rationale) = if rsi_below_35 && below_200 {
        (
            "Technically oversold / near-oversold zone",
            if weak_fund && deteriorating {
                "Falling Knife Risk"
            } else if weak_fund {
                "Weak but Cheap"
            } else {
                "Possible Technical Entry Zone"
            },
            vec![
                "RSI below 35 and price below 200 DMA (heuristic).".to_string(),
                "Check fundamentals before averaging down.".to_string(),
            ],
        )
    } else if rsi_overbought && stretched_50 && near_high {
        (
            "Technically extended",
            "Momentum Strong but Costly",
            vec![
                "RSI elevated and price stretched vs 50 DMA / 52-week high.".to_string(),
                "Better entry may come on consolidation.".to_string(),
            ],
        )
    } else if rsi_neutral && near_50 && room_from_high {
        (
            "Technically fair",
            "Neutral Zone",
            vec![
                "Price near moving averages without extreme RSI.".to_string(),
                "May wait for breakout or pullback for clearer risk/reward.".to_string(),
            ],
        )
    } else if vol.map(|v| v > 35.0).unwrap_or(false) {
        (
            "Technically fair",
            "High volatility",
            vec!["Annualized volatility is elevated — size positions conservatively.".to_string()],
        )
    } else {
        (
            "Technically fair",
            if roc1.map(|r| r > 8.0).unwrap_or(false) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        FundamentalAnalysis, FundamentalSection, TechnicalAnalysis, TechnicalMomentum, TechnicalState,
        TechnicalTrend, TechnicalVolatility, TechnicalVolume, ValuationAnalysis,
    };

    fn empty_section() -> FundamentalSection {
        FundamentalSection {
            title: String::new(),
            interpretation: String::new(),
            flags: vec![],
            confidence: "High".into(),
            lines: vec![],
        }
    }

    #[test]
    fn recovery_rsi_neutral_does_not_claim_oversold() {
        let technical = TechnicalAnalysis {
            trend: TechnicalTrend {
                sma_50: Some(1306.59),
                sma_200: Some(1399.52),
                price_vs_sma50_pct: Some(0.72),
                price_vs_sma200_pct: Some(-5.97),
                trend_label: "Recovery (above 50 DMA, below 200 DMA)".into(),
                ..Default::default()
            },
            momentum: TechnicalMomentum {
                rsi_14: Some(49.04),
                rsi_label: "Neutral".into(),
                ..Default::default()
            },
            volatility: TechnicalVolatility::default(),
            volume: TechnicalVolume::default(),
            confidence: "High".into(),
            state: TechnicalState {
                above_dma50: Some(true),
                above_dma200: Some(false),
                rsi_oversold: Some(false),
                rsi_weak: Some(false),
                rsi_below_35: Some(false),
                macd_bullish: Some(true),
                price_stretched_vs_50: Some(false),
            },
        };
        let fundamental = FundamentalAnalysis {
            growth: empty_section(),
            profitability: empty_section(),
            balance_sheet: empty_section(),
            cash_flow: empty_section(),
            efficiency: empty_section(),
        };
        let valuation = ValuationAnalysis {
            valuation_label: "Fair".into(),
            ..Default::default()
        };
        let sig = build_technical_entry_signal(&technical, &fundamental, &valuation);
        assert!(!sig.rationale.iter().any(|s| s.contains("RSI below 35")));
        assert_ne!(sig.zone, "Technically oversold / value zone");
    }
}
