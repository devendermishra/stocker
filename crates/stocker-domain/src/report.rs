use serde::{Deserialize, Serialize};

use crate::models::{
    AnnualReport, AssetProfile, BankingMetrics, CanonicalMetrics, CompanyOverview,
    FinancialCompanyType, FinancialStrengthAudit, Financials, FinancialsApplicable,
    FinancialsRawYahoo, FundamentalAnalysis, ManagementAnalysis, MarketSignals, NewsItem,
    PeerAnalysis, ReportInsights, ResearchRating, ResearchSummary, ScoreBreakdown,
    ScreenerMetricSnapshot, SectorAnalysisDetail, Shareholders, StockAnalysis,
    StructuredResearchSections, TechnicalAnalysis, TechnicalEntrySignal, ValuationAnalysis,
};

/// Research report produced from already-fetched market data.
///
/// `retrieved_at` is set by the caller. This type does not read the clock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReport {
    pub symbol: String,
    pub long_name: Option<String>,
    pub price: f64,
    /// ISO-8601 UTC time of the live market-data fetch. Empty when a saved report omitted it.
    #[serde(default)]
    pub retrieved_at: String,
    pub financials: Financials,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub financials_applicable: Option<FinancialsApplicable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub financials_raw_yahoo: Option<FinancialsRawYahoo>,
    #[serde(default)]
    pub canonical: CanonicalMetrics,
    pub shareholders: Shareholders,
    pub annual_reports: Vec<AnnualReport>,
    pub company_summary: Option<String>,
    pub asset_profile: AssetProfile,
    #[serde(default)]
    pub financial_company_type: FinancialCompanyType,
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
    #[serde(default)]
    pub financial_strength_audit: FinancialStrengthAudit,
    #[serde(default)]
    pub market_signals: MarketSignals,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank_metrics: Option<BankingMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screener_enrichment: Option<ScreenerMetricSnapshot>,
}
