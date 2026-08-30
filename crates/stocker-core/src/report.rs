use serde::Serialize;

use crate::analysis::{
    build_structured_sections, compute_peer_analysis_for, compute_report_insights,
    compute_sector_analysis_for,
};
use crate::error::Result;
use crate::fetcher::{
    discover_nse_peer_symbols, fetch_asset_profile, fetch_chart_history,
    fetch_company_news, fetch_financials, fetch_market_signals, fetch_officer_pay, fetch_peer_quotes,
    fetch_price, fetch_sector_news, fetch_shareholders, fetch_statement_bundle,
    sanitize_india_insider_transactions,
};
use crate::bank_metrics::bank_metrics_for;
use crate::models::{
    AssetProfile, BankingMetrics, CanonicalMetrics, CompanyOverview, Financials, FinancialsApplicable, FinancialsRawYahoo, FinancialStrengthAudit, FundamentalAnalysis,
    ManagementAnalysis, MarketSignals, NewsItem, PeerAnalysis, PeerQuote, ReportInsights,
    ResearchRating, ResearchSummary, ScoreBreakdown, ScreenerMetricSnapshot, SectorAnalysisDetail,
    Shareholders, StockAnalysis, StructuredResearchSections, TechnicalAnalysis,
    TechnicalEntrySignal, ValuationAnalysis,
};
use crate::research_summary::build_company_overview;
use crate::technical_analysis::build_technical_analysis;
use crate::technical_entry_signal::build_technical_entry_signal;
use crate::symbol::{default_india_symbol_context, resolve_india_symbol, IndiaSymbolContext};
use crate::yahoo_metrics::{
    ev_to_ebitda, ev_to_sales, report_fcf, resolve_enterprise_value,
};

#[derive(Debug, Clone, Serialize)]
pub struct ResearchReport {
    pub symbol: String,
    pub long_name: Option<String>,
    pub price: f64,
    /// ISO-8601 UTC time of this live Yahoo fan-out.
    pub retrieved_at: String,
    pub financials: Financials,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub financials_applicable: Option<FinancialsApplicable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub financials_raw_yahoo: Option<FinancialsRawYahoo>,
    pub canonical: CanonicalMetrics,
    pub shareholders: Shareholders,
    pub annual_reports: Vec<crate::models::AnnualReport>,
    pub company_summary: Option<String>,
    pub asset_profile: AssetProfile,
    pub financial_company_type: crate::models::FinancialCompanyType,
    pub stock_analysis: StockAnalysis,
    pub management_analysis: ManagementAnalysis,
    pub sector_analysis: SectorAnalysisDetail,
    pub peer_analysis: PeerAnalysis,
    pub news: Vec<NewsItem>,
    pub sector_news: Vec<NewsItem>,
    pub report_insights: ReportInsights,
    pub structured_sections: StructuredResearchSections,
    pub score_breakdown: ScoreBreakdown,
    pub company_overview: CompanyOverview,
    pub fundamental_analysis: FundamentalAnalysis,
    pub valuation_analysis: ValuationAnalysis,
    pub technical_analysis: TechnicalAnalysis,
    pub technical_entry: TechnicalEntrySignal,
    pub research_rating: ResearchRating,
    pub research_summary: ResearchSummary,
    pub financial_strength_audit: FinancialStrengthAudit,
    pub market_signals: MarketSignals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_metrics: Option<BankingMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screener_enrichment: Option<ScreenerMetricSnapshot>,
}

/// Fetch and analyze one India-listed symbol (Yahoo `*.NS` or `*.BO`).
///
/// When `resolve_ctx` is `None`, NSE membership is loaded from `data/EQUITY_L.csv` if present.
pub async fn build_research_report(
    raw_symbol: &str,
    screener: Option<ScreenerMetricSnapshot>,
    resolve_ctx: Option<&IndiaSymbolContext>,
) -> Result<ResearchReport> {
    let default_ctx = default_india_symbol_context();
    let ctx = resolve_ctx.unwrap_or(&default_ctx);
    let symbol = resolve_india_symbol(raw_symbol, ctx)?;
    let retrieved_at = chrono::Utc::now().to_rfc3339();

    let (
        price,
        mut financials,
        shareholders,
        officer_pay,
        profile,
        statements,
        chart,
        mut market_signals,
    ) = tokio::join!(
        fetch_price(&symbol),
        fetch_financials(&symbol),
        fetch_shareholders(&symbol),
        fetch_officer_pay(&symbol),
        fetch_asset_profile(&symbol),
        fetch_statement_bundle(&symbol),
        fetch_chart_history(&symbol, "5y"),
        fetch_market_signals(&symbol),
    );

    let (insider_txs, insider_drop_note) =
        sanitize_india_insider_transactions(&symbol, price, market_signals.insider_transactions);
    market_signals.insider_transactions = insider_txs;
    if let Some(note) = insider_drop_note {
        if !market_signals.narrative.contains("insider") {
            market_signals.narrative = format!("{} {}", market_signals.narrative, note);
        }
    }

    let news = fetch_company_news(
        &symbol,
        profile.long_name.as_deref(),
        profile.sector.as_deref(),
        profile.industry.as_deref(),
        8,
    )
    .await;

    let sector_key = profile
        .industry
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            profile
                .sector
                .as_deref()
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("NSE listed equities");
    let sector_news = fetch_sector_news(sector_key).await;

    let company_type = crate::financial_company::classify_financial_company(&symbol, &profile);
    if company_type.is_lender() {
        financials.industrial_yahoo_fields_analysis_applicable = false;
    }

    let peer_symbols = discover_nse_peer_symbols(
        &symbol,
        profile.industry.as_deref(),
        profile.sector.as_deref(),
        8,
    )
    .await;

    let mut batch = vec![symbol.clone()];
    batch.extend(peer_symbols);
    let quote_rows = fetch_peer_quotes(&batch).await;

    let subject_quote = quote_rows
        .iter()
        .find(|r| r.symbol.eq_ignore_ascii_case(&symbol))
        .cloned()
        .unwrap_or_else(|| PeerQuote {
            symbol: symbol.clone(),
            short_name: profile.long_name.clone(),
            price,
            pe_ratio: financials.pe_ratio,
            forward_pe: financials.forward_pe,
            price_to_book: financials.price_to_book,
            price_to_sales: financials.price_to_sales,
            ev_to_ebitda: financials.yahoo_ev_to_ebitda.or_else(|| {
                ev_to_ebitda(
                    resolve_enterprise_value(
                        financials.enterprise_value,
                        financials.market_cap,
                        financials.total_debt,
                        financials.total_cash,
                    ),
                    financials.ebitda,
                )
            }),
            ev_to_sales: ev_to_sales(
                resolve_enterprise_value(
                    financials.enterprise_value,
                    financials.market_cap,
                    financials.total_debt,
                    financials.total_cash,
                ),
                financials.revenue,
            ),
            market_cap: financials.market_cap,
            revenue: financials.revenue,
            revenue_growth: financials.revenue_growth,
            pat_growth: financials.earnings_growth,
            ebitda: financials.ebitda,
            ebitda_margin: if financials.revenue > 0.0 && financials.ebitda > 0.0 {
                Some(financials.ebitda / financials.revenue)
            } else {
                None
            },
            return_on_equity: financials.return_on_equity,
            return_on_capital_employed: financials.return_on_capital_employed,
            return_on_assets: financials.return_on_assets,
            profit_margins: financials.profit_margins,
            debt_to_equity: financials.debt_to_equity,
            free_cashflow: financials.free_cashflow,
            officer_pay,
            average_volume_10_day: financials.average_volume_10_day,
            dividend_yield: financials.dividend_yield,
            industrial_metrics_analysis_applicable: !company_type.is_lender(),
        });

    let mut peer_only: Vec<PeerQuote> = quote_rows
        .into_iter()
        .filter(|r| !r.symbol.eq_ignore_ascii_case(&symbol))
        .collect();
    if company_type.is_lender() {
        for p in peer_only.iter_mut() {
            p.industrial_metrics_analysis_applicable = false;
        }
    }

    let canonical = crate::canonical::build_canonical_for(&statements, &financials, company_type);

    let inc_desc = crate::statements::income_annual_desc(&statements);
    let rev_levels: Vec<f64> = crate::statements::income_annual_asc(&statements)
        .iter()
        .map(|r| r.revenue)
        .collect();
    let yahoo_rev_g = financials.revenue_growth.filter(|x| x.abs() > 1e-9);
    let rev_check = crate::series_integrity::check_level_series(&rev_levels, yahoo_rev_g);
    let annual_reports =
        crate::series_integrity::annual_reports_from_income(
            &inc_desc,
            &rev_check,
            !company_type.is_lender(),
        );

    let bank_metrics = bank_metrics_for(&symbol);
    let coverage = crate::financial_company::build_data_coverage(
        company_type,
        &financials,
        &canonical,
        bank_metrics.as_ref(),
    );
    let fundamental_analysis = crate::fundamental_analysis::build_fundamental_analysis_for(
        &statements,
        &financials,
        company_type,
        bank_metrics.as_ref(),
        Some(&canonical),
    );
    let financial_strength_audit = crate::financial_strength_audit::build_financial_strength_audit_for(
        &statements,
        &financials,
        &profile,
        screener.as_ref(),
        bank_metrics.as_ref(),
        company_type,
        coverage,
    );

    let statement_fcf = if company_type.is_lender() {
        None
    } else {
        report_fcf(financials.free_cashflow, &statements)
    };
    let mut stock_analysis = crate::analysis::compute_stock_analysis_for(
        price,
        &financials,
        &annual_reports,
        statement_fcf,
        &canonical,
        company_type,
    );
    let management_analysis = crate::analysis::compute_management_analysis_for(
        officer_pay,
        financials.revenue,
        profile.long_business_summary.as_deref(),
        company_type,
    );
    let mut sector_analysis = compute_sector_analysis_for(
        profile.sector.as_deref(),
        profile.industry.as_deref(),
        &sector_news,
        company_type,
    );
    let mut peer_analysis = compute_peer_analysis_for(
        &subject_quote,
        &peer_only,
        profile.industry.as_deref(),
        profile.long_business_summary.as_deref(),
        company_type,
    );
    if company_type.is_lender() {
        let (comp, direct, bank_c) = crate::financial_company::peer_comparability_for(company_type);
        peer_analysis.peer_comparability = comp;
        peer_analysis.direct_peer_comparability = direct;
        peer_analysis.bank_comparability = bank_c;
        peer_analysis.peer_set_kind = if company_type.is_nbfc_family() {
            "direct_nbfc".to_string()
        } else {
            "banks".to_string()
        };
    }
    if !company_type.is_lender() {
        let sector_name = profile
            .sector
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Unclassified");
        let inputs = crate::sector_research::sector_inputs_from_quotes(
            sector_name,
            &subject_quote,
            &peer_only,
        );
        sector_analysis.research =
            Some(crate::sector_research::compute_sector_research_profile(&inputs));
    }
    let mut report_insights = compute_report_insights(
        &stock_analysis,
        &management_analysis,
        &peer_analysis,
        &financials,
        company_type,
        Some(&financial_strength_audit.data_coverage),
    );
    if canonical.quarterly_statement_stale && !canonical.quarterly_statement_stale_note.is_empty() {
        report_insights
            .data_notes
            .insert(0, canonical.quarterly_statement_stale_note.clone());
    }
    let (mut structured_sections, mut score_breakdown) = build_structured_sections(
        &symbol,
        profile.long_name.as_deref(),
        &financials,
        &stock_analysis,
        &management_analysis,
        &sector_analysis,
        &subject_quote,
        &peer_only,
        &shareholders,
        &statements,
        &financial_strength_audit,
        &canonical,
        company_type,
    );

    let company_overview = build_company_overview(
        &symbol,
        &profile,
        &financials,
        price,
        &statements,
        company_type,
    );
    let valuation_analysis = crate::valuation_analysis::build_valuation_analysis_for(
        price,
        &financials,
        &chart,
        &statements,
        &subject_quote,
        &peer_only,
        &fundamental_analysis,
        &peer_analysis.peer_comparability,
        company_type,
        financial_strength_audit.data_coverage.recommendation_gated,
    );
    if company_type.is_lender() {
        stock_analysis.valuation_label = valuation_analysis.valuation_label.clone();
        structured_sections.valuation = valuation_analysis.valuation_label.clone();
    }
    let technical_analysis = build_technical_analysis(price, &financials, &chart);
    let technical_entry = build_technical_entry_signal(
        &technical_analysis,
        &fundamental_analysis,
        &valuation_analysis,
    );
    let research_rating = crate::stock_scoring::build_research_rating_for(
        &financials,
        &fundamental_analysis,
        &valuation_analysis,
        &technical_analysis,
        &technical_entry,
        &peer_only,
        &shareholders,
        &structured_sections.risks,
        &financial_strength_audit,
        &market_signals,
        &canonical,
        company_type,
    );
    score_breakdown.screening_score = score_breakdown.screening_score.max(
        score_breakdown.total_score.unwrap_or(0.0),
    );
    let screening_model = score_breakdown.screening_score;
    score_breakdown.score_provisional = research_rating.overall_score_provisional;
    score_breakdown.total_score = research_rating.overall_score;
    if research_rating.overall_score_provisional {
        let avail = score_breakdown
            .available_dimension_score
            .or(research_rating.provisional_screening_score)
            .unwrap_or(0.0);
        let crit = financial_strength_audit.data_coverage.critical_pct;
        score_breakdown.provisional_screening_score = Some(avail);
        score_breakdown.critical_coverage_pct = Some(crit);
        score_breakdown.coverage_adjusted_score = None;
        score_breakdown.interpretation = format!(
            "Available-metric score {:.1}/100. Critical coverage {:.0}%. Investment rating withheld. Screening model (known factors) {:.1}.",
            avail,
            crit,
            screening_model
        );
        structured_sections.final_recommendation = format!(
            "Available-metric score {:.1}/100 — {}. Critical coverage {:.0}%. Not comparable to a well-covered industrial score.",
            avail, research_rating.rating_label, crit
        );
    } else {
        score_breakdown.provisional_screening_score = research_rating.provisional_screening_score;
        score_breakdown.coverage_adjusted_score = score_breakdown.available_dimension_score;
        let overall = research_rating.overall_score.unwrap_or(0.0);
        score_breakdown.interpretation = format!(
            "Investment score {:.1}/100 ({}); available-metric score {:.1}/100; screening model {:.1}/100.",
            overall,
            research_rating.rating_label,
            score_breakdown.available_dimension_score.unwrap_or(overall),
            screening_model
        );
        structured_sections.final_recommendation = format!(
            "Investment score {:.1}/100 — {}.",
            overall, research_rating.rating_label
        );
    }
    let research_summary = crate::research_summary::build_research_summary_for(
        &fundamental_analysis,
        &valuation_analysis,
        &technical_analysis,
        &technical_entry,
        &research_rating,
        &structured_sections.risks,
        &financial_strength_audit,
        &market_signals,
        company_type,
        &financials,
        &canonical,
        &company_overview,
    );

    Ok(ResearchReport {
        symbol: symbol.clone(),
        long_name: profile.long_name.clone(),
        price,
        retrieved_at,
        financials: financials.clone(),
        financials_applicable: Some(financials.applicable_view()),
        financials_raw_yahoo: Some(financials.raw_yahoo_view()),
        canonical,
        shareholders,
        annual_reports,
        company_summary: profile.long_business_summary.clone(),
        asset_profile: profile.clone(),
        financial_company_type: company_type,
        stock_analysis,
        management_analysis,
        sector_analysis,
        peer_analysis,
        news,
        sector_news,
        report_insights,
        structured_sections,
        score_breakdown,
        company_overview,
        fundamental_analysis,
        valuation_analysis,
        technical_analysis,
        technical_entry,
        research_rating,
        research_summary,
        financial_strength_audit,
        market_signals,
        bank_metrics,
        screener_enrichment: screener,
    })
}
