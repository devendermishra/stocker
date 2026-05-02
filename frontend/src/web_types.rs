//! JSON view models for the WASM client (API returns the same shape as `stocker_core` JSON).

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Financials {
    pub revenue: f64,
    pub net_income: f64,
    pub pe_ratio: f64,
    pub forward_pe: f64,
    pub total_debt: f64,
    pub ebitda: f64,
    pub profit_margins: f64,
    pub return_on_equity: f64,
    pub debt_to_equity: f64,
    pub free_cashflow: f64,
    pub operating_cashflow: f64,
    pub market_cap: f64,
    pub dividend_yield: f64,
    pub payout_ratio: f64,
    pub revenue_growth: f64,
    pub earnings_growth: f64,
    pub regular_market_change_percent: f64,
    pub previous_close: f64,
    pub fifty_two_week_high: f64,
    pub fifty_two_week_low: f64,
    pub beta: f64,
    pub ex_dividend_date: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Shareholders {
    pub insiders_percent: f64,
    pub institutions_percent: f64,
    pub promoter_percent: Option<f64>,
    pub fii_percent: Option<f64>,
    pub dii_percent: Option<f64>,
    pub mutual_fund_percent: Option<f64>,
    pub retail_percent: Option<f64>,
    pub pledge_percent: Option<f64>,
    pub insider_activity_note: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnnualReport {
    pub date: String,
    pub revenue: f64,
    pub net_income: f64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub published_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetProfile {
    pub sector: Option<String>,
    pub industry: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StockAnalysis {
    pub quality_score: f64,
    pub valuation_label: String,
    pub margin_trend: String,
    pub narrative: String,
    pub fcf_yield_pct: Option<f64>,
    pub earnings_yield_pct: Option<f64>,
    pub revenue_cagr_trailing_3y_pct: Option<f64>,
    pub net_income_cagr_trailing_3y_pct: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ManagementAnalysis {
    pub pay_vs_revenue_score: f64,
    pub tone_score: f64,
    pub tone_label: String,
    pub narrative: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SectorAnalysis {
    pub outlook_narrative: String,
    pub sector_news_summary: String,
    pub sample_headlines: Vec<String>,
    pub sector_headline_themes: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct PeerQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub price: f64,
    pub pe_ratio: f64,
    pub ev_to_ebitda: Option<f64>,
    pub market_cap: f64,
    pub revenue_growth: f64,
    pub pat_growth: f64,
    pub ebitda_margin: f64,
    pub return_on_equity: f64,
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: f64,
    pub profit_margins: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PeerAnalysis {
    pub peers: Vec<PeerQuote>,
    pub subject_percentile_pe: Option<f64>,
    pub subject_percentile_roe: Option<f64>,
    pub subject_percentile_quality: Option<f64>,
    pub narrative: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportInsights {
    pub executive_summary: String,
    pub strengths: Vec<String>,
    pub watch_items: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResearchReport {
    pub symbol: String,
    pub long_name: Option<String>,
    pub price: f64,
    pub financials: Financials,
    pub shareholders: Shareholders,
    pub annual_reports: Vec<AnnualReport>,
    pub company_summary: Option<String>,
    pub asset_profile: AssetProfile,
    pub stock_analysis: StockAnalysis,
    pub management_analysis: ManagementAnalysis,
    pub sector_analysis: SectorAnalysis,
    pub peer_analysis: PeerAnalysis,
    pub news: Vec<NewsItem>,
    pub sector_news: Vec<NewsItem>,
    pub report_insights: ReportInsights,
    pub structured_sections: StructuredResearchSections,
    pub score_breakdown: ScoreBreakdown,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct CashFlowQuality {
    pub pat: f64,
    pub cfo: f64,
    pub ebitda: f64,
    pub free_cashflow: f64,
    pub capex_estimate: f64,
    pub pat_vs_cfo_delta: Option<f64>,
    pub cfo_vs_ebitda_ratio: Option<f64>,
    pub cash_conversion_ratio: Option<f64>,
    pub capex_requirement_ratio: Option<f64>,
    pub narrative: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MonitorableItem {
    pub area: String,
    pub what_to_track: String,
    pub status: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RiskItem {
    pub risk: String,
    pub severity: String,
    pub note: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RiskBuckets {
    pub business_risks: Vec<RiskItem>,
    pub financial_risks: Vec<RiskItem>,
    pub management_risks: Vec<RiskItem>,
    pub valuation_risks: Vec<RiskItem>,
    pub regulatory_risks: Vec<RiskItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PeerComparisonRow {
    pub metric: String,
    pub company_label: String,
    pub peer_1_label: String,
    pub peer_2_label: String,
    pub peer_3_label: String,
    pub company: Option<f64>,
    pub peer_1: Option<f64>,
    pub peer_2: Option<f64>,
    pub peer_3: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScenarioAnalysis {
    pub base_case: String,
    pub upside_case: String,
    pub downside_case: String,
    pub capital_impairment_guardrail: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StructuredResearchSections {
    pub company_overview: String,
    pub business_model: String,
    pub industry_opportunity: String,
    pub competitive_advantage: String,
    pub management_quality: String,
    pub financial_performance: String,
    pub balance_sheet_strength: String,
    pub cash_flow_quality: CashFlowQuality,
    pub valuation: String,
    pub peer_comparison: Vec<PeerComparisonRow>,
    pub growth_triggers: Vec<String>,
    pub risks: RiskBuckets,
    pub scenario_analysis: ScenarioAnalysis,
    pub entry_exit_strategy: String,
    pub key_monitorables: Vec<MonitorableItem>,
    pub final_recommendation: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScoreBreakdown {
    pub business_quality: f64,
    pub industry_tailwind: f64,
    pub financial_strength: f64,
    pub management_quality: f64,
    pub valuation_comfort: f64,
    pub growth_triggers: f64,
    pub risk_reward: f64,
    pub total_score: f64,
    pub interpretation: String,
}
