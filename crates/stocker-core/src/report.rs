use serde::Serialize;

use crate::analysis::{
    build_structured_sections, compute_management_analysis, compute_peer_analysis, compute_report_insights,
    compute_sector_analysis, compute_stock_analysis,
};
use crate::error::Result;
use crate::fetcher::{
    discover_nse_peer_symbols, fetch_annual_reports, fetch_asset_profile, fetch_chart_history,
    fetch_company_news, fetch_financials, fetch_market_signals, fetch_officer_pay, fetch_peer_quotes,
    fetch_price, fetch_sector_news, fetch_shareholders, fetch_statement_bundle,
};
use crate::bank_metrics::bank_metrics_for;
use crate::financial_strength_audit::build_financial_strength_audit;
use crate::fundamental_analysis::build_fundamental_analysis;
use crate::models::{
    AssetProfile, BankingMetrics, CompanyOverview, Financials, FinancialStrengthAudit, FundamentalAnalysis,
    ManagementAnalysis, MarketSignals, NewsItem, PeerAnalysis, PeerQuote, ReportInsights,
    ResearchRating, ResearchSummary, ScoreBreakdown, ScreenerMetricSnapshot, SectorAnalysisDetail,
    Shareholders, StockAnalysis, StructuredResearchSections, TechnicalAnalysis,
    TechnicalEntrySignal, ValuationAnalysis,
};
use crate::research_summary::{build_company_overview, build_research_summary};
use crate::stock_scoring::build_research_rating;
use crate::technical_analysis::build_technical_analysis;
use crate::technical_entry_signal::build_technical_entry_signal;
use crate::valuation_analysis::build_valuation_analysis;
use crate::symbol::{default_india_symbol_context, resolve_india_symbol, IndiaSymbolContext};

#[derive(Debug, Serialize)]
pub struct ResearchReport {
    pub symbol: String,
    pub long_name: Option<String>,
    pub price: f64,
    pub financials: Financials,
    pub shareholders: Shareholders,
    pub annual_reports: Vec<crate::models::AnnualReport>,
    pub company_summary: Option<String>,
    pub asset_profile: AssetProfile,
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

    let (
        price,
        mut financials,
        shareholders,
        annual_reports,
        officer_pay,
        profile,
        statements,
        chart,
        market_signals,
    ) = tokio::join!(
        fetch_price(&symbol),
        fetch_financials(&symbol),
        fetch_shareholders(&symbol),
        fetch_annual_reports(&symbol),
        fetch_officer_pay(&symbol),
        fetch_asset_profile(&symbol),
        fetch_statement_bundle(&symbol),
        fetch_chart_history(&symbol, "5y"),
        fetch_market_signals(&symbol),
    );

    if financials.net_income <= 0.0 {
        if let Some(latest_income) = annual_reports
            .iter()
            .max_by(|a, b| a.date.cmp(&b.date))
            .map(|r| r.net_income)
            .filter(|v| *v > 0.0)
        {
            financials.net_income = latest_income;
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
            ev_to_ebitda: if financials.enterprise_value > 0.0 && financials.ebitda > 0.0 {
                Some(financials.enterprise_value / financials.ebitda)
            } else if financials.ebitda > 0.0 && financials.market_cap > 0.0 {
                Some(financials.market_cap / financials.ebitda)
            } else {
                None
            },
            ev_to_sales: if financials.enterprise_value > 0.0 && financials.revenue > 0.0 {
                Some(financials.enterprise_value / financials.revenue)
            } else {
                None
            },
            market_cap: financials.market_cap,
            revenue: financials.revenue,
            revenue_growth: financials.revenue_growth,
            pat_growth: financials.earnings_growth,
            ebitda: financials.ebitda,
            ebitda_margin: if financials.revenue > 0.0 && financials.ebitda > 0.0 {
                financials.ebitda / financials.revenue
            } else {
                0.0
            },
            return_on_equity: financials.return_on_equity,
            return_on_capital_employed: financials.return_on_capital_employed,
            return_on_assets: financials.return_on_assets,
            profit_margins: financials.profit_margins,
            debt_to_equity: financials.debt_to_equity,
            free_cashflow: financials.free_cashflow,
            officer_pay,
            average_volume_10_day: financials.average_volume_10_day,
        });

    let peer_only: Vec<PeerQuote> = quote_rows
        .into_iter()
        .filter(|r| !r.symbol.eq_ignore_ascii_case(&symbol))
        .collect();

    let fundamental_analysis =
        build_fundamental_analysis(&statements, &financials, &annual_reports);
    let bank_metrics = bank_metrics_for(&symbol);
    let financial_strength_audit = build_financial_strength_audit(
        &statements,
        &financials,
        &profile,
        screener.as_ref(),
        bank_metrics.as_ref(),
    );

    let stock_analysis = compute_stock_analysis(price, &financials, &annual_reports);
    let management_analysis = compute_management_analysis(
        officer_pay,
        financials.revenue,
        profile.long_business_summary.as_deref(),
    );
    let sector_analysis = compute_sector_analysis(
        profile.sector.as_deref(),
        profile.industry.as_deref(),
        &sector_news,
    );
    let peer_analysis = compute_peer_analysis(&subject_quote, &peer_only);
    let report_insights = compute_report_insights(
        &stock_analysis,
        &management_analysis,
        &peer_analysis,
        &financials,
    );
    let (structured_sections, score_breakdown) = build_structured_sections(
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
    );

    let company_overview = build_company_overview(
        &symbol,
        &profile,
        &financials,
        price,
        &statements,
    );
    let valuation_analysis = build_valuation_analysis(
        price,
        &financials,
        &chart,
        &statements,
        &subject_quote,
        &peer_only,
        &fundamental_analysis,
    );
    let technical_analysis = build_technical_analysis(price, &financials, &chart);
    let technical_entry = build_technical_entry_signal(
        &technical_analysis,
        &fundamental_analysis,
        &valuation_analysis,
    );
    let research_rating = build_research_rating(
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
    );
    let research_summary = build_research_summary(
        &fundamental_analysis,
        &valuation_analysis,
        &technical_analysis,
        &technical_entry,
        &research_rating,
        &structured_sections.risks,
        &financial_strength_audit,
        &market_signals,
    );

    Ok(ResearchReport {
        symbol: symbol.clone(),
        long_name: profile.long_name.clone(),
        price,
        financials: financials.clone(),
        shareholders,
        annual_reports,
        company_summary: profile.long_business_summary.clone(),
        asset_profile: profile.clone(),
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
