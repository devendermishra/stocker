//! Heuristic sector research: Porter Five Forces, lifecycle, type, and industry profile.
//! Yahoo / listed-company proxies only — not investment advice.

use crate::math::median;
use crate::models::{
    CompetitionNature, CompetitionStructure, DemandSupplyAssessment, DemandSupplyGap,
    GrowthProspects, GrowthProspectsLevel, IndustryPricingPower, PeerQuote, PorterFiveForces,
    PorterForce, PricingPowerLevel, PricingPowerSide, ProfitabilityAssessment, ProfitabilityLevel,
    SectorLifecycle, SectorLifecyclePhase, SectorResearchInputs, SectorResearchProfile,
    SectorTypeAssessment, SectorTypeKind,
};

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

fn intensity_label(intensity: f64) -> String {
    if intensity >= 66.0 {
        "High".to_string()
    } else if intensity >= 40.0 {
        "Moderate".to_string()
    } else {
        "Low".to_string()
    }
}

fn pricing_level(score: f64) -> PricingPowerLevel {
    // score 0–100: higher = more pricing power for that side
    if score >= 60.0 {
        PricingPowerLevel::High
    } else if score >= 35.0 {
        PricingPowerLevel::Moderate
    } else {
        PricingPowerLevel::Low
    }
}

fn fmt_opt_pct(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.decimals$}%"),
        _ => "n/a".to_string(),
    }
}

fn fmt_opt_ratio_as_pct(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.decimals$}%", x * 100.0),
        _ => "n/a".to_string(),
    }
}

fn fmt_opt_num(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.decimals$}"),
        _ => "n/a".to_string(),
    }
}

fn growth_for_signals(inp: &SectorResearchInputs) -> f64 {
    inp.median_sales_growth_3y_pct
        .or(inp.median_sales_growth_5y_pct)
        .or(inp.median_sales_growth_ttm_pct)
        .unwrap_or(0.0)
}

fn iqr(values: &[f64]) -> Option<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.len() < 4 {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    let q1 = v[n / 4];
    let q3 = v[(3 * n) / 4];
    Some(q3 - q1)
}

/// Build inputs from subject + peer quotes (report path). Yahoo growth fields are decimals.
pub fn sector_inputs_from_quotes(
    sector: &str,
    subject: &PeerQuote,
    peers: &[PeerQuote],
) -> SectorResearchInputs {
    let mut rows: Vec<&PeerQuote> = Vec::with_capacity(1 + peers.len());
    rows.push(subject);
    rows.extend(peers.iter());

    let mcaps: Vec<f64> = rows
        .iter()
        .map(|r| r.market_cap)
        .filter(|m| m.is_finite() && *m > 0.0)
        .collect();
    let total_mcap: f64 = mcaps.iter().sum();
    let mut shares: Vec<f64> = if total_mcap > 0.0 {
        mcaps.iter().map(|m| m / total_mcap).collect()
    } else {
        Vec::new()
    };
    shares.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let hhi = shares.iter().map(|s| s * s).sum::<f64>();
    let top3 = shares.iter().take(3).sum::<f64>();

    let sales_3y: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.revenue_growth.map(|g| g * 100.0))
        .collect();
    let profit_3y: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.pat_growth.map(|g| g * 100.0))
        .collect();
    let op_m: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.ebitda_margin)
        .collect();
    let net_m: Vec<f64> = rows
        .iter()
        .map(|r| r.profit_margins)
        .filter(|g| g.is_finite())
        .collect();
    let roe: Vec<f64> = rows.iter().filter_map(|r| r.return_on_equity).collect();
    let de: Vec<f64> = rows.iter().filter_map(|r| r.debt_to_equity).collect();
    let profitable = rows
        .iter()
        .filter(|r| r.profit_margins.is_finite() && r.profit_margins > 0.0)
        .count();

    SectorResearchInputs {
        sector: if sector.is_empty() {
            "Unclassified".to_string()
        } else {
            sector.to_string()
        },
        company_count: rows.len(),
        with_snapshot_count: rows.len(),
        hhi,
        top3_mcap_share: top3,
        median_gross_margin: median(op_m.iter().copied()), // proxy when gross missing
        median_op_margin: median(op_m),
        median_net_margin: median(net_m),
        median_roe: median(roe),
        median_debt_to_equity: median(de),
        median_sales_growth_ttm_pct: median(sales_3y.iter().copied()),
        median_sales_growth_3y_pct: median(sales_3y.iter().copied()),
        median_sales_growth_5y_pct: None,
        median_profit_growth_3y_pct: median(profit_3y),
        growth_dispersion_pct: iqr(&sales_3y),
        share_profitable: if rows.is_empty() {
            None
        } else {
            Some(profitable as f64 / rows.len() as f64)
        },
        margin_trend: None,
    }
}

/// Aggregate per-name cohort rows into [`SectorResearchInputs`].
/// Growth fields must already be percentages; margins/ROE decimals.
pub fn sector_inputs_from_aggregates(
    sector: &str,
    company_count: usize,
    rows: &[SectorCohortRow],
) -> SectorResearchInputs {
    let with_snap = rows.len();
    let mcaps: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.market_cap.filter(|m| m.is_finite() && *m > 0.0))
        .collect();
    let total_mcap: f64 = mcaps.iter().sum();
    let mut shares: Vec<f64> = if total_mcap > 0.0 {
        mcaps.iter().map(|m| m / total_mcap).collect()
    } else {
        Vec::new()
    };
    shares.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let hhi = shares.iter().map(|s| s * s).sum::<f64>();
    let top3 = shares.iter().take(3).sum::<f64>();

    let sales_3y: Vec<f64> = rows.iter().filter_map(|r| r.sales_growth_3y_pct).collect();
    let profitable_n = rows
        .iter()
        .filter(|r| r.profit_after_tax.map(|p| p > 0.0).unwrap_or(false))
        .count();
    let profitable_den = rows
        .iter()
        .filter(|r| r.profit_after_tax.is_some())
        .count();

    SectorResearchInputs {
        sector: if sector.is_empty() {
            "Unclassified".to_string()
        } else {
            sector.to_string()
        },
        company_count: if company_count > 0 {
            company_count
        } else {
            with_snap
        },
        with_snapshot_count: with_snap,
        hhi,
        top3_mcap_share: top3,
        median_gross_margin: median(rows.iter().filter_map(|r| r.gross_margins)),
        median_op_margin: median(
            rows.iter()
                .filter_map(|r| r.op_margin.or(r.ebitda_margins)),
        ),
        median_net_margin: median(rows.iter().filter_map(|r| r.net_margin)),
        median_roe: median(rows.iter().filter_map(|r| r.return_on_equity)),
        median_debt_to_equity: median(rows.iter().filter_map(|r| r.debt_to_equity)),
        median_sales_growth_ttm_pct: median(rows.iter().filter_map(|r| r.sales_growth_ttm_pct)),
        median_sales_growth_3y_pct: median(sales_3y.iter().copied()),
        median_sales_growth_5y_pct: median(rows.iter().filter_map(|r| r.sales_growth_5y_pct)),
        median_profit_growth_3y_pct: median(rows.iter().filter_map(|r| r.profit_growth_3y_pct)),
        growth_dispersion_pct: iqr(&sales_3y),
        share_profitable: if profitable_den == 0 {
            None
        } else {
            Some(profitable_n as f64 / profitable_den as f64)
        },
        margin_trend: {
            let latest: Vec<f64> = rows.iter().filter_map(|r| r.npm_latest).collect();
            let prior: Vec<f64> = rows.iter().filter_map(|r| r.npm_preceding).collect();
            match (median(latest), median(prior)) {
                (Some(a), Some(b)) => Some(a - b),
                _ => None,
            }
        },
    }
}

/// One listed name in a sector cohort (from screener snapshots).
#[derive(Debug, Clone, Default)]
pub struct SectorCohortRow {
    pub symbol: String,
    pub short_name: Option<String>,
    pub market_cap: Option<f64>,
    pub gross_margins: Option<f64>,
    pub ebitda_margins: Option<f64>,
    pub op_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub sales_growth_ttm_pct: Option<f64>,
    pub sales_growth_3y_pct: Option<f64>,
    pub sales_growth_5y_pct: Option<f64>,
    pub profit_growth_3y_pct: Option<f64>,
    pub profit_after_tax: Option<f64>,
    pub npm_latest: Option<f64>,
    pub npm_preceding: Option<f64>,
}

fn porter_force(name: &str, intensity: f64, narrative: String, evidence: Vec<String>) -> PorterForce {
    let intensity = clamp(intensity, 0.0, 100.0);
    PorterForce {
        name: name.to_string(),
        intensity,
        label: intensity_label(intensity),
        narrative,
        evidence,
    }
}

fn compute_porter(inp: &SectorResearchInputs) -> PorterFiveForces {
    let n = inp.company_count as f64;
    let hhi = inp.hhi;
    let op = inp.median_op_margin.unwrap_or(0.08);
    let gross = inp.median_gross_margin.unwrap_or(op);
    let net = inp.median_net_margin.unwrap_or(op * 0.6);
    let roe = inp.median_roe.unwrap_or(0.12);
    let de = inp.median_debt_to_equity.unwrap_or(0.5);
    let g = growth_for_signals(inp);

    // Rivalry: fragmented + many names + thin margins
    let rivalry_i = clamp(
        (1.0 - hhi) * 55.0 + (n / 40.0).min(1.0) * 25.0 + clamp((0.12 - op) / 0.12, 0.0, 1.0) * 20.0,
        0.0,
        100.0,
    );
    let rivalry = porter_force(
        "Competitive rivalry",
        rivalry_i,
        format!(
            "Low-confidence heuristic: rivalry intensity is {} based on listed concentration and operating margins.",
            intensity_label(rivalry_i).to_lowercase()
        ),
        vec![
            format!("HHI {:.2} (1.0 = monopoly)", hhi),
            format!("{} listed names in cohort", inp.company_count),
            format!("Median op margin {}", fmt_opt_ratio_as_pct(inp.median_op_margin, 1)),
        ],
    );

    // New entrants: attractive returns + low barriers (fragmented, low D/E)
    let attract = clamp((roe / 0.20) * 40.0 + (op / 0.15) * 30.0, 0.0, 70.0);
    let low_barrier = clamp((1.0 - hhi) * 40.0 + clamp(1.0 - de / 1.5, 0.0, 1.0) * 30.0, 0.0, 60.0);
    let entrants_i = clamp(attract * 0.55 + low_barrier * 0.45, 0.0, 100.0);
    let new_entrants = porter_force(
        "Threat of new entrants",
        entrants_i,
        format!(
            "Low-confidence heuristic: entry threat is {} — attractive economics raise interest while concentration/leverage shape barriers.",
            intensity_label(entrants_i).to_lowercase()
        ),
        vec![
            format!("Median ROE {}", fmt_opt_ratio_as_pct(inp.median_roe, 1)),
            format!("Median D/E {}", fmt_opt_num(inp.median_debt_to_equity, 2)),
            format!("Top-3 mcap share {:.0}%", inp.top3_mcap_share * 100.0),
        ],
    );

    // Supplier: low gross margin → high supplier power
    let supplier_i = clamp((0.35 - gross) / 0.35 * 100.0, 0.0, 100.0);
    let supplier_power = porter_force(
        "Bargaining power of suppliers",
        supplier_i,
        format!(
            "Low-confidence heuristic: supplier power is {} given median gross-margin levels among listed names.",
            intensity_label(supplier_i).to_lowercase()
        ),
        vec![format!(
            "Median gross margin {}",
            fmt_opt_ratio_as_pct(inp.median_gross_margin, 1)
        )],
    );

    // Buyer: low net/op margins → high buyer power
    let buyer_i = clamp((0.12 - net) / 0.12 * 70.0 + (0.15 - op) / 0.15 * 30.0, 0.0, 100.0);
    let buyer_power = porter_force(
        "Bargaining power of buyers",
        buyer_i,
        format!(
            "Low-confidence heuristic: buyer power is {} based on net and operating margin compression.",
            intensity_label(buyer_i).to_lowercase()
        ),
        vec![
            format!("Median net margin {}", fmt_opt_ratio_as_pct(inp.median_net_margin, 1)),
            format!("Median op margin {}", fmt_opt_ratio_as_pct(inp.median_op_margin, 1)),
        ],
    );

    // Substitutes: weak growth + soft margins
    let sub_i = clamp(
        clamp((-g) / 10.0, 0.0, 1.0) * 50.0
            + clamp((0.10 - op) / 0.10, 0.0, 1.0) * 30.0
            + if g < 5.0 { 20.0 } else { 0.0 },
        0.0,
        100.0,
    );
    let substitutes = porter_force(
        "Threat of substitutes",
        sub_i,
        format!(
            "Low-confidence heuristic: substitute pressure is {} using growth and margin proxies (listed data only; not a real substitutes analysis).",
            intensity_label(sub_i).to_lowercase()
        ),
        vec![
            format!("Median sales growth {}", fmt_opt_pct(Some(g), 1)),
            format!("Median op margin {}", fmt_opt_ratio_as_pct(inp.median_op_margin, 1)),
        ],
    );

    let attractiveness = clamp(
        100.0
            - (rivalry.intensity * 0.25
                + new_entrants.intensity * 0.20
                + supplier_power.intensity * 0.15
                + buyer_power.intensity * 0.20
                + substitutes.intensity * 0.20),
        0.0,
        100.0,
    );
    let summary = format!(
        "Heuristic industry attractiveness {:.0}/100 for {} (Yahoo listed-company metrics; not advice).",
        attractiveness, inp.sector
    );

    PorterFiveForces {
        rivalry,
        new_entrants,
        supplier_power,
        buyer_power,
        substitutes,
        attractiveness,
        summary,
    }
}

fn compute_lifecycle(inp: &SectorResearchInputs) -> SectorLifecycle {
    let g3 = inp.median_sales_growth_3y_pct.unwrap_or(growth_for_signals(inp));
    let g5 = inp.median_sales_growth_5y_pct.unwrap_or(g3);
    let gttm = inp.median_sales_growth_ttm_pct.unwrap_or(g3);
    let share_p = inp.share_profitable.unwrap_or(0.7);
    let op = inp.median_op_margin.unwrap_or(0.08);
    let n = inp.company_count;
    let hhi = inp.hhi;
    let decelerating = gttm + 5.0 < g3.min(g5);

    let mut evidence = vec![
        format!("Median sales 3Y {}", fmt_opt_pct(inp.median_sales_growth_3y_pct, 1)),
        format!("Median sales TTM {}", fmt_opt_pct(inp.median_sales_growth_ttm_pct, 1)),
        format!("HHI {:.2}; top-3 {:.0}%", hhi, inp.top3_mcap_share * 100.0),
        format!("Share profitable {}", fmt_opt_ratio_as_pct(inp.share_profitable, 0)),
    ];

    let (phase, narrative) = if n <= 8 && g3 >= 15.0 && (share_p < 0.55 || op < 0.05) && hhi < 0.25
    {
        (
            SectorLifecyclePhase::Startup,
            "Small listed cohort with fast growth and uneven profitability — startup-like phase.".to_string(),
        )
    } else if g3 >= 12.0 && share_p >= 0.55 && hhi < 0.45 {
        (
            SectorLifecyclePhase::Growth,
            "Elevated multi-year growth with workable profitability and still-open structure — growth phase."
                .to_string(),
        )
    } else if decelerating && (hhi >= 0.25 || inp.top3_mcap_share >= 0.45) {
        evidence.push("TTM growth lagging multi-year CAGR (deceleration)".to_string());
        (
            SectorLifecyclePhase::Consolidation,
            "Growth is cooling while concentration rises — consolidation phase.".to_string(),
        )
    } else if g3 < 5.0 || (g3 < 8.0 && hhi >= 0.30) {
        (
            SectorLifecyclePhase::MaturityOrDecline,
            "Soft multi-year growth and/or stable concentration — maturity or decline phase.".to_string(),
        )
    } else if decelerating {
        (
            SectorLifecyclePhase::Consolidation,
            "Growth decelerating versus history — leaning consolidation.".to_string(),
        )
    } else if g3 >= 8.0 {
        (
            SectorLifecyclePhase::Growth,
            "Solid growth without clear late-cycle concentration — growth phase.".to_string(),
        )
    } else {
        (
            SectorLifecyclePhase::MaturityOrDecline,
            "Limited growth signals among listed names — maturity or decline.".to_string(),
        )
    };

    SectorLifecycle {
        phase,
        narrative,
        evidence,
    }
}

fn compute_sector_type(inp: &SectorResearchInputs) -> SectorTypeAssessment {
    let g = growth_for_signals(inp);
    let disp = inp.growth_dispersion_pct.unwrap_or(0.0);
    let g3 = inp.median_sales_growth_3y_pct.unwrap_or(g);
    let gttm = inp.median_sales_growth_ttm_pct.unwrap_or(g3);
    let gap = (gttm - g3).abs();
    let roe = inp.median_roe.unwrap_or(0.12);
    let cyclical = disp >= 18.0 || gap >= 12.0;
    let high_growth = g >= 12.0 && roe >= 0.10;
    let low_growth = g < 6.0;

    let evidence = vec![
        format!("Median growth {}", fmt_opt_pct(Some(g), 1)),
        format!("Growth dispersion (IQR) {}", fmt_opt_pct(inp.growth_dispersion_pct, 1)),
        format!("|TTM−3Y| gap {:.1} pp", gap),
        format!("Median ROE {}", fmt_opt_ratio_as_pct(inp.median_roe, 1)),
    ];

    let (sector_type, narrative) = if high_growth && cyclical {
        (
            SectorTypeKind::CyclicalGrowth,
            "High growth with elevated cross-name or path volatility — cyclical-growth.".to_string(),
        )
    } else if high_growth && !cyclical {
        (
            SectorTypeKind::Growth,
            "High median growth with relatively stable paths — growth sector.".to_string(),
        )
    } else if cyclical {
        (
            SectorTypeKind::Cyclical,
            "Large growth dispersion or TTM vs multi-year swings — cyclical sector.".to_string(),
        )
    } else if low_growth && disp < 12.0 {
        (
            SectorTypeKind::Defensive,
            "Modest growth and low dispersion — defensive-like profile.".to_string(),
        )
    } else {
        (
            SectorTypeKind::Defensive,
            "Neither strong growth nor strong cyclicality — classified defensive.".to_string(),
        )
    };

    SectorTypeAssessment {
        sector_type,
        narrative,
        evidence,
    }
}

fn compute_demand_supply(inp: &SectorResearchInputs) -> DemandSupplyAssessment {
    let g = growth_for_signals(inp);
    let op = inp.median_op_margin.unwrap_or(0.08);
    let roe = inp.median_roe.unwrap_or(0.12);
    let de = inp.median_debt_to_equity.unwrap_or(0.5);

    let evidence = vec![
        format!("Median sales growth {}", fmt_opt_pct(Some(g), 1)),
        format!("Median op margin {}", fmt_opt_ratio_as_pct(inp.median_op_margin, 1)),
        format!("Median ROE {}", fmt_opt_ratio_as_pct(inp.median_roe, 1)),
        format!("Median D/E {}", fmt_opt_num(inp.median_debt_to_equity, 2)),
    ];

    let (gap_label, intensity, narrative) = (
        DemandSupplyGap::Balanced,
        40.0,
        format!(
            "Low-confidence heuristic: listed median sales growth is {}. That is not evidence of a physical shortage or glut (prices, M&A, FX, and mix can move revenue without supply tightness).",
            fmt_opt_pct(Some(g), 1)
        ),
    );
    let _ = (op, roe, de);

    DemandSupplyAssessment {
        gap_label,
        intensity,
        narrative,
        evidence,
    }
}

fn competition_label(s: CompetitionStructure) -> &'static str {
    match s {
        CompetitionStructure::Fragmented => "Fragmented",
        CompetitionStructure::ModeratelyConcentrated => "Moderately concentrated",
        CompetitionStructure::Oligopolistic => "Oligopolistic",
    }
}

fn compute_competition(inp: &SectorResearchInputs, rivalry_intensity: f64) -> CompetitionNature {
    let structure = if inp.hhi >= 0.25 || inp.top3_mcap_share >= 0.55 {
        CompetitionStructure::Oligopolistic
    } else if inp.hhi >= 0.12 || inp.top3_mcap_share >= 0.35 {
        CompetitionStructure::ModeratelyConcentrated
    } else {
        CompetitionStructure::Fragmented
    };
    let narrative = format!(
        "{} structure among listed peers; rivalry intensity {:.0}/100.",
        competition_label(structure),
        rivalry_intensity
    );
    CompetitionNature {
        structure,
        narrative,
        evidence: vec![
            format!("HHI {:.2}", inp.hhi),
            format!("Top-3 mcap share {:.0}%", inp.top3_mcap_share * 100.0),
            format!("{} companies", inp.company_count),
            format!("Rivalry {}", intensity_label(rivalry_intensity)),
        ],
    }
}

fn compute_profitability(inp: &SectorResearchInputs) -> ProfitabilityAssessment {
    let roe = inp.median_roe.unwrap_or(0.12);
    let npm = inp.median_net_margin.unwrap_or(0.08);
    let share = inp.share_profitable.unwrap_or(0.5);
    let score = clamp(
        (roe / 0.20) * 45.0 + (npm / 0.12) * 35.0 + share * 20.0,
        0.0,
        100.0,
    );
    let level = if share < 0.55 && score < 55.0 {
        ProfitabilityLevel::Mixed
    } else if score >= 70.0 {
        ProfitabilityLevel::High
    } else if score >= 40.0 {
        ProfitabilityLevel::Moderate
    } else {
        ProfitabilityLevel::Low
    };
    ProfitabilityAssessment {
        level,
        score,
        narrative: format!(
            "Profitability is {:?} among listed names (score {:.0}/100).",
            level, score
        ),
        evidence: vec![
            format!("Median ROE {}", fmt_opt_ratio_as_pct(inp.median_roe, 1)),
            format!("Median NPM {}", fmt_opt_ratio_as_pct(inp.median_net_margin, 1)),
            format!("Share profitable {}", fmt_opt_ratio_as_pct(inp.share_profitable, 0)),
        ],
    }
}

fn compute_growth_prospects(inp: &SectorResearchInputs) -> GrowthProspects {
    let g3 = inp.median_sales_growth_3y_pct.unwrap_or(0.0);
    let g5 = inp.median_sales_growth_5y_pct.unwrap_or(g3);
    let gttm = inp.median_sales_growth_ttm_pct.unwrap_or(g3);
    let pg = inp.median_profit_growth_3y_pct.unwrap_or(g3);
    let multi = (g3 + g5) / 2.0;
    let momentum = gttm - g3;
    let score = clamp(multi * 3.0 + pg * 1.5 + momentum * 1.5 + 40.0, 0.0, 100.0);
    let level = if multi < 0.0 || (multi < 3.0 && momentum < -5.0) {
        GrowthProspectsLevel::Contracting
    } else if score >= 70.0 || multi >= 15.0 {
        GrowthProspectsLevel::Strong
    } else if score >= 50.0 || multi >= 8.0 {
        GrowthProspectsLevel::Moderate
    } else {
        GrowthProspectsLevel::Weak
    };
    GrowthProspects {
        level,
        score,
        narrative: format!(
            "Growth prospects {:?} (heuristic from sales/profit CAGRs and TTM momentum).",
            level
        ),
        evidence: vec![
            format!("Sales 3Y {}", fmt_opt_pct(inp.median_sales_growth_3y_pct, 1)),
            format!("Sales 5Y {}", fmt_opt_pct(inp.median_sales_growth_5y_pct, 1)),
            format!("Sales TTM {}", fmt_opt_pct(inp.median_sales_growth_ttm_pct, 1)),
            format!("Profit 3Y {}", fmt_opt_pct(inp.median_profit_growth_3y_pct, 1)),
        ],
    }
}

fn compute_pricing_power(inp: &SectorResearchInputs) -> IndustryPricingPower {
    let gross = inp.median_gross_margin.unwrap_or(0.25);
    let net = inp.median_net_margin.unwrap_or(0.08);
    let op = inp.median_op_margin.unwrap_or(0.10);

    // Higher margin → lower supplier power / higher firm pricing vs suppliers
    let supplier_score = clamp((gross / 0.40) * 100.0, 0.0, 100.0);
    let customer_score = clamp((net / 0.15) * 70.0 + (op / 0.18) * 30.0, 0.0, 100.0);

    let supplier = PricingPowerSide {
        level: pricing_level(supplier_score),
        score: supplier_score,
        narrative: format!(
            "Industry vs suppliers: {:?} pricing power (gross-margin proxy).",
            pricing_level(supplier_score)
        ),
        evidence: vec![format!(
            "Median gross margin {}",
            fmt_opt_ratio_as_pct(inp.median_gross_margin, 1)
        )],
    };
    let customer = PricingPowerSide {
        level: pricing_level(customer_score),
        score: customer_score,
        narrative: format!(
            "Industry vs customers: {:?} pricing power (net/op margin proxy).",
            pricing_level(customer_score)
        ),
        evidence: vec![
            format!("Median net margin {}", fmt_opt_ratio_as_pct(inp.median_net_margin, 1)),
            format!("Median op margin {}", fmt_opt_ratio_as_pct(inp.median_op_margin, 1)),
        ],
    };
    IndustryPricingPower { supplier, customer }
}

pub fn compute_sector_research_profile(inp: &SectorResearchInputs) -> SectorResearchProfile {
    let porter = compute_porter(inp);
    let lifecycle = compute_lifecycle(inp);
    let sector_type = compute_sector_type(inp);
    let demand_supply = compute_demand_supply(inp);
    let competition = compute_competition(inp, porter.rivalry.intensity);
    let profitability = compute_profitability(inp);
    let growth_prospects = compute_growth_prospects(inp);
    let pricing_power = compute_pricing_power(inp);

    SectorResearchProfile {
        sector: inp.sector.clone(),
        company_count: inp.company_count,
        porter,
        lifecycle,
        sector_type,
        demand_supply,
        competition,
        profitability,
        growth_prospects,
        pricing_power,
        interpretation_confidence: "low".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_inputs() -> SectorResearchInputs {
        SectorResearchInputs {
            sector: "Technology".to_string(),
            company_count: 25,
            with_snapshot_count: 20,
            hhi: 0.08,
            top3_mcap_share: 0.32,
            median_gross_margin: Some(0.40),
            median_op_margin: Some(0.18),
            median_net_margin: Some(0.14),
            median_roe: Some(0.18),
            median_debt_to_equity: Some(0.2),
            median_sales_growth_ttm_pct: Some(16.0),
            median_sales_growth_3y_pct: Some(14.0),
            median_sales_growth_5y_pct: Some(13.0),
            median_profit_growth_3y_pct: Some(15.0),
            growth_dispersion_pct: Some(8.0),
            share_profitable: Some(0.9),
            margin_trend: Some(0.01),
        }
    }

    #[test]
    fn growth_sector_profile() {
        let p = compute_sector_research_profile(&sample_inputs());
        assert_eq!(p.lifecycle.phase, SectorLifecyclePhase::Growth);
        assert_eq!(p.sector_type.sector_type, SectorTypeKind::Growth);
        assert!(p.porter.attractiveness > 40.0);
        assert_eq!(p.demand_supply.gap_label, DemandSupplyGap::Balanced);
        assert_eq!(p.interpretation_confidence, "low");
    }

    #[test]
    fn mature_oversupply() {
        let mut inp = sample_inputs();
        inp.median_sales_growth_ttm_pct = Some(1.0);
        inp.median_sales_growth_3y_pct = Some(2.0);
        inp.median_sales_growth_5y_pct = Some(2.5);
        inp.median_op_margin = Some(0.04);
        inp.median_net_margin = Some(0.02);
        inp.median_roe = Some(0.06);
        inp.median_debt_to_equity = Some(1.2);
        inp.hhi = 0.35;
        inp.top3_mcap_share = 0.60;
        let p = compute_sector_research_profile(&inp);
        assert_eq!(p.lifecycle.phase, SectorLifecyclePhase::MaturityOrDecline);
        assert_eq!(p.demand_supply.gap_label, DemandSupplyGap::Balanced);
    }
}
