use crate::math::cagr;
use crate::models::{
    AnnualReport, CanonicalMetrics, CashFlowQuality, DataCoverage, FinancialCompanyType, Financials, FinancialStrengthAudit, ManagementAnalysis, MonitorableItem, PeerAnalysis,
    PeerBenchmark, PeerComparisonRow, PeerQuote, ReportInsights, RiskBuckets, RiskCategory, RiskItem,
    ScenarioAnalysis, ScoreBreakdown, SectorAnalysisDetail, StockAnalysis, StructuredResearchSections,
};
use crate::yahoo_metrics::{earnings_yield_eps_pct, fcf_yield_pct};

pub fn evaluate_quality(financials: &Financials) -> f64 {
    evaluate_quality_for(financials, FinancialCompanyType::Industrial)
}

pub fn evaluate_quality_for(financials: &Financials, company_type: FinancialCompanyType) -> f64 {
    let mut score: f64 = 50.0;

    if financials.profit_margins > 0.15 {
        score += 15.0;
    } else if financials.profit_margins > 0.05 {
        score += 5.0;
    } else if financials.profit_margins < 0.0 {
        score -= 10.0;
    }

    if let Some(roe) = financials.return_on_equity {
        if roe > 0.15 {
            score += 15.0;
        } else if roe < 0.05 {
            score -= 5.0;
        }
    }

    if company_type.is_lender() {
        // Cash, D/E, FCF, and Yahoo margins are not used as industrial liquidity/leverage screens.
        if let Some(roa) = financials.return_on_assets {
            if roa > 0.015 {
                score += 8.0;
            }
        }
        return score.clamp(0.0_f64, 100.0_f64);
    }

    if let Some(de) = financials.debt_to_equity {
        if de < 0.50 {
            score += 10.0;
        } else if de > 1.50 {
            score -= 10.0;
        }
    }

    if let Some(fcf) = financials.free_cashflow {
        if fcf > 0.0 {
            score += 10.0;
        }
    }

    score.clamp(0.0_f64, 100.0_f64)
}

/// P/E bucket plus 52-week price range context (Yahoo fields; not a price target).
pub fn evaluate_valuation(price: f64, financials: &Financials) -> String {
    let pe = financials.pe_ratio;
    let pe_label = if pe > 0.0 && pe < 15.0 {
        "P/E screen: toward lower/undervalued band"
    } else if pe >= 15.0 && pe <= 25.0 {
        "P/E screen: mid/typical band"
    } else if pe > 25.0 {
        "P/E screen: higher band"
    } else {
        "P/E: unavailable"
    };

    let range_note = {
        let lo = financials.fifty_two_week_low;
        let hi = financials.fifty_two_week_high;
        if price > 0.0 && hi > lo && hi > 0.0 {
            let span = hi - lo;
            if span > 0.0 {
                let pos = ((price - lo) / span).clamp(0.0, 1.0);
                if pos >= 0.9 {
                    Some("trading near 52W high (within Yahoo range)")
                } else if pos <= 0.15 {
                    Some("trading near 52W low (within Yahoo range)")
                } else {
                    Some("trading between 52W high and low")
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    match range_note {
        Some(r) => format!("{}. {}", pe_label, r),
        None => pe_label.to_string(),
    }
}

pub fn evaluate_management(officer_pay: f64, revenue: f64) -> f64 {
    if revenue <= 0.0 || !revenue.is_finite() {
        return 50.0;
    }
    if officer_pay <= 0.0 || !officer_pay.is_finite() {
        return 50.0;
    }
    let pay_ratio = officer_pay / revenue;
    // Low disclosed pay vs revenue scores higher; penalty scales smoothly (no 50 vs 86 cliff).
    let normalized = (pay_ratio / 0.02).clamp(0.0, 2.0);
    (62.0 + (1.0 - normalized) * 24.0).clamp(0.0, 100.0)
}

/// Heuristic sentiment on Yahoo `longBusinessSummary` — not audited MD&A.
pub fn tone_from_summary(text: Option<&str>) -> (f64, String) {
    let Some(t) = text.filter(|s| !s.is_empty()) else {
        return (50.0, "Neutral".to_string());
    };
    let lower = t.to_lowercase();
    let pos = [
        "growth", "leading", "strong", "expand", "opportunity", "innovation", "diverse", "robust",
        "improve", "profit", "customer", "sustainable",
    ];
    let neg = [
        "risk", "decline", "challenge", "uncertain", "litigation", "debt", "loss", "weak",
        "competition", "volatile", "adverse",
    ];
    let mut score: f64 = 50.0;
    for w in pos {
        if lower.contains(w) {
            score += 3.0;
        }
    }
    for w in neg {
        if lower.contains(w) {
            score -= 3.0;
        }
    }
    score = score.clamp(0.0, 100.0);
    let label = if score >= 62.0 {
        "Cautiously positive"
    } else if score <= 38.0 {
        "Cautiously negative"
    } else {
        "Neutral"
    };
    (score, label.to_string())
}

fn margin_trend_from_reports(reports: &[AnnualReport]) -> String {
    let mut sorted: Vec<&AnnualReport> = reports.iter().filter(|a| a.series_warning.is_none()).collect();
    if sorted.len() < 2 {
        return "Insufficient consistent history".to_string();
    }
    sorted.sort_by(|a, b| a.date.cmp(&b.date));
    let first = sorted[0];
    let last = sorted[sorted.len() - 1];
    let (Some(r0), Some(r1)) = (first.revenue, last.revenue) else {
        return "Insufficient consistent history".to_string();
    };
    if r0 <= 0.0 || r1 <= 0.0 {
        return "Insufficient consistent history".to_string();
    }
    let m0 = first.net_income / r0;
    let m1 = last.net_income / r1;
    if m1 > m0 + 0.01 {
        "Improving net margin vs oldest year in series".to_string()
    } else if m1 + 0.01 < m0 {
        "Compressing net margin vs oldest year in series".to_string()
    } else {
        "Stable net margin vs oldest year in series".to_string()
    }
}

fn fcf_and_earnings_yields(
    price: f64,
    financials: &Financials,
    statement_fcf: Option<f64>,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>, String) {
    let fcf = statement_fcf.or(financials.free_cashflow);
    let fcf_y = fcf_yield_pct(fcf, financials.market_cap);
    let earn_y = earnings_yield_eps_pct(price, financials.trailing_eps, financials.pe_ratio);
    let (price_in_range, dist_from_high) = {
        let lo = financials.fifty_two_week_low;
        let hi = financials.fifty_two_week_high;
        if price > 0.0 && hi > lo {
            let span = hi - lo;
            if span > 0.0 {
                let pos = ((price - lo) / span).clamp(0.0, 1.0);
                let d = if hi > 0.0 {
                    Some(((hi - price) / hi) * 100.0)
                } else {
                    None
                };
                (Some(pos), d)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    };
    let peg = {
        let pe = financials.pe_ratio;
        let g = financials.earnings_growth.unwrap_or(0.0);
        if pe > 0.0 && g > 0.0 {
            let g_pct = g * 100.0;
            if g_pct > 0.0 {
                let raw = pe / g_pct;
                format!(
                    "Heuristic P/E to Yahoo earnings-growth: {:.1} (P/E {:.1}, YoY growth {:.0}%); compare to ~1 for rule-of-thumb PEG — data is noisy, not a recommendation.",
                    raw, pe, g_pct
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    };
    (fcf_y, earn_y, price_in_range, dist_from_high, peg)
}

pub fn compute_stock_analysis(
    price: f64,
    financials: &Financials,
    annual_reports: &[AnnualReport],
    statement_fcf: Option<f64>,
    canonical: &CanonicalMetrics,
) -> StockAnalysis {
    compute_stock_analysis_for(price, financials, annual_reports, statement_fcf, canonical, FinancialCompanyType::Industrial)
}

pub fn compute_stock_analysis_for(
    price: f64,
    financials: &Financials,
    annual_reports: &[AnnualReport],
    statement_fcf: Option<f64>,
    canonical: &CanonicalMetrics,
    company_type: FinancialCompanyType,
) -> StockAnalysis {
    let data_quality_score = evaluate_quality_for(financials, company_type);
    let valuation_label = evaluate_valuation(price, financials);
    let margin_trend = if company_type.is_lender() {
        None
    } else {
        Some(margin_trend_from_reports(annual_reports))
    };
    let (fcf_yield_pct, earnings_yield_pct, price_in_52w_range, distance_from_52w_high_pct, peg_style_note) =
        if company_type.is_lender() {
            let earn_y = earnings_yield_eps_pct(price, financials.trailing_eps, financials.pe_ratio);
            (None, earn_y, None, None, String::new())
        } else {
            fcf_and_earnings_yields(price, financials, statement_fcf)
        };

    let (rev_full, ni_full) = {
        let mut sorted = annual_reports.to_vec();
        sorted.sort_by(|a, b| a.date.cmp(&b.date));
        let usable: Vec<&AnnualReport> = sorted.iter().filter(|a| a.series_warning.is_none()).collect();
        if usable.len() >= 2 {
            let first = usable[0];
            let last = usable[usable.len() - 1];
            let years = (usable.len().saturating_sub(1).max(1)) as f64;
            (
                match (first.revenue, last.revenue) {
                    (Some(a), Some(b)) => cagr(a, b, years),
                    _ => None,
                },
                cagr(first.net_income.max(1.0), last.net_income.max(1.0), years),
            )
        } else {
            (None, None)
        }
    };
    let yahoo_g = financials.revenue_growth.filter(|x| x.abs() > 1e-9);
    let rev_levels: Vec<f64> = {
        let mut s = annual_reports.to_vec();
        s.sort_by(|a, b| a.date.cmp(&b.date));
        s.iter().filter_map(|a| a.revenue).collect()
    };
    let ni_levels: Vec<f64> = {
        let mut s = annual_reports.to_vec();
        s.sort_by(|a, b| a.date.cmp(&b.date));
        s.iter().map(|a| a.net_income).collect()
    };
    let rev_3 = if company_type.is_lender() {
        None
    } else {
        canonical.revenue_cagr_3y_pct
    };
    let ni_3 = canonical.pat_cagr_3y_pct;
    let rev_5 = if company_type.is_lender() {
        None
    } else {
        crate::series_integrity::trailing_cagr_pct(&rev_levels, 5, yahoo_g).value
    };
    let ni_5 = crate::series_integrity::trailing_cagr_pct(&ni_levels, 5, None).value;
    let rev_full = if company_type.is_lender() { None } else { rev_full };
    let mut narrative = String::new();
    narrative.push_str(&format!(
        "Data quality (Yahoo snapshot) {:.0}/100 (not financial quality). {}. ",
        data_quality_score, valuation_label
    ));
    if !company_type.is_lender() {
        if let Some(g) = financials.revenue_growth.filter(|x| x.abs() > 1e-9) {
            narrative.push_str(&format!(
                "Yahoo current revenueGrowth {:.1}% (not FY annual YoY). ",
                g * 100.0
            ));
        }
        if let Some(fy) = canonical.fy_revenue_yoy_pct {
            narrative.push_str(&format!("FY annual revenue YoY {:.1}%. ", fy));
        }
        if let Some(r) = rev_3 {
            narrative.push_str(&format!("Statement 3Y revenue CAGR {:.1}% pa. ", r));
        }
    } else {
        if let Some(g) = canonical.interest_income_yoy_pct {
            narrative.push_str(&format!("Interest income YoY {:.1}%. ", g));
        }
        if let Some(g) = canonical.nii_yoy_pct {
            narrative.push_str(&format!("NII YoY {:.1}%. ", g));
        }
        if let Some(g) = canonical.yahoo_loan_book_growth_yoy_pct {
            narrative.push_str(&format!(
                "Yahoo loans/receivables YoY {:.1}% (row: {}; not verified as gross advances). ",
                g,
                if canonical.yahoo_loan_book_row.is_empty() {
                    "unspecified"
                } else {
                    canonical.yahoo_loan_book_row.as_str()
                }
            ));
        }
    }
    if let Some(g) = financials.earnings_growth.filter(|x| x.abs() > 1e-9) {
        narrative.push_str(&format!(
            "Yahoo current earningsGrowth {:.1}% (not FY PAT). ",
            g * 100.0
        ));
    }
    if let Some(fy) = canonical.fy_pat_yoy_pct {
        narrative.push_str(&format!("FY annual PAT YoY {:.1}%. ", fy));
    }
    if let Some(n) = ni_3 {
        narrative.push_str(&format!("Statement 3Y PAT CAGR {:.1}% pa. ", n));
    }
    if !company_type.is_lender() && rev_3.is_none() && ni_3.is_none() {
        if let (Some(r), Some(n)) = (rev_full, ni_full) {
            narrative.push_str(&format!(
                "Revenue / net income full-series CAGR (consistent suffix): {:.1}% / {:.1}% pa. ",
                r, n
            ));
        }
    }
    if let Some(fy) = fcf_yield_pct {
        narrative.push_str(&format!("FCF yield vs market cap (CFO − capex) ~{:.1}%. ", fy));
    }
    if company_type.is_lender() {
        narrative.push_str("FCF/CFO conversion is not used for this lender. ");
        if financials.price_to_book > 0.0 {
            narrative.push_str(&format!("P/B {:.2}x. ", financials.price_to_book));
        }
    }
    if let Some(mt) = &margin_trend {
        narrative.push_str(mt);
        narrative.push('.');
    }

    StockAnalysis {
        data_quality_score,
        quality_score_kind: if company_type.is_lender() {
            "Data quality (Yahoo snapshot) — lender; not financial quality".to_string()
        } else {
            "Data quality (Yahoo snapshot)".to_string()
        },
        valuation_label,
        revenue_cagr_full_series_pct: rev_full,
        net_income_cagr_full_series_pct: ni_full,
        revenue_cagr_trailing_3y_pct: rev_3,
        net_income_cagr_trailing_3y_pct: ni_3,
        revenue_cagr_trailing_5y_pct: rev_5,
        net_income_cagr_trailing_5y_pct: ni_5,
        margin_trend,
        narrative,
        fcf_yield_pct,
        earnings_yield_pct,
        peg_style_note,
        price_in_52w_range,
        distance_from_52w_high_pct,
    }
}

pub fn compute_report_insights(
    stock: &StockAnalysis,
    management: &ManagementAnalysis,
    peer: &PeerAnalysis,
    financials: &Financials,
    company_type: FinancialCompanyType,
    coverage: Option<&DataCoverage>,
) -> ReportInsights {
    let mut strengths: Vec<String> = Vec::new();
    let mut watch: Vec<String> = Vec::new();
    let mut data_notes: Vec<String> = Vec::new();
    let mut data_strengths: Vec<String> = Vec::new();

    if stock.data_quality_score >= 60.0 {
        data_strengths.push("Price, valuation and earnings history have usable Yahoo coverage.".to_string());
        data_notes.push("Yahoo snapshot quality heuristic is above our mid band (not a company strength).".to_string());
    } else if stock.data_quality_score < 40.0 {
        data_notes.push("Yahoo snapshot quality heuristic is on the low side; dig into drivers in filings.".to_string());
        watch.push("Yahoo snapshot quality heuristic is on the low side; dig into drivers (margins, ROE, debt, FCF).".to_string());
    }
    if let Some(de) = financials.debt_to_equity {
        if !company_type.is_lender() {
            if de > 1.00 {
                watch.push("Debt-to-equity (Yahoo) is elevated; review leverage vs peers and covenants.".to_string());
            } else if de < 0.30 {
                strengths.push("Debt-to-equity (Yahoo) is relatively light vs higher-leverage screen.".to_string());
            }
        }
    }
    if let Some(fy) = stock.fcf_yield_pct {
        if !company_type.is_lender() {
            if fy > 3.0 {
                strengths.push(format!("FCF yield to market cap ~{:.1}% (CFO − capex) — check sustainability vs capex and working capital.", fy));
            } else if fy < 0.0 {
                watch.push("Negative or weak FCF yield to market cap on last Yahoo print — triangulate with filings.".to_string());
            }
        }
    }
    if let Some(p) = peer.subject_percentile_roe {
        if p >= 65.0 {
            strengths.push(format!("ROE vs this peer set is in the top third (~{:.0}th percentile).", p));
        } else if p < 30.0 {
            watch.push(format!("ROE vs this peer set is in the lower band (~{:.0}th percentile).", p));
        }
    }
    if let Some(pay) = management.pay_vs_revenue_score {
        if pay < 45.0 {
            watch.push("Officer comp vs revenue screen is weak — verify in annual report / governance section.".to_string());
        } else if pay > 70.0 {
            strengths.push("Officer pay vs revenue (heuristic) is not a red flag in this pass.".to_string());
        }
    }
    if !company_type.is_lender() {
        if stock
            .margin_trend
            .as_deref()
            .map(|m| m.contains("Compressing"))
            .unwrap_or(false)
        {
            watch.push("Net margin is compressing vs oldest year in the Yahoo series.".to_string());
        } else if stock.margin_trend.as_deref().map(|m| m.contains("Improving")).unwrap_or(false) {
            strengths.push("Net margin trend in the series is positive vs oldest year (Yahoo history).".to_string());
        }
    }

    let pe_part = if financials.pe_ratio > 0.0 {
        format!("trailing/forward P/E (Yahoo) in the {:.0}x area", financials.pe_ratio)
    } else {
        "P/E is missing or N/A in the feed".to_string()
    };
    let cagr_part = if company_type.is_lender() {
        stock
            .net_income_cagr_trailing_3y_pct
            .map(|r| format!("Statement 3Y PAT CAGR {:.1}%. ", r))
            .unwrap_or_default()
    } else {
        stock
            .revenue_cagr_trailing_3y_pct
            .map(|r| format!("Statement 3Y revenue CAGR {:.1}%. ", r))
            .unwrap_or_default()
    };
    let executive_summary = format!(
        "Snapshot (heuristic, not investment advice). Data quality (Yahoo snapshot) {:.0}/100. {}{} Valuation: {}. Peer percentiles are vs a small fetched list, not a curated comp set. Confirm in SEBI/NSE documents.",
        stock.data_quality_score, cagr_part, pe_part, stock.valuation_label
    );
    let gated = coverage.map(|c| c.recommendation_gated).unwrap_or(false);
    let crit = coverage.map(|c| c.critical_pct).unwrap_or(100.0);
    if company_type.is_lender() {
        data_notes.push(
            "Asset quality, CRAR/CET1, NIM, CASA and official credit/deposit growth are typically in bank filings and earnings presentations — not in yfinance. Gating reflects Yahoo gaps, not missing company disclosure.".to_string(),
        );
    }
    if company_type.is_lender() && (gated || crit < 60.0) {
        watch.push("No adverse Yahoo-supported flags detected, but critical lender-risk coverage is insufficient.".to_string());
    } else if watch.is_empty() {
        watch.push("No major automated watch flags; still check liquidity, guidance, and one-offs.".to_string());
    }
    ReportInsights {
        executive_summary,
        strengths,
        watch_items: watch,
        data_notes,
        data_strengths,
    }
}

pub fn compute_management_analysis(
    officer_pay: f64,
    revenue: f64,
    summary: Option<&str>,
) -> ManagementAnalysis {
    compute_management_analysis_for(officer_pay, revenue, summary, FinancialCompanyType::Industrial)
}

pub fn compute_management_analysis_for(
    officer_pay: f64,
    revenue: f64,
    summary: Option<&str>,
    company_type: FinancialCompanyType,
) -> ManagementAnalysis {
    let (tone_score, tone_label) = tone_from_summary(summary);
    if company_type.is_lender() {
        return ManagementAnalysis {
            pay_vs_revenue_score: None,
            tone_score,
            tone_label: tone_label.clone(),
            narrative: format!(
                "Officer pay vs Yahoo revenue is not used for lenders (Yahoo totalRevenue is not a reliable income base). Public business summary tone: {} (score {:.0}/100). Management-quality points are withheld until governance data exists.",
                tone_label, tone_score
            ),
        };
    }
    let pay_vs_revenue_score = evaluate_management(officer_pay, revenue);
    let narrative = format!(
        "Officer pay vs revenue score {:.0}/100 (heuristic). \
         Public business summary tone: {} (score {:.0}/100). \
         This is not a substitute for reading SEBI filings.",
        pay_vs_revenue_score, tone_label, tone_score
    );
    ManagementAnalysis {
        pay_vs_revenue_score: Some(pay_vs_revenue_score),
        tone_score,
        tone_label,
        narrative,
    }
}

fn sector_themes_from_titles(titles: &[String]) -> String {
    if titles.is_empty() {
        return "No headlines to theme.".to_string();
    }
    let joined = titles.join(" ").to_lowercase();
    let mut hits: Vec<&'static str> = Vec::new();
    for (kw, label) in [
        ("earnings", "earnings & results"),
        ("guidance", "guidance"),
        ("rbi", "rates / policy"),
        ("acquisition", "M&A / deals"),
        ("regulat", "regulation / policy risk"),
        ("export", "exports & demand"),
        ("inflation", "inflation & costs"),
        ("volatil", "volatility & sentiment"),
    ] {
        if joined.contains(kw) {
            hits.push(label);
        }
    }
    if hits.is_empty() {
        "Sector headlines present; no keyword themes matched in this pass.".to_string()
    } else {
        format!("Heuristic read on sample headlines: {}.", hits.join(", "))
    }
}

pub fn compute_sector_analysis(
    sector: Option<&str>,
    industry: Option<&str>,
    sector_news: &[crate::models::NewsItem],
) -> SectorAnalysisDetail {
    compute_sector_analysis_for(sector, industry, sector_news, FinancialCompanyType::Industrial)
}

pub fn compute_sector_analysis_for(
    sector: Option<&str>,
    industry: Option<&str>,
    sector_news: &[crate::models::NewsItem],
    company_type: FinancialCompanyType,
) -> SectorAnalysisDetail {
    let outlook_narrative = if company_type.is_lender() {
        crate::financial_company::lender_sector_outlook(company_type, sector, industry)
    } else {
        match (sector.as_ref(), industry.as_ref()) {
            (Some(s), Some(i)) if !s.is_empty() && !i.is_empty() => {
                format!("Sector: {}. Industry: {}. Yahoo data only; treat as a starting point.", s, i)
            }
            (Some(s), _) if !s.is_empty() => format!("Sector: {}.", s),
            _ => "Sector metadata unavailable from data provider.".to_string(),
        }
    };
    let sector_news_summary = if sector_news.is_empty() {
        "No sector headlines returned.".to_string()
    } else {
        format!("{} headline(s) matched sector/industry topic search.", sector_news.len())
    };
    let sample_headlines: Vec<String> = sector_news
        .iter()
        .take(3)
        .map(|n| n.title.clone())
        .collect();
    let themes_titles: Vec<String> = sector_news.iter().take(8).map(|n| n.title.clone()).collect();
    let sector_headline_themes = sector_themes_from_titles(&themes_titles);
    SectorAnalysisDetail {
        sector: sector.map(String::from),
        industry: industry.map(String::from),
        outlook_narrative,
        sector_news_summary,
        sample_headlines,
        sector_headline_themes,
        research: None,
    }
}

fn percentile_among(subject: f64, peers: &[f64]) -> Option<f64> {
    if !subject.is_finite() {
        return None;
    }
    let mut vals: Vec<f64> = peers.iter().copied().filter(|x| x.is_finite()).collect();
    if vals.is_empty() {
        return None;
    }
    vals.push(subject);
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pos = vals.iter().position(|&x| (x - subject).abs() < f64::EPSILON * 1000.0)?;
    Some((pos as f64 / (vals.len().saturating_sub(1).max(1) as f64)) * 100.0)
}

fn financials_from_peer(p: &PeerQuote) -> Financials {
    Financials {
        revenue: p.revenue,
        pe_ratio: p.pe_ratio,
        profit_margins: p.profit_margins,
        return_on_equity: p.return_on_equity,
        debt_to_equity: p.debt_to_equity,
        free_cashflow: p.free_cashflow,
        market_cap: p.market_cap,
        ..Default::default()
    }
}

pub fn compute_peer_analysis(
    subject: &PeerQuote,
    peers: &[PeerQuote],
    industry: Option<&str>,
    summary: Option<&str>,
) -> PeerAnalysis {
    compute_peer_analysis_for(subject, peers, industry, summary, FinancialCompanyType::Industrial)
}

pub fn compute_peer_analysis_for(
    subject: &PeerQuote,
    peers: &[PeerQuote],
    industry: Option<&str>,
    summary: Option<&str>,
    company_type: FinancialCompanyType,
) -> PeerAnalysis {
    let pe_list: Vec<f64> = peers.iter().map(|p| p.pe_ratio).filter(|p| *p > 0.0).collect();
    let roe_list: Vec<f64> = peers.iter().filter_map(|p| p.return_on_equity).collect();
    let roe_coverage_known = roe_list.len()
        + usize::from(subject.return_on_equity.is_some());
    let roe_coverage_total = peers.len() + 1;
    let quality_list: Vec<f64> = peers
        .iter()
        .map(|p| evaluate_quality_for(&financials_from_peer(p), company_type))
        .collect();
    let mgmt_list: Vec<f64> = peers
        .iter()
        .map(|p| evaluate_management(p.officer_pay, p.revenue))
        .collect();

    let subject_data_quality_score = evaluate_quality_for(&financials_from_peer(subject), company_type);
    let subject_pay_vs_revenue_score = if company_type.is_lender() {
        None
    } else {
        Some(evaluate_management(subject.officer_pay, subject.revenue))
    };

    let subject_percentile_pe = if subject.pe_ratio > 0.0 {
        percentile_among(subject.pe_ratio, &pe_list)
    } else {
        None
    };
    let subject_percentile_roe = match subject.return_on_equity {
        Some(roe) if roe_list.len() >= 2 => percentile_among(roe, &roe_list),
        _ => None,
    };
    let subject_percentile_quality = percentile_among(subject_data_quality_score, &quality_list);
    let subject_percentile_pay_efficiency = if company_type.is_lender() {
        None
    } else {
        subject_pay_vs_revenue_score.and_then(|s| percentile_among(s, &mgmt_list))
    };

    let benchmarks: Vec<PeerBenchmark> = peers
        .iter()
        .map(|p| {
            let data_quality_score = evaluate_quality_for(&financials_from_peer(p), company_type);
            let pay_vs_revenue_score = if company_type.is_lender() {
                None
            } else {
                Some(evaluate_management(p.officer_pay, p.revenue))
            };
            let pay_to_revenue_pct = if company_type.is_lender() || p.revenue <= 0.0 {
                None
            } else {
                Some((p.officer_pay / p.revenue) * 100.0)
            };
            PeerBenchmark {
                symbol: p.symbol.clone(),
                short_name: p.short_name.clone(),
                data_quality_score,
                pay_vs_revenue_score,
                pay_to_revenue_pct,
            }
        })
        .collect();

    let mut narrative = String::new();
    let (comparability, direct_peer_comparability, bank_comparability) = if company_type.is_lender() {
        crate::financial_company::peer_comparability_for(company_type)
    } else {
        (
            crate::canonical::peer_comparability(industry, summary),
            String::new(),
            String::new(),
        )
    };
    if company_type == FinancialCompanyType::NbfcProjectFinance {
        narrative.push_str("Direct peer comparability is High for PFC / IREDA / IRFC / HUDCO. ");
    } else if company_type.is_lender() {
        narrative.push_str(&format!("{direct_peer_comparability}. "));
    } else if comparability == "low" {
        narrative.push_str(
            "Peer comparability is LOW: the subject may be a diversified business versus a narrow industry peer set; P/E premium vs oil/refining names is not treated as a valuation signal. ",
        );
    } else {
        narrative.push_str(&format!("Peer comparability: {}. ", comparability));
    }
    if let Some(p) = subject_percentile_pe {
        if comparability != "low" {
            narrative.push_str(&format!(
                "Trailing P/E is around the {:.0}th percentile vs fetched peers (0=lowest). ",
                p
            ));
        }
    }
    if let Some(r) = subject_percentile_roe {
        narrative.push_str(&format!(
            "ROE is around the {:.0}th percentile vs fetched peers (coverage {}/{}). ",
            r, roe_coverage_known, roe_coverage_total
        ));
    } else {
        narrative.push_str(&format!(
            "ROE percentile: N/A (coverage {}/{}). ",
            roe_coverage_known, roe_coverage_total
        ));
    }
    if let Some(q) = subject_percentile_quality {
        narrative.push_str(&format!(
            "Screening quality (Yahoo snapshot) is around the {:.0}th percentile vs peers. ",
            q
        ));
    }
    if let Some(m) = subject_percentile_pay_efficiency {
        narrative.push_str(&format!(
            "Management pay efficiency score is around the {:.0}th percentile vs peers. ",
            m
        ));
    }
    if narrative.is_empty() {
        narrative = "Peer sample too small or incomplete for percentile ranks.".to_string();
    }

    PeerAnalysis {
        peers: peers.to_vec(),
        benchmarks,
        subject_data_quality_score,
        subject_pay_vs_revenue_score,
        subject_percentile_pe,
        subject_percentile_roe,
        subject_percentile_quality,
        subject_percentile_pay_efficiency,
        roe_coverage_known,
        roe_coverage_total,
        narrative,
        peer_comparability: comparability,
        direct_peer_comparability,
        bank_comparability,
        peer_set_kind: String::new(),
    }
}

fn clamp_score(v: f64, max: f64) -> f64 {
    v.max(0.0).min(max)
}

pub fn compute_cash_flow_quality(
    financials: &Financials,
    canonical: &CanonicalMetrics,
    bundle: Option<&crate::models::StatementBundle>,
) -> CashFlowQuality {
    if canonical.industrial_metrics_suppressed {
        return CashFlowQuality {
            pat: canonical.pat,
            cfo: None,
            ebitda: 0.0,
            free_cashflow: None,
            capex_estimate: None,
            pat_vs_cfo_delta: None,
            cfo_vs_ebitda_ratio: None,
            cash_conversion_ratio: None,
            capex_requirement_ratio: None,
            cumulative_cfo_pat_3y: None,
            cumulative_cfo_pat_5y: None,
            narrative: "CFO/PAT and FCF are not used for lending companies. Loan-book growth and asset quality replace cash-conversion screens.".to_string(),
        };
    }
    let pat = canonical.pat;
    let cfo = canonical.cfo;
    let ebitda = financials.ebitda;
    let free_cashflow = canonical.fcf;
    let capex_estimate = canonical.capex;
    let pat_vs_cfo_delta = match (pat, cfo) {
        (Some(p), Some(c)) => Some(c - p),
        _ => None,
    };
    let cfo_vs_ebitda_ratio = match (cfo, ebitda) {
        (Some(c), e) if e > 0.0 => Some(c / e),
        _ => None,
    };
    let cash_conversion_ratio = match (pat, cfo) {
        (Some(p), Some(c)) if p > 0.0 => Some(c / p),
        _ => None,
    };
    let capex_requirement_ratio = match (cfo, capex_estimate) {
        (Some(c), Some(x)) if c > 0.0 => Some(x / c),
        _ => None,
    };

    let (cumulative_cfo_pat_3y, cumulative_cfo_pat_5y) = bundle
        .map(|b| {
            (
                crate::financial_strength_audit::cumulative_cfo_pat_for_bundle(b, 3),
                crate::financial_strength_audit::cumulative_cfo_pat_for_bundle(b, 5),
            )
        })
        .unwrap_or((None, None));

    let mut narrative = String::new();
    if let Some(r) = cash_conversion_ratio {
        if r >= 1.0 {
            narrative.push_str("CFO covers PAT (cash conversion >= 1x). ");
        } else if r > 0.0 {
            narrative.push_str("CFO trails PAT (cash conversion < 1x). ");
        }
    }
    if let Some(r) = cumulative_cfo_pat_3y {
        narrative.push_str(&format!("3Y cumulative CFO/PAT is {:.2}x. ", r));
    }
    if let Some(r) = cfo_vs_ebitda_ratio {
        narrative.push_str(&format!("CFO/EBITDA is {:.2}x. ", r));
    }
    if let Some(r) = capex_requirement_ratio {
        narrative.push_str(&format!("Estimated capex consumes about {:.0}% of CFO. ", r * 100.0));
    }
    if narrative.is_empty() {
        narrative = "Insufficient data to evaluate cash-flow quality ratios.".to_string();
    }

    CashFlowQuality {
        pat,
        cfo,
        ebitda,
        free_cashflow,
        capex_estimate,
        pat_vs_cfo_delta,
        cfo_vs_ebitda_ratio,
        cash_conversion_ratio,
        capex_requirement_ratio,
        cumulative_cfo_pat_3y,
        cumulative_cfo_pat_5y,
        narrative,
    }
}

pub fn build_peer_comparison_table(subject: &PeerQuote, peers: &[PeerQuote]) -> Vec<PeerComparisonRow> {
    build_peer_comparison_table_for(subject, peers, FinancialCompanyType::Industrial)
}

pub fn build_peer_comparison_table_for(
    subject: &PeerQuote,
    peers: &[PeerQuote],
    company_type: FinancialCompanyType,
) -> Vec<PeerComparisonRow> {
    let mut picked: Vec<&PeerQuote> = peers.iter().take(3).collect();
    while picked.len() < 3 {
        picked.push(subject);
    }
    let p1 = picked[0];
    let p2 = picked[1];
    let p3 = picked[2];
    let label_for = |q: &PeerQuote| {
        q.short_name
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&q.symbol)
            .to_string()
    };
    let company_label = label_for(subject);
    let p1_label = label_for(p1);
    let p2_label = label_for(p2);
    let p3_label = label_for(p3);
    let row = |metric: &str, company: Option<f64>, a: Option<f64>, b: Option<f64>, c: Option<f64>| PeerComparisonRow {
        metric: metric.to_string(),
        company_label: company_label.clone(),
        peer_1_label: p1_label.clone(),
        peer_2_label: p2_label.clone(),
        peer_3_label: p3_label.clone(),
        company,
        peer_1: a,
        peer_2: b,
        peer_3: c,
    };
    let g = |v: Option<f64>| v.map(|x| x * 100.0);
    let pe = |v: f64| (v > 0.0).then_some(v);
    let pb = |q: &PeerQuote| (q.price_to_book > 0.0).then_some(q.price_to_book);
    let dy = |q: &PeerQuote| (q.dividend_yield > 0.0).then_some(q.dividend_yield * 100.0);
    if company_type.is_lender() {
        return vec![
            row("P/E", pe(subject.pe_ratio), pe(p1.pe_ratio), pe(p2.pe_ratio), pe(p3.pe_ratio)),
            row("P/B", pb(subject), pb(p1), pb(p2), pb(p3)),
            row(
                "ROE",
                subject.return_on_equity.map(|v| v * 100.0),
                p1.return_on_equity.map(|v| v * 100.0),
                p2.return_on_equity.map(|v| v * 100.0),
                p3.return_on_equity.map(|v| v * 100.0),
            ),
            row(
                "ROA",
                subject.return_on_assets.map(|v| v * 100.0),
                p1.return_on_assets.map(|v| v * 100.0),
                p2.return_on_assets.map(|v| v * 100.0),
                p3.return_on_assets.map(|v| v * 100.0),
            ),
            row("NIM", None, None, None, None),
            row("GNPA / NNPA", None, None, None, None),
            row("CRAR", None, None, None, None),
            row("Loan book growth", None, None, None, None),
            row(
                "Yahoo current earningsGrowth (not FY PAT)",
                g(subject.pat_growth),
                g(p1.pat_growth),
                g(p2.pat_growth),
                g(p3.pat_growth),
            ),
            row("Book-value growth", None, None, None, None),
            row("Dividend yield", dy(subject), dy(p1), dy(p2), dy(p3)),
        ];
    }
    vec![
        row(
            "Revenue growth",
            g(subject.revenue_growth),
            g(p1.revenue_growth),
            g(p2.revenue_growth),
            g(p3.revenue_growth),
        ),
        row(
            "EBITDA margin",
            g(subject.ebitda_margin),
            g(p1.ebitda_margin),
            g(p2.ebitda_margin),
            g(p3.ebitda_margin),
        ),
        row(
            "Yahoo current earningsGrowth (not FY PAT)",
            g(subject.pat_growth),
            g(p1.pat_growth),
            g(p2.pat_growth),
            g(p3.pat_growth),
        ),
        row(
            "ROE",
            subject.return_on_equity.map(|v| v * 100.0),
            p1.return_on_equity.map(|v| v * 100.0),
            p2.return_on_equity.map(|v| v * 100.0),
            p3.return_on_equity.map(|v| v * 100.0),
        ),
        row(
            "ROCE",
            subject.return_on_capital_employed.map(|v| v * 100.0),
            p1.return_on_capital_employed.map(|v| v * 100.0),
            p2.return_on_capital_employed.map(|v| v * 100.0),
            p3.return_on_capital_employed.map(|v| v * 100.0),
        ),
        row(
            "Debt/equity",
            subject.debt_to_equity,
            p1.debt_to_equity,
            p2.debt_to_equity,
            p3.debt_to_equity,
        ),
        row("P/E", pe(subject.pe_ratio), pe(p1.pe_ratio), pe(p2.pe_ratio), pe(p3.pe_ratio)),
        row(
            "EV/EBITDA (Yahoo stats if present, else quote cash)",
            subject.ev_to_ebitda,
            p1.ev_to_ebitda,
            p2.ev_to_ebitda,
            p3.ev_to_ebitda,
        ),
    ]
}

pub fn categorize_risks(
    financials: &Financials,
    stock: &StockAnalysis,
    shareholders: &crate::models::Shareholders,
    audit: Option<&crate::models::FinancialStrengthAudit>,
    company_type: FinancialCompanyType,
) -> RiskBuckets {
    if company_type.is_lender() {
        return lender_risks(company_type, financials, shareholders);
    }
    let mut business_risks = vec![
        RiskItem { category: RiskCategory::Business, risk: "Demand slowdown".to_string(), severity: "Medium".to_string(), note: "Track volume growth and order trends quarterly.".to_string() },
        RiskItem { category: RiskCategory::Business, risk: "Technology disruption".to_string(), severity: "Medium".to_string(), note: "Watch product mix and R&D response.".to_string() },
        RiskItem { category: RiskCategory::Business, risk: "Cyclicality".to_string(), severity: "Medium".to_string(), note: "Margin and utilization can reverse in downturns.".to_string() },
    ];
    if financials.profit_margins < 0.08 {
        business_risks.push(RiskItem { category: RiskCategory::Business, risk: "Weak entry barriers".to_string(), severity: "High".to_string(), note: "Low margin profile can signal pricing pressure.".to_string() });
    }

    let mut financial_risks = vec![
        RiskItem { category: RiskCategory::Financial, risk: "Working capital stress".to_string(), severity: "Medium".to_string(), note: "Cross-check receivable/inventory days in filings.".to_string() },
    ];
    if financials.debt_to_equity.map(|d| d > 1.00).unwrap_or(false)
        && audit.map(|a| !a.checklist.iter().any(|i| i.metric.contains("GNPA") || i.metric.contains("Stage 3"))).unwrap_or(true)
    {
        financial_risks.push(RiskItem { category: RiskCategory::Financial, risk: "High debt".to_string(), severity: "High".to_string(), note: "Debt/equity is elevated versus comfort zone.".to_string() });
    }
    let statement_cfo_strong = match (financials.net_income, financials.operating_cashflow) {
        (Some(ni), Some(cfo)) if ni > 0.0 => cfo / ni >= 1.0,
        _ => false,
    };
    if let Some(fcf) = financials.free_cashflow {
        if fcf <= 0.0 && !statement_cfo_strong
            && audit.map(|a| !a.checklist.iter().any(|i| i.metric.contains("GNPA") || i.metric.contains("Stage 3"))).unwrap_or(true)
        {
            financial_risks.push(RiskItem { category: RiskCategory::Financial, risk: "Poor cash flow".to_string(), severity: "High".to_string(), note: "Free cash flow is weak/negative on latest print.".to_string() });
        }
    }
    if let Some(a) = audit {
        for flag in a.red_flags.iter().take(4) {
            financial_risks.push(RiskItem {
                category: RiskCategory::Financial,
                risk: flag.chars().take(60).collect(),
                severity: "High".to_string(),
                note: flag.clone(),
            });
        }
        if a.earnings_quality_score.map(|s| s < 40.0).unwrap_or(false) && !a.scores_provisional {
            financial_risks.push(RiskItem {
                category: RiskCategory::Financial,
                risk: "Earnings quality audit failed".to_string(),
                severity: "High".to_string(),
                note: format!(
                    "Earnings quality score {:.0}/100 — CFO/PAT and working capital checks weak.",
                    a.earnings_quality_score.unwrap_or(0.0)
                ),
            });
        }
    }

    let mut management_risks = vec![
        RiskItem { category: RiskCategory::Management, risk: "Related-party deals".to_string(), severity: "Medium".to_string(), note: "Read notes and auditor comments.".to_string() },
        RiskItem { category: RiskCategory::Management, risk: "Capital misallocation".to_string(), severity: "Medium".to_string(), note: "Track M&A/payback quality and buyback discipline.".to_string() },
    ];
    if shareholders.pledge_percent.unwrap_or(0.0) > 0.0 {
        management_risks.push(RiskItem { category: RiskCategory::Management, risk: "Pledge".to_string(), severity: "High".to_string(), note: "Promoter pledge reported; monitor trend as red flag.".to_string() });
    }

    let mut valuation_risks = vec![
        RiskItem { category: RiskCategory::Valuation, risk: "Growth already priced in".to_string(), severity: "Medium".to_string(), note: "Compare multiple vs growth sustainability.".to_string() },
    ];
    if financials.pe_ratio > 35.0 {
        valuation_risks.push(RiskItem { category: RiskCategory::Valuation, risk: "Overvaluation".to_string(), severity: "High".to_string(), note: "High P/E raises de-rating risk if growth misses.".to_string() });
    } else {
        valuation_risks.push(RiskItem { category: RiskCategory::Valuation, risk: "Multiple derating".to_string(), severity: "Medium".to_string(), note: "Even fair valuations can de-rate in weak cycles.".to_string() });
    }

    let regulatory_risks = vec![
        RiskItem { category: RiskCategory::Regulatory, risk: "Policy change".to_string(), severity: "Medium".to_string(), note: "Track sector policy and compliance updates.".to_string() },
        RiskItem { category: RiskCategory::Regulatory, risk: "Taxation change".to_string(), severity: "Medium".to_string(), note: "Watch effective tax-rate volatility.".to_string() },
        RiskItem { category: RiskCategory::Regulatory, risk: "Licensing issues".to_string(), severity: "Low".to_string(), note: "Relevant for regulated industries.".to_string() },
        RiskItem { category: RiskCategory::Regulatory, risk: "Price controls".to_string(), severity: "Low".to_string(), note: "Can compress realized pricing power.".to_string() },
    ];

    let _ = stock;
    RiskBuckets {
        business_risks,
        financial_risks,
        management_risks,
        valuation_risks,
        regulatory_risks,
    }
}

fn lender_risks(
    company_type: FinancialCompanyType,
    financials: &Financials,
    shareholders: &crate::models::Shareholders,
) -> RiskBuckets {
    if company_type.is_bank() {
        let mut management_risks = vec![
            RiskItem { category: RiskCategory::Management, risk: "Credit underwriting and collections".to_string(), severity: "Medium".to_string(), note: "Retail unsecured and SME books can move credit cost quickly.".to_string() },
            RiskItem { category: RiskCategory::Management, risk: "Deposit franchise execution".to_string(), severity: "Medium".to_string(), note: "CASA, term mix, and LDR vs wholesale funding.".to_string() },
        ];
        if shareholders.pledge_percent.unwrap_or(0.0) > 0.0 {
            management_risks.push(RiskItem { category: RiskCategory::Management, risk: "Pledge".to_string(), severity: "High".to_string(), note: "Promoter pledge reported; monitor trend.".to_string() });
        }
        let mut valuation_risks = vec![
            RiskItem { category: RiskCategory::Valuation, risk: "P/B vs ROE de-rating".to_string(), severity: "Medium".to_string(), note: "Bank multiples compress if ROE or asset quality slips.".to_string() },
        ];
        if financials.pe_ratio > 20.0 {
            valuation_risks.push(RiskItem { category: RiskCategory::Valuation, risk: "Rich P/E for a bank".to_string(), severity: "Medium".to_string(), note: "High P/E is secondary to P/B–ROE and asset quality.".to_string() });
        }
        return RiskBuckets {
            business_risks: vec![
                RiskItem { category: RiskCategory::Business, risk: "Credit cycle / unsecured retail".to_string(), severity: "High".to_string(), note: "Unsecured and SME mix vs secured retail/corporate.".to_string() },
                RiskItem { category: RiskCategory::Business, risk: "Deposit competition".to_string(), severity: "High".to_string(), note: "Term-deposit pricing and CASA share vs peers.".to_string() },
                RiskItem { category: RiskCategory::Business, risk: "Wholesale / corporate concentration".to_string(), severity: "Medium".to_string(), note: "Large-ticket corporate and group exposures.".to_string() },
            ],
            financial_risks: vec![
                RiskItem { category: RiskCategory::Financial, risk: "NIM compression".to_string(), severity: "High".to_string(), note: "Loan yields vs cost of deposits; verify NIM in filings.".to_string() },
                RiskItem { category: RiskCategory::Financial, risk: "Credit-cost spike".to_string(), severity: "High".to_string(), note: "GNPA/NNPA, slippages, and credit cost — usually missing from Yahoo.".to_string() },
                RiskItem { category: RiskCategory::Financial, risk: "Liquidity / LDR".to_string(), severity: "High".to_string(), note: "Loan/deposit ratio, LCR, and wholesale funding.".to_string() },
                RiskItem { category: RiskCategory::Financial, risk: "Regulatory capital changes".to_string(), severity: "Medium".to_string(), note: "CET1 / CRAR vs RBI Basel and D-SIB buffers.".to_string() },
            ],
            management_risks,
            valuation_risks,
            regulatory_risks: vec![
                RiskItem { category: RiskCategory::Regulatory, risk: "RBI banking regulation".to_string(), severity: "Medium".to_string(), note: "Capital, LCR, provisioning, and digital lending rules.".to_string() },
            ],
        };
    }
    let project = company_type == FinancialCompanyType::NbfcProjectFinance;
    let mut business_risks = vec![
        RiskItem {
            category: RiskCategory::Business,
            risk: if project { "Power-sector concentration".to_string() } else { "Sector concentration".to_string() },
            severity: "High".to_string(),
            note: "Track borrower mix vs power, infrastructure, and other books in filings.".to_string(),
        },
        RiskItem {
            category: RiskCategory::Business,
            risk: if project { "State-discom counterparty risk".to_string() } else { "Public-sector / utility counterparty risk".to_string() },
            severity: "High".to_string(),
            note: "State utility health and payment delays drive slippages for project financiers.".to_string(),
        },
        RiskItem {
            category: RiskCategory::Business,
            risk: "Private-sector credit risk".to_string(),
            severity: "Medium".to_string(),
            note: "Private borrower mix and large-project underwriting can move credit cost.".to_string(),
        },
        RiskItem {
            category: RiskCategory::Business,
            risk: "Large-project concentration".to_string(),
            severity: "Medium".to_string(),
            note: "Single-name / project exposures vs book size.".to_string(),
        },
    ];
    if project {
        business_risks.push(RiskItem {
            category: RiskCategory::Business,
            risk: "Renewables underwriting risk".to_string(),
            severity: "Medium".to_string(),
            note: "Tariff, offtake, and execution risk on RE exposures.".to_string(),
        });
    }
    let financial_risks = vec![
        RiskItem { category: RiskCategory::Financial, risk: "Funding-cost increase".to_string(), severity: "High".to_string(), note: "Bond yields and borrowing mix vs loan yields.".to_string() },
        RiskItem { category: RiskCategory::Financial, risk: "NIM compression".to_string(), severity: "High".to_string(), note: "Spread vs cost of funds; verify NIM in filings.".to_string() },
        RiskItem { category: RiskCategory::Financial, risk: "ALM mismatch".to_string(), severity: "High".to_string(), note: "Asset-liability duration and refinance risk.".to_string() },
        RiskItem { category: RiskCategory::Financial, risk: "Credit-cost spike".to_string(), severity: "High".to_string(), note: "GNPA/NNPA, Stage 3, and credit cost — usually missing from Yahoo.".to_string() },
        RiskItem { category: RiskCategory::Financial, risk: "Regulatory capital changes".to_string(), severity: "Medium".to_string(), note: "CRAR / Tier-I vs RBI NBFC rules.".to_string() },
    ];
    let mut management_risks = vec![
        RiskItem { category: RiskCategory::Management, risk: "Government ownership / policy direction".to_string(), severity: "Medium".to_string(), note: "SOE lenders can reprice or redirect books with policy.".to_string() },
        RiskItem { category: RiskCategory::Management, risk: "Borrowing-mix / ALM execution".to_string(), severity: "Medium".to_string(), note: "Bank lines vs bonds vs CP; refinancing calendar.".to_string() },
    ];
    if shareholders.pledge_percent.unwrap_or(0.0) > 0.0 {
        management_risks.push(RiskItem { category: RiskCategory::Management, risk: "Pledge".to_string(), severity: "High".to_string(), note: "Promoter pledge reported; monitor trend.".to_string() });
    }
    let mut valuation_risks = vec![
        RiskItem { category: RiskCategory::Valuation, risk: "P/B vs ROE de-rating".to_string(), severity: "Medium".to_string(), note: "Lender multiples compress if ROE or asset quality slips.".to_string() },
    ];
    if financials.pe_ratio > 20.0 {
        valuation_risks.push(RiskItem { category: RiskCategory::Valuation, risk: "Rich P/E for a lender".to_string(), severity: "Medium".to_string(), note: "High P/E is secondary to P/B–ROE for NBFCs.".to_string() });
    }
    let regulatory_risks = vec![
        RiskItem { category: RiskCategory::Regulatory, risk: "RBI / scale-based NBFC rules".to_string(), severity: "Medium".to_string(), note: "Capital, provisioning, and liquidity norms.".to_string() },
        RiskItem { category: RiskCategory::Regulatory, risk: "Sector policy (power / infra)".to_string(), severity: "Medium".to_string(), note: "Discom reforms, RE policy, and government capex.".to_string() },
    ];
    RiskBuckets {
        business_risks,
        financial_risks,
        management_risks,
        valuation_risks,
        regulatory_risks,
    }
}

fn industrial_business_quality(financials: &Financials) -> Option<f64> {
    let roe_pts = financials.return_on_equity.map(|r| {
        let pct = if r.abs() <= 1.5 { r * 100.0 } else { r };
        (pct.min(20.0) / 20.0) * 10.0
    });
    let margin_pts = (financials.profit_margins.abs() > 1e-9).then_some(
        (financials.profit_margins.max(0.0).min(0.20) / 0.20) * 10.0,
    );
    match (roe_pts, margin_pts) {
        (Some(a), Some(b)) => Some(clamp_score(a + b, 20.0)),
        (Some(a), None) => Some(clamp_score(a, 20.0)),
        (None, Some(b)) => Some(clamp_score(b, 20.0)),
        _ => None,
    }
}

fn yoy_to_growth_pts(yoy_pct: f64) -> f64 {
    clamp_score(4.0 + yoy_pct * 0.4, 10.0)
}

fn statement_first_growth_triggers(
    canonical: &CanonicalMetrics,
    financials: &Financials,
    company_type: FinancialCompanyType,
) -> (Option<f64>, String) {
    let mut labeled: Vec<(&str, f64)> = Vec::new();
    if company_type.is_lender() {
        if let Some(g) = canonical.nii_yoy_pct {
            labeled.push(("NII YoY", g));
        }
        if let Some(g) = canonical.fy_pat_yoy_pct {
            labeled.push(("FY PAT YoY", g));
        }
        if let Some(g) = canonical.pat_cagr_3y_pct {
            labeled.push(("3Y PAT CAGR", g));
        }
        if let Some(g) = canonical.interest_income_yoy_pct {
            labeled.push(("interest income YoY", g));
        }
    } else {
        if let Some(g) = canonical.fy_pat_yoy_pct {
            labeled.push(("FY PAT YoY", g));
        }
        if let Some(g) = canonical.fy_revenue_yoy_pct {
            labeled.push(("FY revenue YoY", g));
        }
        if let Some(g) = canonical.pat_cagr_3y_pct {
            labeled.push(("3Y PAT CAGR", g));
        }
    }
    let eg_pct = financials.earnings_growth.map(|g| g * 100.0);
    if labeled.is_empty() {
        return match eg_pct {
            Some(g) => (
                Some(clamp_score((g.max(0.0) / 2.0) * 0.5, 10.0)),
                format!(
                    "growth_triggers from Yahoo earningsGrowth {:.1}% only (half weight; no statement YoY/CAGR) — not the composite growth_score baseline",
                    g
                ),
            ),
            None => (
                None,
                "growth_triggers excluded — no statement NII/PAT growth and no Yahoo earningsGrowth".to_string(),
            ),
        };
    }
    let stmt_avg: f64 = labeled.iter().map(|(_, g)| yoy_to_growth_pts(*g)).sum::<f64>()
        / labeled.len() as f64;
    let score = if let Some(g) = eg_pct {
        let secondary = clamp_score((g.max(0.0) / 2.0) * 0.5, 10.0);
        clamp_score(stmt_avg * 0.85 + secondary * 0.15, 10.0)
    } else {
        clamp_score(stmt_avg, 10.0)
    };
    let detail = labeled
        .iter()
        .map(|(n, g)| format!("{n} {g:+.1}%"))
        .collect::<Vec<_>>()
        .join(", ");
    let eg_bit = eg_pct
        .map(|g| format!("; Yahoo earningsGrowth {g:+.1}% is secondary"))
        .unwrap_or_else(|| "; Yahoo earningsGrowth unused".to_string());
    (
        Some(score),
        format!("growth_triggers {:.1}/10 from statement {detail}{eg_bit}", score),
    )
}

pub fn compute_weighted_score(
    financials: &Financials,
    stock: &StockAnalysis,
    management: &ManagementAnalysis,
    _risks: &RiskBuckets,
    company_type: FinancialCompanyType,
    audit: &FinancialStrengthAudit,
    canonical: &CanonicalMetrics,
) -> ScoreBreakdown {
    let business_quality = if company_type.is_lender() {
        None
    } else {
        industrial_business_quality(financials)
    };
    let industry_tailwind = if company_type.is_lender() {
        None
    } else {
        financials.revenue_growth.map(|g| {
            clamp_score(9.0 + g * 20.0, 15.0)
        })
    };
    let financial_strength = if company_type.is_lender()
        && (audit.scores_provisional || audit.overall_strength_score.is_none())
    {
        None
    } else {
        Some(clamp_score(
            {
                let roe_part = financials
                    .return_on_equity
                    .map(|r| ((r * 100.0).min(20.0) / 20.0) * 12.0)
                    .unwrap_or(6.0);
                let de_part = if company_type.is_lender() {
                    0.0
                } else {
                    match financials.debt_to_equity {
                        Some(d) if d < 0.60 => 8.0,
                        Some(_) => 4.0,
                        None => 6.0,
                    }
                };
                roe_part + de_part
            },
            20.0,
        ))
    };
    let management_quality = if company_type.is_lender() {
        None
    } else {
        management.pay_vs_revenue_score.map(|p| clamp_score((p / 100.0) * 15.0, 15.0))
    };
    let valuation_comfort = clamp_score(
        if financials.pe_ratio <= 0.0 {
            7.0
        } else if financials.pe_ratio < 18.0 {
            14.0
        } else if financials.pe_ratio < 25.0 {
            11.0
        } else {
            7.0
        },
        15.0,
    );
    let (growth_triggers, growth_provenance) =
        statement_first_growth_triggers(canonical, financials, company_type);
    let gated = audit.data_coverage.recommendation_gated && company_type.is_lender();
    let credit_unassessed =
        crate::financial_company::lender_fundamental_risk_unassessed(&audit.data_coverage, company_type);
    let risk_reward = None;

    let mut known_pts = valuation_comfort;
    let mut known_max = 15.0;
    if let Some(bq) = business_quality {
        known_pts += bq;
        known_max += 20.0;
    }
    if let Some(it) = industry_tailwind {
        known_pts += it;
        known_max += 15.0;
    }
    if let Some(g) = growth_triggers {
        known_pts += g;
        known_max += 10.0;
    }
    if let Some(rr) = risk_reward {
        known_pts += rr;
        known_max += 5.0;
    }
    if let Some(fs) = financial_strength {
        known_pts += fs;
        known_max += 20.0;
    }
    if let Some(mq) = management_quality {
        known_pts += mq;
        known_max += 15.0;
    }
    let available_dimension_score = if known_max > 0.0 {
        Some((known_pts / known_max) * 100.0)
    } else {
        None
    };
    let crit = if company_type.is_lender() {
        Some(audit.data_coverage.critical_pct)
    } else {
        None
    };
    let coverage_adjusted_score = if gated {
        None
    } else {
        available_dimension_score
    };
    let total_score = if gated {
        None
    } else {
        available_dimension_score
    };
    let screening_score = known_pts;
    let interpretation = if gated {
        format!(
            "Available-metric score {:.1}/100. Critical coverage {:.0}%. Investment rating withheld.",
            available_dimension_score.unwrap_or(0.0),
            audit.data_coverage.critical_pct
        )
    } else if available_dimension_score.unwrap_or(0.0) >= 80.0 {
        "High-quality candidate".to_string()
    } else if available_dimension_score.unwrap_or(0.0) >= 65.0 {
        "Good, but check valuation/risk".to_string()
    } else if available_dimension_score.unwrap_or(0.0) >= 50.0 {
        "Watchlist only".to_string()
    } else {
        "Avoid unless special situation".to_string()
    };
    let mut score_provenance = vec![
        match business_quality {
            Some(s) => format!(
                "business_quality {:.1}/20 from ROE and profit margin (not Yahoo data completeness)",
                s
            ),
            None if company_type.is_lender() => {
                "business_quality excluded — needs bank economics (ROA/ROE, deposit franchise, NIM, asset quality, cost efficiency, capital), not data_quality_score".to_string()
            }
            None => "business_quality excluded — ROE/margins missing (not scored from data completeness)".to_string(),
        },
        match industry_tailwind {
            Some(s) => format!(
                "industry_tailwind {:.1}/15 from Yahoo revenueGrowth (observed, not a template)",
                s
            ),
            None if company_type.is_lender() => {
                "industry_tailwind excluded — no observed credit-cycle / rate / liquidity series (lender template 9/15 is not used)".to_string()
            }
            None => "industry_tailwind excluded — Yahoo revenueGrowth missing".to_string(),
        },
        match financial_strength {
            Some(s) => format!("financial_strength {:.1}/20 from ROE and (non-lender) D/E", s),
            None => "financial_strength excluded — gated / insufficient critical lender data".to_string(),
        },
        match management_quality {
            Some(s) => format!("management_quality {:.1}/15 from pay-vs-revenue", s),
            None => "management_quality excluded — no governance score (lenders / missing pay data)".to_string(),
        },
        format!(
            "valuation_comfort {:.1}/15 from trailing P/E bands (P/E {:.1})",
            valuation_comfort, financials.pe_ratio
        ),
        growth_provenance,
        match risk_reward {
            Some(s) => format!("risk_reward {:.1}/5 from measured risk indicators", s),
            None if credit_unassessed => {
                "risk_reward excluded — asset quality and/or capital adequacy missing; not scored from risk-template item counts or market beta".to_string()
            }
            None => {
                "risk_reward excluded — not scored from predefined risk-template item counts; needs measured risk indicators".to_string()
            }
        },
        format!(
            "screening_score {:.1} = sum of included columns; available_dimension_score = included/max_included × 100",
            screening_score
        ),
        format!(
            "data_quality_score {:.1}/100 is confidence in the Yahoo snapshot, not a company-quality input",
            stock.data_quality_score
        ),
    ];
    if gated {
        score_provenance.push(format!(
            "investment total_score withheld; critical_coverage {:.0}%",
            audit.data_coverage.critical_pct
        ));
    }
    ScoreBreakdown {
        business_quality,
        industry_tailwind,
        financial_strength,
        management_quality,
        valuation_comfort,
        growth_triggers,
        risk_reward,
        total_score,
        interpretation,
        screening_score,
        score_provisional: gated,
        provisional_screening_score: if gated { available_dimension_score } else { None },
        available_dimension_score,
        critical_coverage_pct: crit,
        coverage_adjusted_score,
        score_provenance,
    }
}

pub fn build_monitorables(financials: &Financials, company_type: FinancialCompanyType) -> Vec<MonitorableItem> {
    if company_type.is_bank() {
        let pb_status = if financials.price_to_book > 0.0 && financials.return_on_equity.is_some() {
            "Reassess each result".to_string()
        } else {
            "P/B or ROE missing".to_string()
        };
        return vec![
            MonitorableItem { area: "Credit growth".to_string(), what_to_track: "Advances / loan growth vs guidance".to_string(), status: "Track via filings".to_string() },
            MonitorableItem { area: "Deposit growth".to_string(), what_to_track: "Total deposits YoY".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Deposit vs credit gap".to_string(), what_to_track: "Deposit growth minus credit growth".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "CASA ratio".to_string(), what_to_track: "Current + savings as % of deposits".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "NIM".to_string(), what_to_track: "Net interest margin vs cost of deposits".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Cost of deposits".to_string(), what_to_track: "Blended deposit cost and term mix".to_string(), status: "Track filings".to_string() },
            MonitorableItem { area: "LCR".to_string(), what_to_track: "Liquidity coverage ratio".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "GNPA / NNPA".to_string(), what_to_track: "Stage 3, slippages, PCR".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Credit cost".to_string(), what_to_track: "Provisions / average advances".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "ROA / ROE".to_string(), what_to_track: "Sustainable profitability vs P/B".to_string(), status: if financials.return_on_equity.is_some() { "Yahoo ROE/ROA present".to_string() } else { "Partial / missing".to_string() } },
            MonitorableItem { area: "CET1 / CRAR".to_string(), what_to_track: "Capital vs RBI floor and peer banks".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Loan / deposit ratio".to_string(), what_to_track: "LDR and wholesale funding reliance".to_string(), status: "Track filings".to_string() },
            MonitorableItem { area: "Retail / corporate mix".to_string(), what_to_track: "Loan-book mix and unsecured share".to_string(), status: "Track filings".to_string() },
            MonitorableItem { area: "P/B vs ROE".to_string(), what_to_track: "Multiple vs sustainable ROE and asset quality".to_string(), status: pb_status },
        ];
    }
    if company_type.is_lender() {
        let pb_status = if financials.price_to_book > 0.0 && financials.return_on_equity.is_some() {
            "Reassess each result".to_string()
        } else {
            "P/B or ROE missing".to_string()
        };
        return vec![
            MonitorableItem { area: "Loan book growth".to_string(), what_to_track: "AUM / advances vs prior year".to_string(), status: "Track via filings".to_string() },
            MonitorableItem { area: "Disbursements".to_string(), what_to_track: "Quarterly sanctions and disbursements".to_string(), status: "Track via filings".to_string() },
            MonitorableItem { area: "NIM / spread".to_string(), what_to_track: "NIM and yield minus cost of funds".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Cost of funds".to_string(), what_to_track: "Bond yields, bank lines, incremental COF".to_string(), status: "Track RBI / bond market".to_string() },
            MonitorableItem { area: "GNPA / NNPA".to_string(), what_to_track: "Stage 3, slippages, PCR".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Credit cost".to_string(), what_to_track: "Provisions / average advances".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "CRAR / Tier-I".to_string(), what_to_track: "Capital vs RBI floor".to_string(), status: "N/A in Yahoo".to_string() },
            MonitorableItem { area: "Borrowing mix".to_string(), what_to_track: "Bonds vs bank vs CP".to_string(), status: "Track presentations".to_string() },
            MonitorableItem { area: "ALM".to_string(), what_to_track: "Duration gap and refinance calendar".to_string(), status: "Track filings".to_string() },
            MonitorableItem { area: "Sector concentration".to_string(), what_to_track: "Power / infra / RE / state exposure".to_string(), status: "Track filings".to_string() },
            MonitorableItem { area: "P/B vs ROE".to_string(), what_to_track: "Multiple vs sustainable ROE and asset quality".to_string(), status: pb_status },
        ];
    }
    vec![
        MonitorableItem { area: "Revenue".to_string(), what_to_track: "Growth vs expectation".to_string(), status: if financials.revenue_growth.map(|g| g > 0.0).unwrap_or(false) { "On track".to_string() } else { "Watch".to_string() } },
        MonitorableItem { area: "Margin".to_string(), what_to_track: "Expansion or contraction".to_string(), status: if financials.profit_margins > 0.10 { "Healthy".to_string() } else { "Watch".to_string() } },
        MonitorableItem { area: "Volume".to_string(), what_to_track: "User/customer/order growth".to_string(), status: "Track via quarterly disclosures".to_string() },
        MonitorableItem { area: "Market share".to_string(), what_to_track: "Gaining or losing".to_string(), status: "Track with industry data".to_string() },
        MonitorableItem { area: "Debt".to_string(), what_to_track: "Increasing or reducing".to_string(), status: match financials.debt_to_equity {
            Some(d) if d < 0.70 => "Comfortable".to_string(),
            Some(_) => "Elevated".to_string(),
            None => "Unknown".to_string(),
        } },
        MonitorableItem { area: "Cash flow".to_string(), what_to_track: "CFO/PAT ratio".to_string(), status: match (financials.operating_cashflow, financials.net_income) {
            (Some(cfo), Some(ni)) if ni > 0.0 && cfo / ni >= 1.0 => "Strong".to_string(),
            (Some(_), _) => "Watch".to_string(),
            (None, _) => "Unknown".to_string(),
        } },
        MonitorableItem { area: "Capex".to_string(), what_to_track: "On time or delayed".to_string(), status: "Track management commentary".to_string() },
        MonitorableItem { area: "Management guidance".to_string(), what_to_track: "Upgraded or downgraded".to_string(), status: "Track each quarter".to_string() },
        MonitorableItem { area: "Valuation".to_string(), what_to_track: "P/E, EV/EBITDA vs growth".to_string(), status: "Reassess on each result".to_string() },
        MonitorableItem { area: "Regulation".to_string(), what_to_track: "Any adverse policy change".to_string(), status: "Track sector circulars".to_string() },
    ]
}

pub fn build_structured_sections(
    symbol: &str,
    long_name: Option<&str>,
    financials: &Financials,
    stock: &StockAnalysis,
    management: &ManagementAnalysis,
    sector: &SectorAnalysisDetail,
    subject: &PeerQuote,
    peers: &[PeerQuote],
    shareholders: &crate::models::Shareholders,
    bundle: &crate::models::StatementBundle,
    audit: &crate::models::FinancialStrengthAudit,
    canonical: &CanonicalMetrics,
    company_type: FinancialCompanyType,
) -> (StructuredResearchSections, ScoreBreakdown) {
    let cash_flow_quality = compute_cash_flow_quality(financials, canonical, Some(bundle));
    let risks = categorize_risks(financials, stock, shareholders, Some(audit), company_type);
    let score = compute_weighted_score(financials, stock, management, &risks, company_type, audit, canonical);
    let company = long_name.unwrap_or(symbol);
    let scenario_analysis = if company_type.is_bank() {
        ScenarioAnalysis {
            base_case: "Credit and deposit growth stay orderly; NIM and asset quality hold near recent run-rate.".to_string(),
            upside_case: "Faster CASA/deposit traction, stable credit cost, and operating leverage can support P/B vs ROE.".to_string(),
            downside_case: "Deposit slowdown vs credit, NIM compression, unsecured/retail stress, or capital consumption.".to_string(),
            capital_impairment_guardrail: "If capital or asset quality looks permanently impaired, avoid or cut size — do not treat missing GNPA as a clean book.".to_string(),
        }
    } else if company_type.is_lender() {
        ScenarioAnalysis {
            base_case: "Loan book and PAT track power/infra capex; NIM holds if funding costs stay orderly.".to_string(),
            upside_case: "Faster disbursements, stable asset quality, and cheaper funding can support P/B vs ROE.".to_string(),
            downside_case: "Discom stress, NIM compression, credit-cost spike, or ALM/funding shock.".to_string(),
            capital_impairment_guardrail: "If capital or asset quality looks permanently impaired, avoid or cut size — do not treat missing GNPA as a clean book.".to_string(),
        }
    } else {
        ScenarioAnalysis {
            base_case: "Growth tracks management guidance; valuation stays near long-term average.".to_string(),
            upside_case: "Faster revenue + margin expansion can drive operating leverage and re-rating.".to_string(),
            downside_case: "Demand slowdown, margin compression, or policy headwinds can lower earnings and multiples.".to_string(),
            capital_impairment_guardrail: "If downside case suggests permanent capital impairment, avoid or reduce position size.".to_string(),
        }
    };
    let growth_triggers = if company_type.is_bank() {
        vec![
            "Credit growth vs deposit growth and LDR.".to_string(),
            "CASA and cost of deposits.".to_string(),
            "NIM trajectory.".to_string(),
            "GNPA / NNPA / credit cost from filings.".to_string(),
            "CET1 / CRAR vs regulatory floor.".to_string(),
            "P/B vs ROE after asset-quality check.".to_string(),
        ]
    } else if company_type.is_lender() {
        vec![
            "Loan-book and disbursement growth vs guidance.".to_string(),
            "NIM / spread vs cost of funds.".to_string(),
            "GNPA / NNPA / credit cost from filings.".to_string(),
            "CRAR / Tier-I vs regulatory floor.".to_string(),
            "P/B vs ROE after asset-quality check.".to_string(),
        ]
    } else {
        vec![
            "Investigate new product/service ramps in filings (not confirmed from Yahoo).".to_string(),
            "Investigate capacity expansion / utilization in investor presentations.".to_string(),
            "Investigate sector policy or demand-cycle comments in results.".to_string(),
        ]
    };
    let sections = StructuredResearchSections {
        company_overview: format!("{} ({}) operates in {} / {}.", company, symbol, sector.sector.clone().unwrap_or_else(|| "N/A".to_string()), sector.industry.clone().unwrap_or_else(|| "N/A".to_string())),
        business_model: if company_type.is_bank() {
            "Universal commercial bank with retail, corporate and wholesale lending, a deposit franchise, payments/cards, treasury and fee businesses. Key economics are deposit franchise, credit growth, NIM, asset quality, fee income, operating efficiency and capital — not industrial volume/capex.".to_string()
        } else if company_type == FinancialCompanyType::NbfcProjectFinance {
            "Project-finance / credit book: disbursements, borrowing mix, spreads, and sector concentration — not industrial volume/capex.".to_string()
        } else if company_type.is_lender() {
            "NBFC credit book: disbursements, borrowing mix, spreads, and asset quality — not industrial volume/capex.".to_string()
        } else {
            "Review revenue drivers, customer mix, pricing power, and reinvestment discipline.".to_string()
        },
        industry_opportunity: sector.outlook_narrative.clone(),
        competitive_advantage: if company_type.is_bank() {
            "Assess low-cost deposit / CASA franchise, customer base, distribution and digital/payments ecosystem, underwriting, cross-sell, operating efficiency, brand, funding cost and scale.".to_string()
        } else if company_type.is_lender() {
            "Assess cost of funds, government/policy franchise, underwriting, and access to bond markets.".to_string()
        } else {
            "Assess moat via switching costs, brand strength, cost edge, and execution consistency.".to_string()
        },
        management_quality: management.narrative.clone(),
        financial_performance: stock.narrative.clone(),
        balance_sheet_strength: audit.interpretation.clone(),
        cash_flow_quality,
        valuation: stock.valuation_label.clone(),
        peer_comparison: build_peer_comparison_table_for(subject, peers, company_type),
        growth_triggers,
        risks,
        scenario_analysis,
        entry_exit_strategy: if company_type.is_bank() {
            "Do not rank on a gated Yahoo-only score. Prefer filings for GNPA, CRAR/CET1, NIM, CASA and deposit vs credit growth before size.".to_string()
        } else if company_type.is_lender() {
            "Do not rank on a gated Yahoo-only score. Prefer filings for GNPA, CRAR, and NIM before size.".to_string()
        } else {
            "Prefer staggered entries when valuation offers margin of safety; trim when thesis breaks or valuation materially outruns growth.".to_string()
        },
        key_monitorables: build_monitorables(financials, company_type),
        final_recommendation: format!(
            "Score {:.1}/100: {}.",
            score.total_score.unwrap_or(score.screening_score),
            score.interpretation
        ),
    };
    (sections, score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(symbol: &str, roe: Option<f64>) -> PeerQuote {
        PeerQuote {
            symbol: symbol.to_string(),
            short_name: Some(symbol.to_string()),
            price: 100.0,
            pe_ratio: 12.0,
            forward_pe: None,
            price_to_book: 1.0,
            price_to_sales: 1.0,
            ev_to_ebitda: None,
            ev_to_sales: None,
            market_cap: 1e12,
            revenue: 1e11,
            revenue_growth: Some(0.1),
            pat_growth: Some(0.1),
            ebitda: 1e10,
            ebitda_margin: Some(0.1),
            return_on_equity: roe,
            return_on_capital_employed: None,
            return_on_assets: None,
            profit_margins: 0.08,
            debt_to_equity: Some(0.4),
            free_cashflow: Some(1e9),
            officer_pay: 0.0,
            average_volume_10_day: 1.0,
            dividend_yield: 0.0,
            industrial_metrics_analysis_applicable: true,
        }
    }

    #[test]
    fn bank_templates_are_not_nbfc_project_finance() {
        let f = Financials {
            pe_ratio: 15.75,
            price_to_book: 1.83,
            return_on_equity: Some(0.138),
            ..Default::default()
        };
        let m = build_monitorables(&f, FinancialCompanyType::Bank);
        assert!(m.iter().any(|x| x.area == "CASA ratio"));
        assert!(m.iter().any(|x| x.area == "Deposit growth"));
        assert!(!m.iter().any(|x| x.area == "Disbursements"));
        assert!(!m.iter().any(|x| x.area.contains("Borrowing mix")));
        let nbfc = build_monitorables(&f, FinancialCompanyType::NbfcProjectFinance);
        assert!(nbfc.iter().any(|x| x.area == "Disbursements"));
        let risks = lender_risks(FinancialCompanyType::Bank, &f, &crate::models::Shareholders::default());
        assert!(!risks.business_risks.iter().any(|r| r.risk.contains("discom") || r.risk.contains("Power")));
        assert!(risks.financial_risks.iter().any(|r| r.note.contains("RBI Basel") || r.risk.contains("LDR")));
    }

    #[test]
    fn lender_peer_table_drops_industrial_multiples() {
        let subject = peer("RECLTD.NS", Some(0.2));
        let rows = build_peer_comparison_table_for(&subject, &[], FinancialCompanyType::NbfcProjectFinance);
        assert!(rows.iter().any(|r| r.metric == "P/B"));
        assert!(rows.iter().any(|r| r.metric == "NIM"));
        assert!(!rows.iter().any(|r| r.metric.contains("EBITDA")));
        assert!(!rows.iter().any(|r| r.metric.contains("Revenue growth")));
    }

    #[test]
    fn lender_peer_narrative_does_not_say_medium() {
        let subject = peer("RECLTD.NS", Some(0.2));
        let a = compute_peer_analysis_for(
            &subject,
            &[],
            Some("Credit Services"),
            Some("Rural electrification project finance."),
            FinancialCompanyType::NbfcProjectFinance,
        );
        assert_eq!(a.peer_comparability, "high");
        assert!(a.narrative.contains("Direct peer comparability is High for PFC / IREDA / IRFC / HUDCO"));
        assert!(!a.narrative.to_lowercase().contains("peer comparability: medium"));
    }

    #[test]
    fn missing_peer_roe_does_not_become_zero_percentile() {
        let subject = peer("RELIANCE.NS", None);
        let peers = vec![
            peer("ONGC.NS", None),
            peer("IOC.NS", None),
            peer("BPCL.NS", None),
        ];
        let a = compute_peer_analysis(&subject, &peers, None, None);
        assert!(a.subject_percentile_roe.is_none());
        assert_eq!(a.roe_coverage_known, 0);
        assert!(a.narrative.contains("ROE percentile: N/A"));
    }

    #[test]
    fn missing_roe_does_not_penalize_quality_like_zero() {
        let missing = Financials::default();
        let zero = Financials {
            return_on_equity: Some(0.0),
            ..Default::default()
        };
        assert!((evaluate_quality(&missing) - 50.0).abs() < 1e-9);
        assert!(evaluate_quality(&zero) < evaluate_quality(&missing));
    }

    #[test]
    fn missing_de_does_not_get_low_leverage_bonus() {
        let missing = Financials::default();
        let low = Financials {
            debt_to_equity: Some(0.2),
            ..Default::default()
        };
        assert!(evaluate_quality(&low) > evaluate_quality(&missing));
    }

    #[test]
    fn officer_pay_score_has_no_tiny_ratio_cliff() {
        let rev = 1e13_f64;
        let missing = evaluate_management(0.0, rev);
        assert!((missing - 50.0).abs() < 1e-9);
        let tiny = evaluate_management(rev * 5e-6, rev);
        let small = evaluate_management(rev * 2e-5, rev);
        assert!((tiny - small).abs() < 2.0, "tiny={tiny} small={small}");
        assert!(tiny > 80.0);
    }

    #[test]
    fn missing_peer_pat_growth_is_none_not_zero() {
        let mut subject = peer("RELIANCE.NS", Some(0.1));
        subject.pat_growth = Some(-0.224);
        let mut ioc = peer("IOC.NS", None);
        ioc.pat_growth = None;
        let rows = build_peer_comparison_table(&subject, &[ioc]);
        let pat = rows.iter().find(|r| r.metric.contains("earningsGrowth")).unwrap();
        assert!((pat.company.unwrap() + 22.4).abs() < 1e-9);
        assert!(pat.peer_1.is_none());
    }

    #[test]
    fn gated_lender_score_excludes_placeholder_financial_and_management() {
        let stock = StockAnalysis {
            data_quality_score: 70.0,
            quality_score_kind: String::new(),
            valuation_label: "Fairly Valued".into(),
            revenue_cagr_full_series_pct: None,
            net_income_cagr_full_series_pct: None,
            revenue_cagr_trailing_3y_pct: None,
            net_income_cagr_trailing_3y_pct: None,
            revenue_cagr_trailing_5y_pct: None,
            net_income_cagr_trailing_5y_pct: None,
            margin_trend: None,
            narrative: String::new(),
            fcf_yield_pct: None,
            earnings_yield_pct: None,
            peg_style_note: String::new(),
            price_in_52w_range: None,
            distance_from_52w_high_pct: None,
        };
        let mgmt = ManagementAnalysis {
            pay_vs_revenue_score: None,
            tone_score: 47.0,
            tone_label: "Neutral".into(),
            narrative: String::new(),
        };
        let risks = RiskBuckets {
            business_risks: vec![],
            financial_risks: vec![],
            management_risks: vec![],
            valuation_risks: vec![],
            regulatory_risks: vec![],
        };
        let mut audit = FinancialStrengthAudit::default();
        audit.scores_provisional = true;
        audit.overall_strength_score = None;
        audit.data_coverage.recommendation_gated = true;
        audit.data_coverage.critical_pct = 20.0;
        let s = compute_weighted_score(
            &Financials {
                pe_ratio: 6.0,
                ..Default::default()
            },
            &stock,
            &mgmt,
            &risks,
            FinancialCompanyType::NbfcProjectFinance,
            &audit,
            &CanonicalMetrics::default(),
        );
        assert!(s.financial_strength.is_none());
        assert!(s.management_quality.is_none());
        assert!(s.business_quality.is_none());
        assert!(s.industry_tailwind.is_none());
        assert!(s.coverage_adjusted_score.is_none());
        assert!(s.available_dimension_score.is_some());
        assert!((s.critical_coverage_pct.unwrap() - 20.0).abs() < 1e-9);
        assert!(s.interpretation.contains("Investment rating withheld"));
        assert!(s.risk_reward.is_none());
        assert!(s.growth_triggers.is_none());
        assert!(s.score_provenance.iter().any(|p| p.contains("risk_reward excluded")));
        assert!(s.score_provenance.iter().any(|p| p.contains("business_quality excluded")));
    }

    #[test]
    fn growth_triggers_prefer_statement_series_over_yahoo_earnings_growth() {
        let canonical = CanonicalMetrics {
            nii_yoy_pct: Some(6.8),
            fy_pat_yoy_pct: Some(4.6),
            pat_cagr_3y_pct: Some(12.5),
            interest_income_yoy_pct: Some(4.1),
            ..Default::default()
        };
        let (score, prov) = super::statement_first_growth_triggers(
            &canonical,
            &Financials {
                earnings_growth: Some(0.181),
                ..Default::default()
            },
            FinancialCompanyType::Bank,
        );
        let score = score.expect("statement growth should score");
        assert!(score < 8.5, "should not be ~9 from earningsGrowth 18.1% alone, got {score}");
        assert!(prov.contains("NII YoY"));
        assert!(prov.contains("secondary"));
    }

    #[test]
    fn lender_data_quality_is_not_a_company_strength() {
        let stock = StockAnalysis {
            data_quality_score: 80.0,
            quality_score_kind: String::new(),
            valuation_label: "Fairly Valued".into(),
            revenue_cagr_full_series_pct: None,
            net_income_cagr_full_series_pct: None,
            revenue_cagr_trailing_3y_pct: None,
            net_income_cagr_trailing_3y_pct: None,
            revenue_cagr_trailing_5y_pct: None,
            net_income_cagr_trailing_5y_pct: None,
            margin_trend: None,
            narrative: String::new(),
            fcf_yield_pct: None,
            earnings_yield_pct: None,
            peg_style_note: String::new(),
            price_in_52w_range: None,
            distance_from_52w_high_pct: None,
        };
        let mgmt = ManagementAnalysis {
            pay_vs_revenue_score: None,
            tone_score: 47.0,
            tone_label: "Neutral".into(),
            narrative: String::new(),
        };
        let peer = PeerAnalysis {
            peers: vec![],
            benchmarks: vec![],
            subject_data_quality_score: 80.0,
            subject_pay_vs_revenue_score: None,
            subject_percentile_pe: None,
            subject_percentile_roe: None,
            subject_percentile_quality: None,
            subject_percentile_pay_efficiency: None,
            roe_coverage_known: 0,
            roe_coverage_total: 0,
            narrative: String::new(),
            peer_comparability: String::new(),
            direct_peer_comparability: String::new(),
            bank_comparability: String::new(),
            peer_set_kind: String::new(),
        };
        let coverage = DataCoverage {
            recommendation_gated: true,
            critical_pct: 20.0,
            ..Default::default()
        };
        let ins = compute_report_insights(
            &stock,
            &mgmt,
            &peer,
            &Financials::default(),
            FinancialCompanyType::NbfcProjectFinance,
            Some(&coverage),
        );
        assert!(ins.strengths.is_empty());
        assert!(ins.data_strengths.iter().any(|s| s.contains("Yahoo coverage")));
        assert!(ins
            .watch_items
            .iter()
            .any(|w| w.contains("critical lender-risk coverage is insufficient")));
    }
}

