//! JSON view models for the WASM client (API returns the same shape as `stocker_core` JSON).

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Financials {
    pub revenue: f64,
    pub net_income: f64,
    pub pe_ratio: f64,
    pub forward_pe: f64,
    pub total_debt: f64,
    pub ebitda: f64,
    pub profit_margins: f64,
    pub gross_margins: f64,
    pub operating_margins: f64,
    pub ebitda_margins: f64,
    pub return_on_equity: f64,
    pub return_on_assets: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: f64,
    pub free_cashflow: f64,
    pub operating_cashflow: f64,
    pub shares_outstanding: f64,
    pub market_cap: f64,
    pub enterprise_value: f64,
    pub total_cash: f64,
    pub book_value: f64,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub trailing_eps: f64,
    pub forward_eps: f64,
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
    pub regular_market_volume: f64,
    pub average_volume_10_day: f64,
}

impl Default for Financials {
    fn default() -> Self {
        Self {
            revenue: 0.0,
            net_income: 0.0,
            pe_ratio: 0.0,
            forward_pe: 0.0,
            total_debt: 0.0,
            ebitda: 0.0,
            profit_margins: 0.0,
            gross_margins: 0.0,
            operating_margins: 0.0,
            ebitda_margins: 0.0,
            return_on_equity: 0.0,
            return_on_assets: None,
            return_on_capital_employed: None,
            debt_to_equity: 0.0,
            free_cashflow: 0.0,
            operating_cashflow: 0.0,
            shares_outstanding: 0.0,
            market_cap: 0.0,
            enterprise_value: 0.0,
            total_cash: 0.0,
            book_value: 0.0,
            price_to_book: 0.0,
            price_to_sales: 0.0,
            trailing_eps: 0.0,
            forward_eps: 0.0,
            dividend_yield: 0.0,
            payout_ratio: 0.0,
            revenue_growth: 0.0,
            earnings_growth: 0.0,
            regular_market_change_percent: 0.0,
            previous_close: 0.0,
            fifty_two_week_high: 0.0,
            fifty_two_week_low: 0.0,
            beta: 0.0,
            ex_dividend_date: None,
            regular_market_volume: 0.0,
            average_volume_10_day: 0.0,
        }
    }
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

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AssetProfile {
    pub long_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub long_business_summary: Option<String>,
    pub website: Option<String>,
    pub country: Option<String>,
    pub exchange: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StockAnalysis {
    pub quality_score: f64,
    pub valuation_label: String,
    pub revenue_cagr_full_series_pct: Option<f64>,
    pub net_income_cagr_full_series_pct: Option<f64>,
    pub revenue_cagr_trailing_3y_pct: Option<f64>,
    pub net_income_cagr_trailing_3y_pct: Option<f64>,
    pub revenue_cagr_trailing_5y_pct: Option<f64>,
    pub net_income_cagr_trailing_5y_pct: Option<f64>,
    pub margin_trend: String,
    pub narrative: String,
    pub fcf_yield_pct: Option<f64>,
    pub earnings_yield_pct: Option<f64>,
    pub peg_style_note: String,
    pub price_in_52w_range: Option<f64>,
    pub distance_from_52w_high_pct: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ManagementAnalysis {
    pub pay_vs_revenue_score: f64,
    pub tone_score: f64,
    pub tone_label: String,
    pub narrative: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SectorAnalysis {
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub outlook_narrative: String,
    pub sector_news_summary: String,
    pub sample_headlines: Vec<String>,
    pub sector_headline_themes: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct PeerQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub price: f64,
    pub pe_ratio: f64,
    pub forward_pe: f64,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    pub ev_to_sales: Option<f64>,
    pub market_cap: f64,
    pub revenue: f64,
    pub revenue_growth: f64,
    pub pat_growth: f64,
    pub ebitda: f64,
    pub ebitda_margin: f64,
    pub return_on_equity: f64,
    pub return_on_capital_employed: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub debt_to_equity: f64,
    pub profit_margins: f64,
    pub free_cashflow: f64,
    pub officer_pay: f64,
    pub average_volume_10_day: f64,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct PeerAnalysis {
    pub peers: Vec<PeerQuote>,
    pub subject_quality_score: f64,
    pub subject_pay_vs_revenue_score: f64,
    pub subject_percentile_pe: Option<f64>,
    pub subject_percentile_roe: Option<f64>,
    pub subject_percentile_quality: Option<f64>,
    pub subject_percentile_pay_efficiency: Option<f64>,
    pub narrative: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportInsights {
    pub executive_summary: String,
    pub strengths: Vec<String>,
    pub watch_items: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FundamentalSection {
    pub interpretation: String,
    pub flags: Vec<String>,
    pub confidence: String,
    pub lines: Vec<(String, String)>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FundamentalAnalysis {
    pub growth: FundamentalSection,
    pub profitability: FundamentalSection,
    pub balance_sheet: FundamentalSection,
    pub cash_flow: FundamentalSection,
    pub efficiency: FundamentalSection,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct HistoricalMultiples {
    pub median_pe_3y: Option<f64>,
    pub median_pe_5y: Option<f64>,
    pub median_pb_3y: Option<f64>,
    pub median_pb_5y: Option<f64>,
    pub median_ev_ebitda_3y: Option<f64>,
    pub median_ev_ebitda_5y: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct PeerValuationCompare {
    pub median_pe: Option<f64>,
    pub median_pb: Option<f64>,
    pub median_ev_ebitda: Option<f64>,
    pub median_ps: Option<f64>,
    pub median_roe: Option<f64>,
    pub median_roce: Option<f64>,
    pub interpretation: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct EarningsBasedValue {
    pub input_eps: f64,
    pub input_growth_rate: f64,
    pub fair_pe: f64,
    pub margin_of_safety: f64,
    pub fair_value: f64,
    pub upside_downside_pct: Option<f64>,
    pub bull_value: f64,
    pub base_value: f64,
    pub bear_value: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct ValuationAnalysis {
    pub pe: f64,
    pub forward_pe: f64,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    pub ev_to_sales: Option<f64>,
    pub peg_ratio: Option<f64>,
    pub dividend_yield: f64,
    pub earnings_yield_pct: Option<f64>,
    pub fcf_yield_pct: Option<f64>,
    pub market_cap_to_sales: Option<f64>,
    pub historical: HistoricalMultiples,
    pub historical_classification: String,
    pub peer_compare: PeerValuationCompare,
    pub earnings_based: EarningsBasedValue,
    pub valuation_label: String,
    pub peer_value_read: String,
    pub confidence: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalTrend {
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub trend_label: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalMomentum {
    pub rsi_14: Option<f64>,
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_histogram: Option<f64>,
    pub rsi_label: String,
    pub roc_1m_pct: Option<f64>,
    pub roc_3m_pct: Option<f64>,
    pub roc_6m_pct: Option<f64>,
    pub roc_1y_pct: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalVolatility {
    pub dist_from_high_pct: Option<f64>,
    pub vol_1y_ann_pct: Option<f64>,
    pub max_drawdown_1y_pct: Option<f64>,
    pub atr_14: Option<f64>,
    pub note: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalVolume {
    pub volume_breakout: bool,
    pub interpretation: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TechnicalAnalysis {
    pub trend: TechnicalTrend,
    pub momentum: TechnicalMomentum,
    pub volatility: TechnicalVolatility,
    pub volume: TechnicalVolume,
    pub confidence: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TechnicalEntrySignal {
    pub zone: String,
    pub detail_label: String,
    pub rationale: Vec<String>,
    pub fundamental_vs_technical: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScoreExplanation {
    pub factor: String,
    pub impact: String,
    pub points: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct ResearchRating {
    pub growth_score: f64,
    pub quality_score: f64,
    pub valuation_score: f64,
    pub technical_score: f64,
    pub risk_score: f64,
    pub overall_score: f64,
    pub rating_label: String,
    pub fundamental_rating: String,
    pub valuation_rating: String,
    pub technical_rating: String,
    pub risk_rating: String,
    pub explain: Vec<ScoreExplanation>,
    pub cheap_fair_expensive_fundamental: String,
    pub technical_entry_label: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResearchSummary {
    pub business_quality: String,
    pub growth: String,
    pub valuation: String,
    pub technical_position: String,
    pub key_risks: String,
    pub final_view: String,
    pub key_positives: Vec<String>,
    pub key_negatives: Vec<String>,
    pub key_monitorables: Vec<String>,
    pub suggested_action: String,
    pub disclaimer: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CompanyOverview {
    pub company_name: String,
    pub ticker: String,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub market_cap: f64,
    pub current_price: f64,
    pub business_summary_short: String,
    pub website: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub latest_fiscal_year_end: Option<String>,
    pub latest_quarter_end: Option<String>,
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
    pub company_overview: CompanyOverview,
    pub fundamental_analysis: FundamentalAnalysis,
    pub valuation_analysis: ValuationAnalysis,
    pub technical_analysis: TechnicalAnalysis,
    pub technical_entry: TechnicalEntrySignal,
    pub research_rating: ResearchRating,
    pub research_summary: ResearchSummary,
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
