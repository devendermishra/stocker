use crate::math::cagr;
use crate::models::{
    AnnualReport, CashFlowQuality, Financials, ManagementAnalysis, MonitorableItem, PeerAnalysis,
    PeerBenchmark, PeerComparisonRow, PeerQuote, ReportInsights, RiskBuckets, RiskCategory, RiskItem,
    ScenarioAnalysis, ScoreBreakdown, SectorAnalysisDetail, StockAnalysis, StructuredResearchSections,
};

pub fn evaluate_quality(financials: &Financials) -> f64 {
    let mut score: f64 = 50.0;

    if financials.profit_margins > 0.15 {
        score += 15.0;
    } else if financials.profit_margins > 0.05 {
        score += 5.0;
    } else if financials.profit_margins < 0.0 {
        score -= 10.0;
    }

    if financials.return_on_equity > 0.15 {
        score += 15.0;
    } else if financials.return_on_equity < 0.05 {
        score -= 5.0;
    }

    if financials.debt_to_equity < 0.50 {
        score += 10.0;
    } else if financials.debt_to_equity > 1.50 {
        score -= 10.0;
    }

    if financials.free_cashflow > 0.0 {
        score += 10.0;
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
        return 45.0;
    }
    let pay_ratio = if officer_pay > 0.0 && officer_pay.is_finite() {
        officer_pay / revenue
    } else {
        0.0
    };
    let mut score: f64 = 62.0;

    if pay_ratio > 0.0 {
        // Smoothly scale penalty by compensation intensity relative to revenue.
        let normalized = (pay_ratio / 0.02).clamp(0.0, 2.0);
        score += (1.0 - normalized) * 24.0;
    } else {
        // Missing pay data is common; award a small neutral-positive bump, not a max score.
        score += 8.0;
    }

    score.clamp(0.0_f64, 100.0_f64)
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
    if reports.len() < 2 {
        return "Insufficient history".to_string();
    }
    let mut sorted = reports.to_vec();
    sorted.sort_by(|a, b| a.date.cmp(&b.date));
    let first = &sorted[0];
    let last = sorted.last().unwrap();
    let m0 = if first.revenue > 0.0 {
        first.net_income / first.revenue
    } else {
        0.0
    };
    let m1 = if last.revenue > 0.0 {
        last.net_income / last.revenue
    } else {
        0.0
    };
    if m1 > m0 + 0.01 {
        "Improving net margin vs oldest year in series".to_string()
    } else if m1 + 0.01 < m0 {
        "Compressing net margin vs oldest year in series".to_string()
    } else {
        "Stable net margin vs oldest year in series".to_string()
    }
}

/// Last `span_years` fiscal years of growth (need `span_years + 1` annual points).
fn trailing_cagr_revenue_ni(
    reports: &[AnnualReport],
    span_years: usize,
) -> (Option<f64>, Option<f64>) {
    if reports.len() < span_years + 1 {
        return (None, None);
    }
    let mut sorted = reports.to_vec();
    sorted.sort_by(|a, b| a.date.cmp(&b.date));
    let n = sorted.len();
    let start = &sorted[n - 1 - span_years];
    let end = &sorted[n - 1];
    let y = span_years as f64;
    let rev = cagr(start.revenue, end.revenue, y);
    let ni = cagr(
        start.net_income.max(1.0),
        end.net_income.max(1.0),
        y,
    );
    (rev, ni)
}

fn fcf_and_earnings_yields(
    price: f64,
    financials: &Financials,
) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>, String) {
    let fcf_y = if financials.market_cap > 0.0 && financials.free_cashflow.is_finite() {
        Some((financials.free_cashflow / financials.market_cap) * 100.0)
    } else {
        None
    };
    let earn_y = if price > 0.0 && financials.trailing_eps > 0.0 {
        Some((financials.trailing_eps / price) * 100.0)
    } else if financials.pe_ratio > 0.0 {
        Some(100.0 / financials.pe_ratio)
    } else {
        None
    };
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
        let g = financials.earnings_growth;
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
) -> StockAnalysis {
    let quality_score = evaluate_quality(financials);
    let valuation_label = evaluate_valuation(price, financials);
    let margin_trend = margin_trend_from_reports(annual_reports);
    let (fcf_yield_pct, earnings_yield_pct, price_in_52w_range, distance_from_52w_high_pct, peg_style_note) =
        fcf_and_earnings_yields(price, financials);

    let (rev_full, ni_full) = {
        let mut sorted = annual_reports.to_vec();
        sorted.sort_by(|a, b| a.date.cmp(&b.date));
        if sorted.len() >= 2 {
            let first = &sorted[0];
            let last = sorted.last().unwrap();
            let years = (sorted.len().saturating_sub(1).max(1)) as f64;
            (
                cagr(first.revenue, last.revenue, years),
                cagr(
                    first.net_income.max(1.0),
                    last.net_income.max(1.0),
                    years,
                ),
            )
        } else {
            (None, None)
        }
    };
    let (rev_3, ni_3) = trailing_cagr_revenue_ni(annual_reports, 3);
    let (rev_5, ni_5) = trailing_cagr_revenue_ni(annual_reports, 5);

    let mut narrative = String::new();
    narrative.push_str(&format!(
        "Quality score {:.0}/100. {}. ",
        quality_score, valuation_label
    ));
    if let (Some(r), Some(n)) = (rev_3, ni_3) {
        narrative.push_str(&format!(
            "Trailing 3Y revenue CAGR (annual statements): {:.1}% pa; net income {:.1}%. ",
            r, n
        ));
    } else if let (Some(r), Some(n)) = (rev_full, ni_full) {
        narrative.push_str(&format!(
            "Revenue / net income full-series CAGR: {:.1}% / {:.1}% pa (span = fiscal points available). ",
            r, n
        ));
    }
    if let Some(fy) = fcf_yield_pct {
        narrative.push_str(&format!("FCF yield vs market cap (Yahoo) ~{:.1}%. ", fy));
    }
    narrative.push_str(&margin_trend);
    narrative.push('.');

    StockAnalysis {
        quality_score,
        valuation_label,
        revenue_cagr_full_series_pct: rev_full,
        net_income_cagr_full_series_pct: ni_full,
        revenue_cagr_trailing_3y_pct: rev_3,
        net_income_cagr_trailing_3y_pct: ni_3,
        revenue_cagr_trailing_5y_pct: rev_5,
        net_income_cagr_trailing_5y_pct: ni_5,
        margin_trend: margin_trend.clone(),
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
) -> ReportInsights {
    let mut strengths: Vec<String> = Vec::new();
    let mut watch: Vec<String> = Vec::new();

    if stock.quality_score >= 60.0 {
        strengths.push("Composite quality heuristic is above our mid band (Yahoo-based).".to_string());
    } else if stock.quality_score < 40.0 {
        watch.push("Composite quality score is on the low side; dig into drivers (margins, ROE, debt, FCF).".to_string());
    }
    if financials.debt_to_equity > 1.00 {
        watch.push("Debt-to-equity (Yahoo) is elevated; review leverage vs peers and covenants.".to_string());
    } else if financials.debt_to_equity < 0.30 && financials.debt_to_equity > 0.0 {
        strengths.push("Debt-to-equity (Yahoo) is relatively light vs higher-leverage screen.".to_string());
    }
    if let Some(fy) = stock.fcf_yield_pct {
        if fy > 3.0 {
            strengths.push(format!("FCF yield to market cap ~{:.1}% — check sustainability vs capex and working capital.", fy));
        } else if fy < 0.0 {
            watch.push("Negative or weak FCF yield to market cap on last Yahoo print — triangulate with filings.".to_string());
        }
    }
    if let Some(p) = peer.subject_percentile_roe {
        if p >= 65.0 {
            strengths.push(format!("ROE vs this peer set is in the top third (~{:.0}th percentile).", p));
        } else if p < 30.0 {
            watch.push(format!("ROE vs this peer set is in the lower band (~{:.0}th percentile).", p));
        }
    }
    if management.pay_vs_revenue_score < 45.0 {
        watch.push("Officer comp vs revenue screen is weak — verify in annual report / governance section.".to_string());
    } else if management.pay_vs_revenue_score > 70.0 {
        strengths.push("Officer pay vs revenue (heuristic) is not a red flag in this pass.".to_string());
    }
    if stock
        .margin_trend
        .contains("Compressing")
    {
        watch.push("Net margin is compressing vs oldest year in the Yahoo series.".to_string());
    } else if stock.margin_trend.contains("Improving") {
        strengths.push("Net margin trend in the series is positive vs oldest year (Yahoo history).".to_string());
    }

    let pe_part = if financials.pe_ratio > 0.0 {
        format!("trailing/forward P/E (Yahoo) in the {:.0}x area", financials.pe_ratio)
    } else {
        "P/E is missing or N/A in the feed".to_string()
    };
    let executive_summary = format!(
        "Snapshot (Yahoo, heuristic, not investment advice). Quality score {:.0}/100. {}. Valuation: {}. Peer percentiles are vs a small fetched list, not a curated comp set. Confirm in SEBI/NSE documents.",
        stock.quality_score, pe_part, stock.valuation_label
    );
    if strengths.is_empty() {
        strengths.push("No standout automated strengths; verify thesis from filings and business mix.".to_string());
    }
    if watch.is_empty() {
        watch.push("No major automated watch flags; still check liquidity, guidance, and one-offs.".to_string());
    }
    ReportInsights {
        executive_summary,
        strengths,
        watch_items: watch,
    }
}

pub fn compute_management_analysis(
    officer_pay: f64,
    revenue: f64,
    summary: Option<&str>,
) -> ManagementAnalysis {
    let pay_vs_revenue_score = evaluate_management(officer_pay, revenue);
    let (tone_score, tone_label) = tone_from_summary(summary);
    let narrative = format!(
        "Officer pay vs revenue score {:.0}/100 (heuristic). \
         Public business summary tone: {} (score {:.0}/100). \
         This is not a substitute for reading SEBI filings.",
        pay_vs_revenue_score, tone_label, tone_score
    );
    ManagementAnalysis {
        pay_vs_revenue_score,
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
    let outlook_narrative = match (sector.as_ref(), industry.as_ref()) {
        (Some(s), Some(i)) if !s.is_empty() && !i.is_empty() => {
            format!("Sector: {}. Industry: {}. Yahoo data only; treat as a starting point.", s, i)
        }
        (Some(s), _) if !s.is_empty() => format!("Sector: {}.", s),
        _ => "Sector metadata unavailable from data provider.".to_string(),
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

pub fn compute_peer_analysis(subject: &PeerQuote, peers: &[PeerQuote]) -> PeerAnalysis {
    let pe_list: Vec<f64> = peers.iter().map(|p| p.pe_ratio).filter(|p| *p > 0.0).collect();
    let roe_list: Vec<f64> = peers.iter().map(|p| p.return_on_equity).collect();
    let quality_list: Vec<f64> = peers
        .iter()
        .map(|p| {
            evaluate_quality(&Financials {
                revenue: p.revenue,
                net_income: 0.0,
                pe_ratio: p.pe_ratio,
                total_debt: 0.0,
                ebitda: 0.0,
                profit_margins: p.profit_margins,
                return_on_equity: p.return_on_equity,
                debt_to_equity: p.debt_to_equity,
                free_cashflow: p.free_cashflow,
                operating_cashflow: 0.0,
                shares_outstanding: 0.0,
                market_cap: p.market_cap,
                ..Default::default()
            })
        })
        .collect();
    let mgmt_list: Vec<f64> = peers
        .iter()
        .map(|p| evaluate_management(p.officer_pay, p.revenue))
        .collect();

    let subject_quality_score = evaluate_quality(&Financials {
        revenue: subject.revenue,
        net_income: 0.0,
        pe_ratio: subject.pe_ratio,
        total_debt: 0.0,
        ebitda: 0.0,
        profit_margins: subject.profit_margins,
        return_on_equity: subject.return_on_equity,
        debt_to_equity: subject.debt_to_equity,
        free_cashflow: subject.free_cashflow,
        operating_cashflow: 0.0,
        shares_outstanding: 0.0,
        market_cap: subject.market_cap,
        ..Default::default()
    });
    let subject_pay_vs_revenue_score = evaluate_management(subject.officer_pay, subject.revenue);

    let subject_percentile_pe = if subject.pe_ratio > 0.0 {
        percentile_among(subject.pe_ratio, &pe_list)
    } else {
        None
    };
    let subject_percentile_roe = percentile_among(subject.return_on_equity, &roe_list);
    let subject_percentile_quality = percentile_among(subject_quality_score, &quality_list);
    let subject_percentile_pay_efficiency =
        percentile_among(subject_pay_vs_revenue_score, &mgmt_list);

    let benchmarks: Vec<PeerBenchmark> = peers
        .iter()
        .map(|p| {
            let quality_score = evaluate_quality(&Financials {
                revenue: p.revenue,
                net_income: 0.0,
                pe_ratio: p.pe_ratio,
                total_debt: 0.0,
                ebitda: 0.0,
                profit_margins: p.profit_margins,
                return_on_equity: p.return_on_equity,
                debt_to_equity: p.debt_to_equity,
                free_cashflow: p.free_cashflow,
                operating_cashflow: 0.0,
                shares_outstanding: 0.0,
                market_cap: p.market_cap,
                ..Default::default()
            });
            let pay_vs_revenue_score = evaluate_management(p.officer_pay, p.revenue);
            let pay_to_revenue_pct = if p.revenue > 0.0 {
                Some((p.officer_pay / p.revenue) * 100.0)
            } else {
                None
            };
            PeerBenchmark {
                symbol: p.symbol.clone(),
                short_name: p.short_name.clone(),
                quality_score,
                pay_vs_revenue_score,
                pay_to_revenue_pct,
            }
        })
        .collect();

    let mut narrative = String::new();
    if let Some(p) = subject_percentile_pe {
        narrative.push_str(&format!(
            "Trailing P/E is around the {:.0}th percentile vs fetched peers (0=lowest). ",
            p
        ));
    }
    if let Some(r) = subject_percentile_roe {
        narrative.push_str(&format!(
            "ROE is around the {:.0}th percentile vs fetched peers. ",
            r
        ));
    }
    if let Some(q) = subject_percentile_quality {
        narrative.push_str(&format!(
            "Composite quality score is around the {:.0}th percentile vs peers. ",
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
        subject_quality_score,
        subject_pay_vs_revenue_score,
        subject_percentile_pe,
        subject_percentile_roe,
        subject_percentile_quality,
        subject_percentile_pay_efficiency,
        narrative,
    }
}

fn clamp_score(v: f64, max: f64) -> f64 {
    v.max(0.0).min(max)
}

pub fn compute_cash_flow_quality(
    financials: &Financials,
    bundle: Option<&crate::models::StatementBundle>,
) -> CashFlowQuality {
    let pat = financials.net_income;
    let cfo = financials.operating_cashflow;
    let ebitda = financials.ebitda;
    let free_cashflow = financials.free_cashflow;
    let capex_estimate = if cfo > 0.0 || free_cashflow != 0.0 {
        (cfo - free_cashflow).max(0.0)
    } else {
        0.0
    };
    let pat_vs_cfo_delta = if pat != 0.0 { Some(cfo - pat) } else { None };
    let cfo_vs_ebitda_ratio = if ebitda > 0.0 { Some(cfo / ebitda) } else { None };
    let cash_conversion_ratio = if pat > 0.0 { Some(cfo / pat) } else { None };
    let capex_requirement_ratio = if cfo > 0.0 { Some(capex_estimate / cfo) } else { None };

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
    vec![
        PeerComparisonRow {
            metric: "Revenue growth".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.revenue_growth * 100.0),
            peer_1: Some(p1.revenue_growth * 100.0),
            peer_2: Some(p2.revenue_growth * 100.0),
            peer_3: Some(p3.revenue_growth * 100.0),
        },
        PeerComparisonRow {
            metric: "EBITDA margin".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.ebitda_margin * 100.0),
            peer_1: Some(p1.ebitda_margin * 100.0),
            peer_2: Some(p2.ebitda_margin * 100.0),
            peer_3: Some(p3.ebitda_margin * 100.0),
        },
        PeerComparisonRow {
            metric: "PAT growth".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.pat_growth * 100.0),
            peer_1: Some(p1.pat_growth * 100.0),
            peer_2: Some(p2.pat_growth * 100.0),
            peer_3: Some(p3.pat_growth * 100.0),
        },
        PeerComparisonRow {
            metric: "ROE".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.return_on_equity * 100.0),
            peer_1: Some(p1.return_on_equity * 100.0),
            peer_2: Some(p2.return_on_equity * 100.0),
            peer_3: Some(p3.return_on_equity * 100.0),
        },
        PeerComparisonRow {
            metric: "ROCE".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: subject.return_on_capital_employed.map(|v| v * 100.0),
            peer_1: p1.return_on_capital_employed.map(|v| v * 100.0),
            peer_2: p2.return_on_capital_employed.map(|v| v * 100.0),
            peer_3: p3.return_on_capital_employed.map(|v| v * 100.0),
        },
        PeerComparisonRow {
            metric: "Debt/equity".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.debt_to_equity),
            peer_1: Some(p1.debt_to_equity),
            peer_2: Some(p2.debt_to_equity),
            peer_3: Some(p3.debt_to_equity),
        },
        PeerComparisonRow {
            metric: "P/E".to_string(),
            company_label: company_label.clone(),
            peer_1_label: p1_label.clone(),
            peer_2_label: p2_label.clone(),
            peer_3_label: p3_label.clone(),
            company: Some(subject.pe_ratio),
            peer_1: Some(p1.pe_ratio),
            peer_2: Some(p2.pe_ratio),
            peer_3: Some(p3.pe_ratio),
        },
        PeerComparisonRow {
            metric: "EV/EBITDA".to_string(),
            company_label,
            peer_1_label: p1_label,
            peer_2_label: p2_label,
            peer_3_label: p3_label,
            company: subject.ev_to_ebitda,
            peer_1: p1.ev_to_ebitda,
            peer_2: p2.ev_to_ebitda,
            peer_3: p3.ev_to_ebitda,
        },
    ]
}

pub fn categorize_risks(
    financials: &Financials,
    stock: &StockAnalysis,
    shareholders: &crate::models::Shareholders,
    audit: Option<&crate::models::FinancialStrengthAudit>,
) -> RiskBuckets {
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
    if financials.debt_to_equity > 1.00 {
        financial_risks.push(RiskItem { category: RiskCategory::Financial, risk: "High debt".to_string(), severity: "High".to_string(), note: "Debt/equity is elevated versus comfort zone.".to_string() });
    }
    if financials.free_cashflow <= 0.0 {
        financial_risks.push(RiskItem { category: RiskCategory::Financial, risk: "Poor cash flow".to_string(), severity: "High".to_string(), note: "Free cash flow is weak/negative on latest print.".to_string() });
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
        if a.earnings_quality_score < 40.0 {
            financial_risks.push(RiskItem {
                category: RiskCategory::Financial,
                risk: "Earnings quality audit failed".to_string(),
                severity: "High".to_string(),
                note: format!(
                    "Earnings quality score {:.0}/100 — CFO/PAT and working capital checks weak.",
                    a.earnings_quality_score
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

pub fn compute_weighted_score(
    financials: &Financials,
    stock: &StockAnalysis,
    management: &ManagementAnalysis,
    risks: &RiskBuckets,
) -> ScoreBreakdown {
    let business_quality = clamp_score((stock.quality_score / 100.0) * 20.0, 20.0);
    let industry_tailwind = clamp_score(9.0 + (financials.revenue_growth * 20.0), 15.0);
    let financial_strength = clamp_score(
        ((financials.return_on_equity * 100.0).min(20.0) / 20.0) * 12.0
            + if financials.debt_to_equity < 0.60 { 8.0 } else { 4.0 },
        20.0,
    );
    let management_quality = clamp_score((management.pay_vs_revenue_score / 100.0) * 15.0, 15.0);
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
    let growth_triggers = clamp_score((financials.earnings_growth * 100.0).max(0.0) / 2.0, 10.0);
    let risk_penalty = (risks.financial_risks.len() + risks.management_risks.len()) as f64;
    let risk_reward = clamp_score(5.0 - (risk_penalty * 0.5), 5.0);
    let total_score = business_quality
        + industry_tailwind
        + financial_strength
        + management_quality
        + valuation_comfort
        + growth_triggers
        + risk_reward;
    let interpretation = if total_score >= 80.0 {
        "High-quality candidate".to_string()
    } else if total_score >= 65.0 {
        "Good, but check valuation/risk".to_string()
    } else if total_score >= 50.0 {
        "Watchlist only".to_string()
    } else {
        "Avoid unless special situation".to_string()
    };
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
    }
}

pub fn build_monitorables(financials: &Financials) -> Vec<MonitorableItem> {
    vec![
        MonitorableItem { area: "Revenue".to_string(), what_to_track: "Growth vs expectation".to_string(), status: if financials.revenue_growth > 0.0 { "On track".to_string() } else { "Watch".to_string() } },
        MonitorableItem { area: "Margin".to_string(), what_to_track: "Expansion or contraction".to_string(), status: if financials.profit_margins > 0.10 { "Healthy".to_string() } else { "Watch".to_string() } },
        MonitorableItem { area: "Volume".to_string(), what_to_track: "User/customer/order growth".to_string(), status: "Track via quarterly disclosures".to_string() },
        MonitorableItem { area: "Market share".to_string(), what_to_track: "Gaining or losing".to_string(), status: "Track with industry data".to_string() },
        MonitorableItem { area: "Debt".to_string(), what_to_track: "Increasing or reducing".to_string(), status: if financials.debt_to_equity < 0.70 { "Comfortable".to_string() } else { "Elevated".to_string() } },
        MonitorableItem { area: "Cash flow".to_string(), what_to_track: "CFO/PAT ratio".to_string(), status: if financials.net_income > 0.0 && financials.operating_cashflow / financials.net_income >= 1.0 { "Strong".to_string() } else { "Watch".to_string() } },
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
) -> (StructuredResearchSections, ScoreBreakdown) {
    let cash_flow_quality = compute_cash_flow_quality(financials, Some(bundle));
    let risks = categorize_risks(financials, stock, shareholders, Some(audit));
    let score = compute_weighted_score(financials, stock, management, &risks);
    let company = long_name.unwrap_or(symbol);
    let scenario_analysis = ScenarioAnalysis {
        base_case: "Growth tracks management guidance; valuation stays near long-term average.".to_string(),
        upside_case: "Faster revenue + margin expansion can drive operating leverage and re-rating.".to_string(),
        downside_case: "Demand slowdown, margin compression, or policy headwinds can lower earnings and multiples.".to_string(),
        capital_impairment_guardrail: "If downside case suggests permanent capital impairment, avoid or reduce position size.".to_string(),
    };
    let sections = StructuredResearchSections {
        company_overview: format!("{} ({}) operates in {} / {}.", company, symbol, sector.sector.clone().unwrap_or_else(|| "N/A".to_string()), sector.industry.clone().unwrap_or_else(|| "N/A".to_string())),
        business_model: "Review revenue drivers, customer mix, pricing power, and reinvestment discipline.".to_string(),
        industry_opportunity: sector.outlook_narrative.clone(),
        competitive_advantage: "Assess moat via switching costs, brand strength, cost edge, and execution consistency.".to_string(),
        management_quality: management.narrative.clone(),
        financial_performance: stock.narrative.clone(),
        balance_sheet_strength: audit.interpretation.clone(),
        cash_flow_quality,
        valuation: stock.valuation_label.clone(),
        peer_comparison: build_peer_comparison_table(subject, peers),
        growth_triggers: vec![
            "New product/service ramp".to_string(),
            "Capacity expansion and utilization gains".to_string(),
            "Favorable sector policy or demand cycle".to_string(),
        ],
        risks,
        scenario_analysis,
        entry_exit_strategy: "Prefer staggered entries when valuation offers margin of safety; trim when thesis breaks or valuation materially outruns growth.".to_string(),
        key_monitorables: build_monitorables(financials),
        final_recommendation: format!("Score {:.1}/100: {}.", score.total_score, score.interpretation),
    };
    (sections, score)
}
