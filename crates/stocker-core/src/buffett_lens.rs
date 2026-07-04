//! Berkshire / Buffett–Munger style heuristic lens: owner earnings, moat, capital
//! intensity, management trust, and margin of safety. Shared by research reports and screener.

use crate::financial_strength_audit::cumulative_cfo_pat_for_bundle;
use crate::math::median;
use crate::models::{
    AssetProfile, BuffettLensReport, BuffettLensScores, BuffettScoreReason, BusinessTierLabel,
    CapitalIntensity, EarningsPicture, FinancialStrengthAudit, Financials, ManagementAnalysis,
    ManagementTrust, ManagementTrustInputs, MoatAssessment, MoatDurabilityLabel, MoatTypeHit,
    PeerQuote, PriceVsValue, ScreenerMetricSnapshot, StatementBundle, StockAnalysis,
    ValuationAnalysis,
};

fn clamp_score(x: f64) -> f64 {
    x.clamp(0.0, 100.0)
}

fn avg_maintenance_capex(bundle: &StatementBundle, years: usize) -> Option<f64> {
    let mut cf = bundle.cashflow_annual.clone();
    cf.sort_by(|a, b| b.end_ts.unwrap_or(0).cmp(&a.end_ts.unwrap_or(0)));
    if cf.is_empty() {
        return None;
    }
    let take = cf.len().min(years);
    let sum: f64 = cf
        .iter()
        .take(take)
        .map(|r| r.capital_expenditure.abs())
        .sum();
    Some(sum / take as f64)
}

fn owner_earnings_from(financials: &Financials, bundle: &StatementBundle) -> (Option<f64>, Option<f64>) {
    let cfo = if financials.operating_cashflow.abs() > 0.0 {
        financials.operating_cashflow
    } else {
        bundle
            .cashflow_annual
            .first()
            .map(|r| r.operating_cashflow)
            .unwrap_or(0.0)
    };
    if cfo <= 0.0 {
        return (None, None);
    }
    let maint = avg_maintenance_capex(bundle, 3)
        .or_else(|| {
            if financials.free_cashflow != 0.0 || cfo != 0.0 {
                Some((cfo - financials.free_cashflow).max(0.0))
            } else {
                None
            }
        })
        .unwrap_or(0.0);
    let oe = cfo - maint;
    if oe <= 0.0 {
        return (None, Some(maint));
    }
    (Some(oe), Some(maint))
}

fn margin_trend_label(financials: &Financials, bundle: &StatementBundle) -> &'static str {
    let mut inc = bundle.income_annual.clone();
    inc.sort_by(|a, b| a.end_ts.unwrap_or(0).cmp(&b.end_ts.unwrap_or(0)));
    if inc.len() < 2 {
        return "Stable";
    }
    let first = &inc[0];
    let last = inc.last().unwrap();
    let m0 = if first.revenue > 0.0 {
        first.net_income / first.revenue
    } else {
        0.0
    };
    let m1 = if last.revenue > 0.0 {
        last.net_income / last.revenue
    } else {
        financials.profit_margins
    };
    if m1 > m0 + 0.02 {
        "Improving"
    } else if m1 < m0 - 0.02 {
        "Compressing"
    } else {
        "Stable"
    }
}

fn ni_volatility(bundle: &StatementBundle) -> Option<f64> {
    let mut inc = bundle.income_annual.clone();
    inc.sort_by(|a, b| a.end_ts.unwrap_or(0).cmp(&b.end_ts.unwrap_or(0)));
    if inc.len() < 3 {
        return None;
    }
    let vals: Vec<f64> = inc.iter().map(|r| r.net_income).collect();
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    if mean.abs() < 1e-6 {
        return None;
    }
    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64;
    Some(var.sqrt() / mean.abs())
}

fn is_asset_light(profile: &AssetProfile) -> bool {
    let sector = profile.sector.as_deref().unwrap_or("").to_lowercase();
    let industry = profile.industry.as_deref().unwrap_or("").to_lowercase();
    sector.contains("technology")
        || sector.contains("communication")
        || industry.contains("software")
        || industry.contains("internet")
        || industry.contains("it services")
}

fn infer_moat_types(
    financials: &Financials,
    profile: &AssetProfile,
    margin_trend: &str,
    peers: &[PeerQuote],
) -> Vec<MoatTypeHit> {
    let mut hits = Vec::new();
    let summary = profile.long_business_summary.as_deref().unwrap_or("").to_lowercase();
    let peer_roce_med = {
        let xs: Vec<f64> = peers
            .iter()
            .filter_map(|p| p.return_on_capital_employed.map(|r| r * 100.0))
            .collect();
        median(xs)
    };
    let roce = financials
        .return_on_capital_employed
        .map(|r| r * 100.0)
        .unwrap_or(0.0);

    if financials.gross_margins > 0.40 && margin_trend != "Compressing" {
        hits.push(MoatTypeHit {
            moat_type: "brand".to_string(),
            label: "Brand / pricing power".to_string(),
            evidence: format!(
                "Gross margin {:.0}% with {} margin trend.",
                financials.gross_margins * 100.0,
                margin_trend.to_lowercase()
            ),
        });
    }
    if let Some(med) = peer_roce_med {
        if roce > med && financials.profit_margins < 0.15 {
            hits.push(MoatTypeHit {
                moat_type: "cost".to_string(),
                label: "Cost advantage".to_string(),
                evidence: format!(
                    "ROCE {:.1}% above peer median {:.1}% despite modest net margins.",
                    roce, med
                ),
            });
        }
    }
    for (kw, label) in [
        ("platform", "Network effects"),
        ("marketplace", "Network effects"),
        ("network", "Network effects"),
    ] {
        if summary.contains(kw) && margin_trend != "Compressing" {
            hits.push(MoatTypeHit {
                moat_type: "network".to_string(),
                label: label.to_string(),
                evidence: format!(
                    "Business summary suggests network/platform economics; margins {}.",
                    margin_trend.to_lowercase()
                ),
            });
            break;
        }
    }
    for kw in ["regulated", "license", "utility", "bank", "insurance"] {
        if summary.contains(kw)
            || profile
                .sector
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(kw)
        {
            hits.push(MoatTypeHit {
                moat_type: "regulatory".to_string(),
                label: "Regulatory / scale".to_string(),
                evidence: "Sector or summary wording points to regulatory or scale barriers."
                    .to_string(),
            });
            break;
        }
    }
    if financials.operating_margins > 0.18
        && financials.revenue_growth > 0.03
        && margin_trend != "Compressing"
    {
        hits.push(MoatTypeHit {
            moat_type: "habit".to_string(),
            label: "Customer captivity / habit".to_string(),
            evidence: format!(
                "Solid operating margin {:.0}% with positive revenue growth.",
                financials.operating_margins * 100.0
            ),
        });
    }
    hits
}

fn compute_moat_score(
    financials: &Financials,
    bundle: &StatementBundle,
    margin_trend: &str,
    cfo_pat_3y: Option<f64>,
) -> f64 {
    let mut score = 38.0_f64;
    let roe = financials.return_on_equity * 100.0;
    if roe > 18.0 {
        score += 16.0;
    } else if roe > 12.0 {
        score += 9.0;
    } else if roe < 5.0 {
        score -= 10.0;
    }
    if let Some(roce) = financials.return_on_capital_employed {
        let r = roce * 100.0;
        if r > 18.0 {
            score += 12.0;
        } else if r > 12.0 {
            score += 6.0;
        }
    }
    if financials.gross_margins > 0.40 {
        score += 8.0;
    } else if financials.gross_margins > 0.25 {
        score += 4.0;
    }
    if financials.profit_margins > 0.12 {
        score += 8.0;
    } else if financials.profit_margins < 0.08 {
        score -= 12.0;
    }
    match margin_trend {
        "Improving" => score += 8.0,
        "Compressing" => score -= 12.0,
        _ => {}
    }
    if let Some(r) = cfo_pat_3y {
        if r >= 1.0 {
            score += 10.0;
        } else if r < 0.8 {
            score -= 10.0;
        }
    }
    if financials.free_cashflow > 0.0 {
        score += 6.0;
    } else if financials.net_income > 0.0 {
        score -= 8.0;
    }
    if financials.revenue_growth > 0.08 {
        score += 5.0;
    } else if financials.revenue_growth < 0.0 {
        score -= 5.0;
    }
    if let Some(vol) = ni_volatility(bundle) {
        if vol < 0.25 {
            score += 5.0;
        } else if vol > 0.6 {
            score -= 6.0;
        }
    }
    clamp_score(score)
}

fn durability_label(moat_score: f64) -> MoatDurabilityLabel {
    if moat_score >= 70.0 {
        MoatDurabilityLabel::Wide
    } else if moat_score >= 45.0 {
        MoatDurabilityLabel::Narrow
    } else if moat_score >= 25.0 {
        MoatDurabilityLabel::Uncertain
    } else {
        MoatDurabilityLabel::None
    }
}

fn earnings_durability_score(
    moat_score: f64,
    margin_trend: &str,
    cfo_pat_3y: Option<f64>,
    audit_score: Option<f64>,
) -> f64 {
    let mut s = moat_score * 0.55;
    match margin_trend {
        "Improving" => s += 12.0,
        "Compressing" => s -= 15.0,
        _ => s += 4.0,
    }
    if let Some(r) = cfo_pat_3y {
        if r >= 1.0 {
            s += 15.0;
        } else if r < 0.85 {
            s -= 12.0;
        }
    }
    if let Some(a) = audit_score {
        s += (a - 50.0) * 0.2;
    }
    clamp_score(s)
}

fn capital_intensity_score(
    financials: &Financials,
    bundle: &StatementBundle,
    profile: &AssetProfile,
) -> f64 {
    let cfo = financials.operating_cashflow.max(1.0);
    let capex_ratio = avg_maintenance_capex(bundle, 3)
        .map(|c| c / cfo)
        .unwrap_or_else(|| {
            if cfo > 0.0 {
                ((cfo - financials.free_cashflow).max(0.0)) / cfo
            } else {
                1.0
            }
        });
    let mut score = 70.0 - capex_ratio * 80.0;
    if is_asset_light(profile) {
        score += 10.0;
    }
    if financials.return_on_capital_employed.map(|r| r > 0.15).unwrap_or(false) && capex_ratio < 0.35
    {
        score += 12.0;
    }
    if capex_ratio > 0.55 {
        score -= 15.0;
    }
    clamp_score(score)
}

fn management_trust_score(
    financials: &Financials,
    bundle: &StatementBundle,
    inputs: &ManagementTrustInputs,
) -> f64 {
    let mut score = 48.0_f64;
    let cfo_pat = cumulative_cfo_pat_for_bundle(bundle, 3).or(inputs.cumulative_cfo_pat_3y);
    if let Some(r) = cfo_pat {
        if r >= 1.0 {
            score += 18.0;
        } else if r < 0.85 {
            score -= 15.0;
        }
    }
    if financials.debt_to_equity < 0.50 {
        score += 10.0;
    } else if financials.debt_to_equity > 1.0 {
        score -= 15.0;
    }
    if let Some(ic) = inputs.interest_coverage {
        if ic >= 4.0 {
            score += 8.0;
        } else if ic < 2.0 {
            score -= 12.0;
        }
    }
    let roe = financials.return_on_equity * 100.0;
    if roe > 15.0 {
        score += 10.0;
    } else if roe < 6.0 {
        score -= 8.0;
    }
    if let Some(p) = inputs.piotroski {
        if p >= 7.0 {
            score += 10.0;
        } else if p <= 3.0 {
            score -= 10.0;
        }
    }
    if let Some(p) = inputs.pledge_pct {
        if p > 0.0 {
            score -= 18.0;
        }
    }
    if let Some(net) = inputs.insider_net_shares {
        if net > 0.0 {
            score += 6.0;
        } else if net < 0.0 {
            score -= 8.0;
        }
    }
    if let Some(p) = inputs.pay_vs_revenue_score {
        if p > 70.0 {
            score += 6.0;
        } else if p < 45.0 {
            score -= 8.0;
        }
    }
    clamp_score(score)
}

fn trust_verdict(score: f64) -> String {
    if score >= 72.0 {
        "Trustworthy (heuristic)".to_string()
    } else if score >= 55.0 {
        "Mixed — verify in filings".to_string()
    } else if score >= 35.0 {
        "Caution".to_string()
    } else {
        "Insufficient / weak signals".to_string()
    }
}

fn business_tier_from(moat: f64, trust: f64, durability: f64) -> f64 {
    let composite = moat * 0.45 + trust * 0.25 + durability * 0.30;
    if composite >= 75.0 {
        4.0
    } else if composite >= 58.0 {
        3.0
    } else if composite >= 40.0 {
        2.0
    } else {
        1.0
    }
}

pub fn business_tier_label(tier: f64) -> BusinessTierLabel {
    match tier as i32 {
        4 => BusinessTierLabel::Wonderful,
        3 => BusinessTierLabel::Good,
        2 => BusinessTierLabel::Mediocre,
        _ => BusinessTierLabel::Weak,
    }
}

fn margin_of_safety_pct(price: f64, fair_value: f64) -> Option<f64> {
    if price > 0.0 && fair_value > 0.0 {
        Some(((fair_value - price) / fair_value) * 100.0)
    } else {
        None
    }
}

fn fair_value_heuristic(financials: &Financials, historical_pe: Option<f64>) -> f64 {
    let eps = financials.trailing_eps;
    if eps <= 0.0 {
        return 0.0;
    }
    let fair_pe = historical_pe.unwrap_or(18.0).clamp(8.0, 45.0);
    eps * fair_pe * 0.85
}

#[allow(clippy::too_many_arguments)]
fn build_score_reasons(
    financials: &Financials,
    bundle: &StatementBundle,
    profile: &AssetProfile,
    margin_trend: &str,
    cfo_pat_3y: Option<f64>,
    scores: &BuffettLensScores,
    moat_score: f64,
    trust_inputs: &ManagementTrustInputs,
    price: f64,
    fair_value: f64,
    audit: &FinancialStrengthAudit,
) -> Vec<BuffettScoreReason> {
    let roe = financials.return_on_equity * 100.0;
    let mut moat_bits = vec![format!("ROE {:.1}%", roe)];
    if let Some(roce) = financials.return_on_capital_employed {
        moat_bits.push(format!("ROCE {:.1}%", roce * 100.0));
    }
    moat_bits.push(format!("net margin {:.1}%", financials.profit_margins * 100.0));
    moat_bits.push(format!("margins {margin_trend}"));
    if let Some(r) = cfo_pat_3y {
        moat_bits.push(format!("3Y CFO/PAT {:.2}", r));
    }
    if financials.free_cashflow <= 0.0 && financials.net_income > 0.0 {
        moat_bits.push("positive PAT but weak FCF".to_string());
    }

    let durability = scores.earnings_durability_score.unwrap_or(0.0);
    let mut dur_bits = vec![
        format!("moat {:.0}/100 anchors durability", moat_score),
        format!("margin trend {margin_trend}"),
    ];
    if let Some(r) = cfo_pat_3y {
        dur_bits.push(format!(
            "cash conversion 3Y CFO/PAT {:.2} {}",
            r,
            if r >= 1.0 { "supports" } else { "weakens" }
        ));
    }
    if audit.earnings_quality_score > 0.0 {
        dur_bits.push(format!(
            "financial strength audit {:.0}/100",
            audit.earnings_quality_score
        ));
    }

    let capex_ratio = avg_maintenance_capex(bundle, 3)
        .map(|c| c / financials.operating_cashflow.max(1.0))
        .unwrap_or(0.0);
    let cap_score = scores.capital_intensity_score.unwrap_or(0.0);
    let cap_bits = vec![
        format!("maintenance capex ~{:.0}% of CFO", capex_ratio * 100.0),
        if is_asset_light(profile) {
            "asset-light sector profile".to_string()
        } else {
            "capital-intensive sector profile".to_string()
        },
        format!(
            "reinvestment burden {}",
            if cap_score >= 60.0 {
                "low"
            } else if cap_score >= 40.0 {
                "moderate"
            } else {
                "high"
            }
        ),
    ];

    let trust_score = scores.management_trust_score.unwrap_or(0.0);
    let mut trust_bits = Vec::new();
    if let Some(r) = cfo_pat_3y.or(trust_inputs.cumulative_cfo_pat_3y) {
        trust_bits.push(format!("3Y CFO/PAT {:.2}", r));
    }
    trust_bits.push(format!("D/E {:.2}", financials.debt_to_equity));
    if let Some(ic) = trust_inputs.interest_coverage {
        trust_bits.push(format!("interest coverage {:.1}x", ic));
    }
    if let Some(p) = trust_inputs.pledge_pct {
        if p > 0.0 {
            trust_bits.push(format!("promoter pledge {:.1}%", p));
        }
    }
    if audit.red_flags.is_empty() {
        trust_bits.push("no major audit red flags".to_string());
    } else {
        trust_bits.push(format!("audit flag: {}", audit.red_flags[0]));
    }

    let mos = scores
        .margin_of_safety_pct
        .or_else(|| margin_of_safety_pct(price, fair_value));
    let mos_bits = if fair_value > 0.0 && price > 0.0 {
        vec![
            format!("price {:.2} vs fair value heuristic {:.2}", price, fair_value),
            format!(
                "margin of safety {:.1}%",
                mos.unwrap_or(0.0)
            ),
        ]
    } else {
        vec!["fair value unavailable (negative or missing EPS)".to_string()]
    };

    let tier = scores.business_tier.unwrap_or(1.0);
    let tier_bits = vec![
        format!(
            "composite of moat {:.0}, trust {:.0}, durability {:.0}",
            moat_score, trust_score, durability
        ),
        format!("tier label {}", tier_label_str(business_tier_label(tier))),
    ];

    let owner_yield = scores.owner_earnings_yield_pct;
    let owner_bits = if let Some(y) = owner_yield {
        vec![format!(
            "owner earnings yield {:.1}% vs market cap",
            y
        )]
    } else {
        vec!["owner earnings not positive after maintenance capex".to_string()]
    };

    vec![
        BuffettScoreReason {
            dimension: "Moat".to_string(),
            score: moat_score,
            reason: moat_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Earnings durability".to_string(),
            score: durability,
            reason: dur_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Capital intensity".to_string(),
            score: cap_score,
            reason: cap_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Management trust".to_string(),
            score: trust_score,
            reason: trust_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Margin of safety".to_string(),
            score: mos.unwrap_or(0.0),
            reason: mos_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Business tier".to_string(),
            score: tier,
            reason: tier_bits.join("; "),
        },
        BuffettScoreReason {
            dimension: "Owner earnings yield".to_string(),
            score: owner_yield.unwrap_or(0.0),
            reason: owner_bits.join("; "),
        },
    ]
}

/// Numeric Berkshire Lens scores — used by screener refresh and reports.
pub fn compute_buffett_lens_scores(
    financials: &Financials,
    bundle: &StatementBundle,
    profile: &AssetProfile,
    price: f64,
    trust_inputs: &ManagementTrustInputs,
    historical_pe_5y: Option<f64>,
) -> BuffettLensScores {
    let margin_trend = margin_trend_label(financials, bundle);
    let cfo_pat_3y = cumulative_cfo_pat_for_bundle(bundle, 3).or(trust_inputs.cumulative_cfo_pat_3y);

    let moat_score = compute_moat_score(financials, bundle, margin_trend, cfo_pat_3y);
    let earnings_durability =
        earnings_durability_score(moat_score, margin_trend, cfo_pat_3y, None);
    let capital_score = capital_intensity_score(financials, bundle, profile);
    let trust_score = management_trust_score(financials, bundle, trust_inputs);

    let fair_value = fair_value_heuristic(financials, historical_pe_5y);
    let mos = margin_of_safety_pct(price, fair_value);

    let (owner_earnings, _) = owner_earnings_from(financials, bundle);
    let owner_earnings_yield_pct = owner_earnings.and_then(|oe| {
        if financials.market_cap > 0.0 {
            Some((oe / financials.market_cap) * 100.0)
        } else {
            None
        }
    });

    let business_tier = business_tier_from(moat_score, trust_score, earnings_durability);

    BuffettLensScores {
        owner_earnings_ttm: owner_earnings,
        owner_earnings_yield_pct,
        moat_score: Some(moat_score),
        earnings_durability_score: Some(earnings_durability),
        capital_intensity_score: Some(capital_score),
        management_trust_score: Some(trust_score),
        margin_of_safety_pct: mos,
        business_tier: Some(business_tier),
    }
}

fn scores_from_enrichment(enrichment: &ScreenerMetricSnapshot) -> Option<BuffettLensScores> {
    if enrichment.moat_score.is_none() && enrichment.owner_earnings_ttm.is_none() {
        return None;
    }
    Some(BuffettLensScores {
        owner_earnings_ttm: enrichment.owner_earnings_ttm,
        owner_earnings_yield_pct: enrichment.owner_earnings_yield_pct,
        moat_score: enrichment.moat_score,
        earnings_durability_score: enrichment.earnings_durability_score,
        capital_intensity_score: enrichment.capital_intensity_score,
        management_trust_score: enrichment.management_trust_score,
        margin_of_safety_pct: enrichment.margin_of_safety_pct,
        business_tier: enrichment.business_tier,
    })
}

fn graham_buffett_read(tier: f64, mos: Option<f64>, cheap: bool) -> String {
    let wonderful = tier >= 3.5;
    let mos_ok = mos.map(|m| m > 5.0).unwrap_or(false);
    match (wonderful, cheap, mos_ok) {
        (true, _, true) => {
            "Buffett-style: wonderful business at a reasonable price with margin of safety."
                .to_string()
        }
        (true, _, false) => {
            "Wonderful business, but price may not offer enough margin of safety — patience required."
                .to_string()
        }
        (false, true, _) => {
            "Graham-style: mediocre business at a cheap price — verify it is not a value trap."
                .to_string()
        }
        _ => {
            "Neither clearly wonderful nor clearly cheap — competition may be destroying economics."
                .to_string()
        }
    }
}

pub fn tier_label_str(label: BusinessTierLabel) -> &'static str {
    match label {
        BusinessTierLabel::Wonderful => "Wonderful",
        BusinessTierLabel::Good => "Good",
        BusinessTierLabel::Mediocre => "Mediocre",
        BusinessTierLabel::Weak => "Weak",
    }
}

/// Full narrative Berkshire Lens for research reports.
#[allow(clippy::too_many_arguments)]
pub fn build_buffett_lens(
    financials: &Financials,
    stock: &StockAnalysis,
    bundle: &StatementBundle,
    audit: &FinancialStrengthAudit,
    valuation: &ValuationAnalysis,
    management: &ManagementAnalysis,
    trust_inputs: &ManagementTrustInputs,
    profile: &AssetProfile,
    peers: &[PeerQuote],
    price: f64,
    screener: Option<&ScreenerMetricSnapshot>,
) -> BuffettLensReport {
    let margin_trend = margin_trend_label(financials, bundle);

    let mut scores = compute_buffett_lens_scores(
        financials,
        bundle,
        profile,
        price,
        trust_inputs,
        valuation.historical.median_pe_5y,
    );

    if let Some(enrichment) = screener.and_then(scores_from_enrichment) {
        scores = enrichment;
    }

    let moat_score = scores.moat_score.unwrap_or(0.0);
    let moat_types = infer_moat_types(financials, profile, margin_trend, peers);
    let durability = durability_label(moat_score);
    let competition_risk = if financials.profit_margins < 0.08 && margin_trend == "Compressing" {
        "High — low margins and compression suggest competition is destroying economics.".to_string()
    } else if margin_trend == "Compressing" {
        "Elevated — margins are compressing; monitor pricing power.".to_string()
    } else if moat_score >= 60.0 {
        "Lower — returns and margins suggest some structural protection.".to_string()
    } else {
        "Moderate — limited moat evidence from financials alone.".to_string()
    };

    let (owner_earnings, maint_capex) = owner_earnings_from(financials, bundle);
    let reported_pat = financials.net_income;

    let earnings_picture = EarningsPicture {
        owner_earnings_ttm: owner_earnings,
        reported_pat_ttm: reported_pat,
        free_cashflow: financials.free_cashflow,
        owner_earnings_yield_pct: scores.owner_earnings_yield_pct,
        roe_pct: financials.return_on_equity * 100.0,
        roce_pct: financials.return_on_capital_employed.map(|r| r * 100.0),
        gross_margin_pct: financials.gross_margins * 100.0,
        operating_margin_pct: financials.operating_margins * 100.0,
        maintenance_capex_estimate: maint_capex,
        narrative: format!(
            "Owner earnings (CFO − maintenance capex heuristic) {:.0} vs reported PAT {:.0}. \
             ROE {:.1}%. Focus on economic cash power, not adjusted earnings alone.",
            owner_earnings.unwrap_or(0.0),
            reported_pat,
            financials.return_on_equity * 100.0
        ),
    };

    let moat_assessment = MoatAssessment {
        score: moat_score,
        durability: durability.clone(),
        moat_types: moat_types.clone(),
        margin_trend: margin_trend.to_string(),
        competition_risk,
        narrative: if moat_types.is_empty() {
            format!(
                "Moat score {:.0}/100 ({:?}). No strong moat type inferred — treat as competitive commodity unless filings show otherwise.",
                moat_score, durability
            )
        } else {
            let types: Vec<_> = moat_types.iter().map(|m| m.label.clone()).collect();
            format!(
                "Moat score {:.0}/100 ({:?}). Inferred advantages: {}. Margin trend: {}.",
                moat_score,
                durability,
                types.join(", "),
                margin_trend.to_lowercase()
            )
        },
    };

    let capex_ratio = maint_capex
        .and_then(|c| {
            if financials.operating_cashflow > 0.0 {
                Some(c / financials.operating_cashflow)
            } else {
                None
            }
        })
        .unwrap_or(0.0);
    let capital_intensity = CapitalIntensity {
        score: scores.capital_intensity_score.unwrap_or(0.0),
        capex_to_cfo_ratio: capex_ratio,
        reinvestment_rate: if reported_pat > 0.0 {
            maint_capex.map(|c| c / reported_pat)
        } else {
            None
        },
        classification: if scores.capital_intensity_score.unwrap_or(0.0) >= 60.0 {
            "Asset-light compounder".to_string()
        } else if scores.capital_intensity_score.unwrap_or(0.0) >= 40.0 {
            "Moderate reinvestment needs".to_string()
        } else {
            "Capital-intensive".to_string()
        },
        narrative: format!(
            "{} Estimated maintenance capex consumes {:.0}% of CFO. \
             Buffett prefers high returns without constant heavy reinvestment.",
            if scores.capital_intensity_score.unwrap_or(0.0) >= 60.0 {
                "Can compound with modest capital."
            } else {
                "Requires meaningful ongoing capital."
            },
            capex_ratio * 100.0
        ),
    };

    let trust_score = scores.management_trust_score.unwrap_or(0.0);
    let management_trust = ManagementTrust {
        score: trust_score,
        verdict: trust_verdict(trust_score),
        integrity_note: if audit.red_flags.is_empty() {
            "No major automated earnings-quality red flags.".to_string()
        } else {
            format!(
                "Integrity caution: {}",
                audit.red_flags.first().cloned().unwrap_or_default()
            )
        },
        narrative: format!(
            "Management trust {:.0}/100 ({}). Officer pay efficiency {:.0}/100. \
             Integrity is non-negotiable — confirm related-party dealings and incentives in annual report.",
            trust_score,
            trust_verdict(trust_score),
            management.pay_vs_revenue_score
        ),
    };

    let tier = scores.business_tier.unwrap_or(1.0);
    let tier_label = business_tier_label(tier);
    let fair_value = valuation
        .earnings_based
        .fair_value
        .max(fair_value_heuristic(
            financials,
            valuation.historical.median_pe_5y,
        ));
    let mos = scores
        .margin_of_safety_pct
        .or_else(|| margin_of_safety_pct(price, fair_value));
    let cheap = valuation.valuation_label.contains("Cheap")
        || valuation.valuation_label.contains("Very Cheap");

    let price_vs_value = PriceVsValue {
        business_tier: tier,
        business_tier_label: tier_label,
        fair_value,
        margin_of_safety_pct: mos,
        upside_downside_pct: valuation.earnings_based.upside_downside_pct,
        graham_buffett_read: graham_buffett_read(tier, mos, cheap),
        narrative: format!(
            "Business tier: {}. Fair value heuristic {:.2} vs price {:.2}. MOS {:.1}%. {}",
            tier_label_str(business_tier_label(tier)),
            fair_value,
            price,
            mos.unwrap_or(0.0),
            if financials.debt_to_equity > 1.0 {
                "High leverage adds fragility — never risk what you need for what you do not need."
            } else {
                "Balance sheet leverage is not extreme on this screen."
            }
        ),
    };

    let accounting_flags: Vec<String> = audit
        .red_flags
        .iter()
        .cloned()
        .chain(
            audit
                .checklist
                .iter()
                .filter(|i| i.status == crate::models::AuditStatus::Fail)
                .map(|i| i.metric.clone()),
        )
        .take(6)
        .collect();

    let five_answers = [
        format!("Q1 Earnings: {}", earnings_picture.narrative),
        format!("Q2 Durability: {}", moat_assessment.narrative),
        format!("Q3 Capital: {}", capital_intensity.narrative),
        format!("Q4 Management: {}", management_trust.narrative),
        format!("Q5 Price vs value: {}", price_vs_value.narrative),
    ]
    .map(String::from)
    .to_vec();

    let headline = format!(
        "{} business · moat {:.0}/100 · trust {:.0}/100 · MOS {:.1}% — {}",
        tier_label_str(business_tier_label(tier)),
        moat_score,
        trust_score,
        mos.unwrap_or(0.0),
        price_vs_value.graham_buffett_read
    );

    let _ = stock;

    let cfo_pat_3y = cumulative_cfo_pat_for_bundle(bundle, 3).or(trust_inputs.cumulative_cfo_pat_3y);
    let score_reasons = build_score_reasons(
        financials,
        bundle,
        profile,
        margin_trend,
        cfo_pat_3y,
        &scores,
        moat_score,
        trust_inputs,
        price,
        fair_value,
        audit,
    );

    BuffettLensReport {
        scores,
        earnings_picture,
        moat_assessment,
        capital_intensity,
        management_trust,
        price_vs_value,
        accounting_skepticism_flags: accounting_flags,
        five_answers,
        headline_verdict: headline,
        philosophy_note: "Heuristic Berkshire-style lens from Yahoo/statement data. \
            Moat types, management character, and footnotes require manual filing review. Not investment advice."
            .to_string(),
        score_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CashflowRow, IncomeStatementRow, ManagementTrustInputs};

    fn base_financials() -> Financials {
        Financials {
            revenue: 10_000.0,
            net_income: 1_200.0,
            pe_ratio: 18.0,
            return_on_equity: 0.20,
            return_on_capital_employed: Some(0.22),
            profit_margins: 0.12,
            gross_margins: 0.45,
            operating_margins: 0.20,
            operating_cashflow: 1_400.0,
            free_cashflow: 1_000.0,
            debt_to_equity: 0.35,
            market_cap: 20_000.0,
            trailing_eps: 10.0,
            revenue_growth: 0.12,
            earnings_growth: 0.15,
            ..Default::default()
        }
    }

    fn compounder_bundle() -> StatementBundle {
        let rows: Vec<CashflowRow> = (0..3)
            .map(|i| CashflowRow {
                end_date_fmt: format!("202{}-03-31", 3 - i),
                end_ts: Some(1_700_000_000 - i as i64 * 31_536_000),
                operating_cashflow: 1_400.0,
                capital_expenditure: 200.0,
                free_cashflow: 1_200.0,
            })
            .collect();
        let inc: Vec<IncomeStatementRow> = (0..4)
            .map(|i| IncomeStatementRow {
                end_date_fmt: format!("202{}-03-31", 3 - i),
                end_ts: Some(1_700_000_000 - i as i64 * 31_536_000),
                revenue: 8_000.0 + i as f64 * 500.0,
                net_income: 900.0 + i as f64 * 100.0,
                ..Default::default()
            })
            .collect();
        StatementBundle {
            income_annual: inc,
            cashflow_annual: rows,
            ..Default::default()
        }
    }

    #[test]
    fn wide_moat_compounder_scores_high() {
        let f = base_financials();
        let b = compounder_bundle();
        let p = AssetProfile {
            sector: Some("Technology".into()),
            ..Default::default()
        };
        let scores =
            compute_buffett_lens_scores(&f, &b, &p, 180.0, &ManagementTrustInputs::default(), Some(20.0));
        assert!(scores.moat_score.unwrap_or(0.0) >= 65.0);
        assert!(scores.business_tier.unwrap_or(0.0) >= 3.0);
        assert!(scores.owner_earnings_ttm.unwrap_or(0.0) > 0.0);
    }

    #[test]
    fn value_trap_low_moat_and_margins() {
        let mut f = base_financials();
        f.profit_margins = 0.05;
        f.gross_margins = 0.18;
        f.return_on_equity = 0.06;
        f.return_on_capital_employed = Some(0.05);
        f.free_cashflow = -200.0;
        f.pe_ratio = 8.0;
        let b = compounder_bundle();
        let scores =
            compute_buffett_lens_scores(&f, &b, &AssetProfile::default(), 80.0, &ManagementTrustInputs::default(), Some(18.0));
        assert!(scores.moat_score.unwrap_or(100.0) < 50.0);
        assert!(scores.business_tier.unwrap_or(4.0) <= 2.0);
    }

    #[test]
    fn high_debt_reduces_trust() {
        let mut f = base_financials();
        f.debt_to_equity = 1.8;
        let inputs = ManagementTrustInputs {
            interest_coverage: Some(1.5),
            ..Default::default()
        };
        let scores = compute_buffett_lens_scores(
            &f,
            &compounder_bundle(),
            &AssetProfile::default(),
            180.0,
            &inputs,
            None,
        );
        assert!(scores.management_trust_score.unwrap_or(100.0) < 55.0);
    }

    #[test]
    fn business_tier_labels() {
        assert!(matches!(
            business_tier_label(4.0),
            BusinessTierLabel::Wonderful
        ));
        assert!(matches!(business_tier_label(1.0), BusinessTierLabel::Weak));
    }
}
