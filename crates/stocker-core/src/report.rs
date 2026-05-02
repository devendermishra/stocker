use serde::Serialize;

use crate::analysis::{
    build_structured_sections, compute_management_analysis, compute_peer_analysis, compute_report_insights,
    compute_sector_analysis, compute_stock_analysis,
};
use crate::error::Result;
use crate::fetcher::{
    discover_nse_peer_symbols, fetch_annual_reports, fetch_asset_profile, fetch_company_news,
    fetch_financials, fetch_officer_pay, fetch_peer_quotes, fetch_price, fetch_sector_news,
    fetch_shareholders,
};
use crate::models::{
    AssetProfile, Financials, ManagementAnalysis, NewsItem, PeerAnalysis, PeerQuote, ReportInsights,
    ScoreBreakdown, SectorAnalysisDetail, Shareholders, StockAnalysis, StructuredResearchSections,
};
use crate::symbol::normalize_nse_symbol;

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
}

/// Fetch and analyze one NSE symbol (Yahoo `*.NS`).
pub async fn build_research_report(raw_symbol: &str) -> Result<ResearchReport> {
    let symbol = normalize_nse_symbol(raw_symbol)?;

    let (price, mut financials, shareholders, annual_reports, officer_pay, profile) = tokio::join!(
        fetch_price(&symbol),
        fetch_financials(&symbol),
        fetch_shareholders(&symbol),
        fetch_annual_reports(&symbol),
        fetch_officer_pay(&symbol),
        fetch_asset_profile(&symbol),
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
            ev_to_ebitda: if financials.ebitda > 0.0 && financials.market_cap > 0.0 {
                Some(financials.market_cap / financials.ebitda)
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
            return_on_capital_employed: None,
            profit_margins: financials.profit_margins,
            debt_to_equity: financials.debt_to_equity,
            free_cashflow: financials.free_cashflow,
            officer_pay,
        });

    let peer_only: Vec<PeerQuote> = quote_rows
        .into_iter()
        .filter(|r| !r.symbol.eq_ignore_ascii_case(&symbol))
        .collect();

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
    })
}
