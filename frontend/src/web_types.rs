//! JSON view models for the WASM client (API returns the same shape as `stocker_core` JSON).

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Financials {
    pub revenue: f64,
    pub net_income: Option<f64>,
    #[serde(default)]
    pub net_income_to_common: Option<f64>,
    pub pe_ratio: f64,
    pub forward_pe: Option<f64>,
    pub total_debt: f64,
    pub ebitda: f64,
    pub profit_margins: f64,
    pub gross_margins: f64,
    pub operating_margins: f64,
    pub ebitda_margins: f64,
    pub return_on_equity: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub free_cashflow: Option<f64>,
    pub operating_cashflow: Option<f64>,
    pub shares_outstanding: f64,
    pub market_cap: f64,
    pub enterprise_value: Option<f64>,
    #[serde(default)]
    pub yahoo_ev_to_ebitda: Option<f64>,
    pub total_cash: f64,
    pub book_value: f64,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub trailing_eps: f64,
    pub forward_eps: Option<f64>,
    pub dividend_yield: f64,
    pub payout_ratio: f64,
    #[serde(default)]
    pub revenue_growth: Option<f64>,
    #[serde(default)]
    pub earnings_growth: Option<f64>,
    pub regular_market_change_percent: f64,
    pub previous_close: f64,
    pub fifty_two_week_high: f64,
    pub fifty_two_week_low: f64,
    pub beta: f64,
    pub ex_dividend_date: Option<String>,
    pub regular_market_volume: f64,
    pub average_volume_10_day: f64,
    #[serde(default = "default_true")]
    pub industrial_yahoo_fields_analysis_applicable: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Financials {
    fn default() -> Self {
        Self {
            revenue: 0.0,
            net_income: None,
            net_income_to_common: None,
            pe_ratio: 0.0,
            forward_pe: None,
            total_debt: 0.0,
            ebitda: 0.0,
            profit_margins: 0.0,
            gross_margins: 0.0,
            operating_margins: 0.0,
            ebitda_margins: 0.0,
            return_on_equity: None,
            return_on_assets: None,
            return_on_capital_employed: None,
            debt_to_equity: None,
            free_cashflow: None,
            operating_cashflow: None,
            shares_outstanding: 0.0,
            market_cap: 0.0,
            enterprise_value: None,
            yahoo_ev_to_ebitda: None,
            total_cash: 0.0,
            book_value: 0.0,
            price_to_book: 0.0,
            price_to_sales: 0.0,
            trailing_eps: 0.0,
            forward_eps: None,
            dividend_yield: 0.0,
            payout_ratio: 0.0,
            revenue_growth: None,
            earnings_growth: None,
            regular_market_change_percent: 0.0,
            previous_close: 0.0,
            fifty_two_week_high: 0.0,
            fifty_two_week_low: 0.0,
            beta: 0.0,
            ex_dividend_date: None,
            regular_market_volume: 0.0,
            average_volume_10_day: 0.0,
            industrial_yahoo_fields_analysis_applicable: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct FinancialsApplicable {
    pub pe_ratio: f64,
    pub price_to_book: f64,
    pub trailing_eps: f64,
    pub book_value: f64,
    pub dividend_yield: f64,
    pub market_cap: f64,
    pub beta: f64,
    pub return_on_equity: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub net_income: Option<f64>,
    pub earnings_growth: Option<f64>,
    pub forward_pe: Option<f64>,
    pub forward_eps: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct FinancialsRawYahoo {
    pub revenue: f64,
    pub revenue_growth: Option<f64>,
    pub ebitda: f64,
    pub gross_margins: f64,
    pub operating_margins: f64,
    pub ebitda_margins: f64,
    pub profit_margins: f64,
    pub price_to_sales: f64,
    pub enterprise_value: Option<f64>,
    pub yahoo_ev_to_ebitda: Option<f64>,
    pub free_cashflow: Option<f64>,
    pub operating_cashflow: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub current_ratio: Option<f64>,
    pub total_debt: f64,
    pub total_cash: f64,
    pub analysis_applicable: bool,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnnualReport {
    pub date: String,
    #[serde(default)]
    pub revenue: Option<f64>,
    #[serde(default)]
    pub yahoo_total_revenue_raw: f64,
    #[serde(default = "default_true")]
    pub revenue_represents_sales: bool,
    pub net_income: f64,
    #[serde(default)]
    pub net_income_yahoo_row: Option<String>,
    #[serde(default)]
    pub pat_scope: String,
    #[serde(default)]
    pub series_warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub published_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StockAnalysis {
    #[serde(alias = "quality_score")]
    pub data_quality_score: f64,
    #[serde(default)]
    pub quality_score_kind: String,
    pub valuation_label: String,
    pub revenue_cagr_full_series_pct: Option<f64>,
    pub net_income_cagr_full_series_pct: Option<f64>,
    pub revenue_cagr_trailing_3y_pct: Option<f64>,
    pub net_income_cagr_trailing_3y_pct: Option<f64>,
    pub revenue_cagr_trailing_5y_pct: Option<f64>,
    pub net_income_cagr_trailing_5y_pct: Option<f64>,
    #[serde(default)]
    pub margin_trend: Option<String>,
    pub narrative: String,
    pub fcf_yield_pct: Option<f64>,
    pub earnings_yield_pct: Option<f64>,
    pub peg_style_note: String,
    pub price_in_52w_range: Option<f64>,
    pub distance_from_52w_high_pct: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagementAnalysis {
    #[serde(default)]
    pub pay_vs_revenue_score: Option<f64>,
    pub tone_score: f64,
    pub tone_label: String,
    pub narrative: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct SectorAnalysis {
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub outlook_narrative: String,
    pub sector_news_summary: String,
    pub sample_headlines: Vec<String>,
    pub sector_headline_themes: String,
    pub research: Option<crate::sectors_api::SectorResearchProfileView>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct PeerQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub price: f64,
    pub pe_ratio: f64,
    pub forward_pe: Option<f64>,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    pub ev_to_sales: Option<f64>,
    pub market_cap: f64,
    pub revenue: f64,
    #[serde(default)]
    pub revenue_growth: Option<f64>,
    #[serde(default)]
    pub pat_growth: Option<f64>,
    pub ebitda: f64,
    #[serde(default)]
    pub ebitda_margin: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub profit_margins: f64,
    pub free_cashflow: Option<f64>,
    pub officer_pay: f64,
    pub average_volume_10_day: f64,
    #[serde(default)]
    pub dividend_yield: f64,
    #[serde(default = "default_true")]
    pub industrial_metrics_analysis_applicable: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct PeerAnalysis {
    pub peers: Vec<PeerQuote>,
    #[serde(alias = "subject_quality_score")]
    pub subject_data_quality_score: f64,
    #[serde(default)]
    pub subject_pay_vs_revenue_score: Option<f64>,
    pub subject_percentile_pe: Option<f64>,
    pub subject_percentile_roe: Option<f64>,
    pub subject_percentile_quality: Option<f64>,
    pub subject_percentile_pay_efficiency: Option<f64>,
    #[serde(default)]
    pub roe_coverage_known: usize,
    #[serde(default)]
    pub roe_coverage_total: usize,
    pub narrative: String,
    #[serde(default)]
    pub peer_comparability: String,
    #[serde(default)]
    pub direct_peer_comparability: String,
    #[serde(default)]
    pub bank_comparability: String,
    #[serde(default)]
    pub peer_set_kind: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReportInsights {
    pub executive_summary: String,
    pub strengths: Vec<String>,
    pub watch_items: Vec<String>,
    #[serde(default)]
    pub data_notes: Vec<String>,
    #[serde(default)]
    pub data_strengths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FundamentalSection {
    #[serde(default)]
    pub title: String,
    pub interpretation: String,
    pub flags: Vec<String>,
    pub confidence: String,
    pub lines: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FundamentalAnalysis {
    pub growth: FundamentalSection,
    pub profitability: FundamentalSection,
    pub balance_sheet: FundamentalSection,
    pub cash_flow: FundamentalSection,
    pub efficiency: FundamentalSection,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct HistoricalMultiples {
    pub median_pe_3y: Option<f64>,
    pub median_pe_5y: Option<f64>,
    pub median_pb_3y: Option<f64>,
    pub median_pb_5y: Option<f64>,
    pub median_ev_ebitda_3y: Option<f64>,
    pub median_ev_ebitda_5y: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    #[serde(default)]
    pub is_model_assumption: bool,
    #[serde(default)]
    pub assumption_note: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValuationAnalysis {
    pub pe: f64,
    pub forward_pe: Option<f64>,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub yahoo_ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub calculated_ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub ev_to_ebitda_note: String,
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
    #[serde(default)]
    pub pb_roe_interpretation: String,
    #[serde(default)]
    pub lender_valuation: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalTrend {
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub trend_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalVolatility {
    pub dist_from_high_pct: Option<f64>,
    pub vol_1y_ann_pct: Option<f64>,
    pub max_drawdown_1y_pct: Option<f64>,
    pub atr_14: Option<f64>,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalVolume {
    pub volume_breakout: bool,
    pub interpretation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TechnicalAnalysis {
    pub trend: TechnicalTrend,
    pub momentum: TechnicalMomentum,
    pub volatility: TechnicalVolatility,
    pub volume: TechnicalVolume,
    pub confidence: String,
    #[serde(default)]
    pub state: TechnicalState,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TechnicalState {
    pub above_dma50: Option<bool>,
    pub above_dma200: Option<bool>,
    pub rsi_oversold: Option<bool>,
    pub rsi_weak: Option<bool>,
    pub rsi_below_35: Option<bool>,
    pub macd_bullish: Option<bool>,
    pub price_stretched_vs_50: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TechnicalEntrySignal {
    pub zone: String,
    pub detail_label: String,
    pub rationale: Vec<String>,
    pub fundamental_vs_technical: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoreExplanation {
    pub factor: String,
    pub impact: String,
    pub points: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResearchRating {
    pub growth_score: f64,
    #[serde(default)]
    pub financial_quality_score: Option<f64>,
    pub valuation_score: f64,
    pub technical_score: f64,
    #[serde(default, alias = "risk_score")]
    pub risk_penalty: Option<f64>,
    #[serde(default)]
    pub overall_score: Option<f64>,
    #[serde(default)]
    pub overall_score_provisional: bool,
    #[serde(default)]
    pub provisional_screening_score: Option<f64>,
    #[serde(default)]
    pub data_quality_score: f64,
    #[serde(default)]
    pub screening_quality_score: Option<f64>,
    pub rating_label: String,
    pub fundamental_rating: String,
    pub valuation_rating: String,
    pub technical_rating: String,
    pub risk_rating: String,
    #[serde(default)]
    pub fundamental_risk_rating: String,
    #[serde(default)]
    pub market_beta_risk: String,
    pub explain: Vec<ScoreExplanation>,
    pub cheap_fair_expensive_fundamental: String,
    pub technical_entry_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ActionGuidance {
    pub if_holding: String,
    pub if_considering_entry: String,
    pub wait_for_events: Vec<String>,
    pub headline: String,
    pub rationale_bullets: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditChecklistItem {
    pub metric: String,
    pub value: Option<f64>,
    pub value_display: String,
    pub benchmark: String,
    pub status: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CoverageDimension {
    pub name: String,
    pub coverage_pct: f64,
    pub present: usize,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DataCoverage {
    pub overall_pct: f64,
    pub critical_pct: f64,
    pub confidence: String,
    pub dimensions: Vec<CoverageDimension>,
    pub recommendation_gated: bool,
    pub gate_reason: Option<String>,
    #[serde(default)]
    pub critical_present: usize,
    #[serde(default)]
    pub critical_total: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FinancialStrengthAudit {
    #[serde(default)]
    pub earnings_quality_score: Option<f64>,
    #[serde(default)]
    pub balance_sheet_score: Option<f64>,
    #[serde(default)]
    pub overall_strength_score: Option<f64>,
    pub checklist: Vec<AuditChecklistItem>,
    pub red_flags: Vec<String>,
    pub strengths: Vec<String>,
    pub interpretation: String,
    pub confidence: String,
    #[serde(default)]
    pub scores_provisional: bool,
    #[serde(default)]
    pub financial_quality_display: String,
    #[serde(default)]
    pub data_coverage: DataCoverage,
    #[serde(default)]
    pub quality_weights_note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AnalystRecommendationPeriod {
    pub period: String,
    pub strong_buy: u32,
    pub buy: u32,
    pub hold: u32,
    pub sell: u32,
    pub strong_sell: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AnalystRecommendations {
    pub trend: Vec<AnalystRecommendationPeriod>,
    pub net_bullish_score: i32,
    pub consensus_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InsiderTransaction {
    pub filer_name: String,
    pub transaction_text: String,
    pub shares: f64,
    pub value: Option<f64>,
    pub start_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InstitutionalHolder {
    pub organization: String,
    pub pct_held: f64,
    pub position: f64,
    pub value: f64,
    pub report_date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MarketSignals {
    pub analyst: AnalystRecommendations,
    pub insider_transactions: Vec<InsiderTransaction>,
    pub institutional_holders: Vec<InstitutionalHolder>,
    pub narrative: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BankingMetrics {
    pub gnpa_pct: Option<f64>,
    pub nnpa_pct: Option<f64>,
    pub provision_coverage_ratio_pct: Option<f64>,
    pub credit_growth_yoy_pct: Option<f64>,
    pub deposit_growth_yoy_pct: Option<f64>,
    pub casa_ratio_pct: Option<f64>,
    pub as_of_date: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    #[serde(default)]
    pub action_guidance: ActionGuidance,
    pub disclaimer: String,
    #[serde(default)]
    pub company_type_headline: String,
    #[serde(default)]
    pub executive_blocks: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    #[serde(default, alias = "latest_quarter_end")]
    pub latest_yahoo_quarter_end: Option<String>,
    #[serde(default)]
    pub latest_reported_quarter_end: Option<String>,
    #[serde(default)]
    pub quarterly_statement_stale: bool,
    #[serde(default)]
    pub quarterly_statement_age_days: Option<i64>,
    #[serde(default)]
    pub financial_company_type: String,
    #[serde(default)]
    pub financial_company_type_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct CanonicalMetrics {
    pub cfo: Option<f64>,
    pub capex: Option<f64>,
    pub fcf: Option<f64>,
    pub pat: Option<f64>,
    #[serde(default)]
    pub fy_pat: Option<f64>,
    #[serde(default)]
    pub ttm_pat: Option<f64>,
    #[serde(default, alias = "latest_quarter_pat")]
    pub latest_yahoo_quarter_pat: Option<f64>,
    #[serde(default, alias = "latest_quarter_pat_period")]
    pub latest_yahoo_quarter_pat_period: String,
    #[serde(default, alias = "latest_quarter_pat_source_column")]
    pub latest_yahoo_quarter_pat_source_column: String,
    #[serde(default)]
    pub latest_reported_quarter_end: Option<String>,
    #[serde(default)]
    pub quarterly_statement_stale: bool,
    #[serde(default)]
    pub quarterly_statement_age_days: Option<i64>,
    #[serde(default)]
    pub quarterly_statement_stale_note: String,
    #[serde(default)]
    pub pat_period: String,
    #[serde(default)]
    pub pat_scope: String,
    #[serde(default)]
    pub pat_yahoo_row: Option<String>,
    pub revenue: Option<f64>,
    pub roce: Option<f64>,
    pub current_ratio: Option<f64>,
    pub interest_coverage: Option<f64>,
    pub cash_and_cash_equivalents: Option<f64>,
    pub short_term_investments: Option<f64>,
    pub gross_cash_and_liquid_investments: Option<f64>,
    pub total_debt: Option<f64>,
    pub net_debt_vs_cash_equivalents: Option<f64>,
    pub net_debt_vs_liquid: Option<f64>,
    pub is_net_cash_equivalents: bool,
    #[serde(default)]
    pub raw_balance_sheet: Option<RawBalanceSheetMetrics>,
    pub revenue_cagr_3y_pct: Option<f64>,
    pub pat_cagr_3y_pct: Option<f64>,
    #[serde(default)]
    pub fy_revenue_yoy_pct: Option<f64>,
    #[serde(default)]
    pub fy_pat_yoy_pct: Option<f64>,
    #[serde(default)]
    pub interest_income: Option<f64>,
    #[serde(default, alias = "total_income")]
    pub yahoo_revenue_field: Option<f64>,
    #[serde(default)]
    pub interest_expense: Option<f64>,
    #[serde(default)]
    pub net_interest_income: Option<f64>,
    #[serde(default)]
    pub canonical_nii: Option<f64>,
    #[serde(default)]
    pub canonical_nii_source: String,
    #[serde(default)]
    pub nii_reconciliation_difference: Option<f64>,
    #[serde(default)]
    pub calculated_nii: Option<f64>,
    #[serde(default, alias = "reported_nii")]
    pub yahoo_reported_nii: Option<f64>,
    #[serde(default)]
    pub nii_definition: String,
    #[serde(default)]
    pub other_income: Option<f64>,
    #[serde(default)]
    pub yahoo_loan_book_field: Option<f64>,
    #[serde(default)]
    pub yahoo_loan_book_row: String,
    #[serde(default)]
    pub yahoo_loan_book_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub canonical_advances: Option<f64>,
    #[serde(default)]
    pub loan_book: Option<f64>,
    #[serde(default)]
    pub loan_book_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub interest_income_yoy_pct: Option<f64>,
    #[serde(default)]
    pub nii_yoy_pct: Option<f64>,
    #[serde(default)]
    pub industrial_metrics_suppressed: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct RawBalanceSheetMetrics {
    pub cash_and_cash_equivalents: Option<f64>,
    pub short_term_investments: Option<f64>,
    pub total_debt: Option<f64>,
    pub net_debt_vs_cash_equivalents: Option<f64>,
    pub net_debt_vs_liquid: Option<f64>,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResearchReport {
    pub symbol: String,
    pub long_name: Option<String>,
    pub price: f64,
    #[serde(default)]
    pub retrieved_at: String,
    pub financials: Financials,
    #[serde(default)]
    pub financials_applicable: Option<FinancialsApplicable>,
    #[serde(default)]
    pub financials_raw_yahoo: Option<FinancialsRawYahoo>,
    #[serde(default)]
    pub canonical: CanonicalMetrics,
    pub shareholders: Shareholders,
    pub annual_reports: Vec<AnnualReport>,
    pub company_summary: Option<String>,
    pub asset_profile: AssetProfile,
    #[serde(default)]
    pub financial_company_type: String,
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
    #[serde(default)]
    pub financial_strength_audit: FinancialStrengthAudit,
    #[serde(default)]
    pub market_signals: MarketSignals,
    #[serde(default)]
    pub bank_metrics: Option<BankingMetrics>,
    #[serde(default)]
    pub screener_enrichment: Option<ScreenerMetricSnapshot>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ScreenerMetricSnapshot {
    pub operating_cashflow_ttm: Option<f64>,
    pub profit_after_tax_ttm: Option<f64>,
    pub interest_coverage_ratio: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub piotroski_f_score: Option<f64>,
    pub altman_z_score: Option<f64>,
    pub updated_at: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CashFlowQuality {
    pub pat: Option<f64>,
    pub cfo: Option<f64>,
    pub ebitda: f64,
    pub free_cashflow: Option<f64>,
    pub capex_estimate: Option<f64>,
    pub pat_vs_cfo_delta: Option<f64>,
    pub cfo_vs_ebitda_ratio: Option<f64>,
    pub cash_conversion_ratio: Option<f64>,
    pub capex_requirement_ratio: Option<f64>,
    #[serde(default)]
    pub cumulative_cfo_pat_3y: Option<f64>,
    #[serde(default)]
    pub cumulative_cfo_pat_5y: Option<f64>,
    pub narrative: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorableItem {
    pub area: String,
    pub what_to_track: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RiskItem {
    pub risk: String,
    pub severity: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RiskBuckets {
    pub business_risks: Vec<RiskItem>,
    pub financial_risks: Vec<RiskItem>,
    pub management_risks: Vec<RiskItem>,
    pub valuation_risks: Vec<RiskItem>,
    pub regulatory_risks: Vec<RiskItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScenarioAnalysis {
    pub base_case: String,
    pub upside_case: String,
    pub downside_case: String,
    pub capital_impairment_guardrail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoreBreakdown {
    #[serde(default)]
    pub business_quality: Option<f64>,
    #[serde(default)]
    pub industry_tailwind: Option<f64>,
    #[serde(default)]
    pub financial_strength: Option<f64>,
    #[serde(default)]
    pub management_quality: Option<f64>,
    pub valuation_comfort: f64,
    #[serde(default)]
    pub growth_triggers: Option<f64>,
    #[serde(default)]
    pub risk_reward: Option<f64>,
    #[serde(default)]
    pub total_score: Option<f64>,
    pub interpretation: String,
    #[serde(default)]
    pub screening_score: f64,
    #[serde(default)]
    pub score_provisional: bool,
    #[serde(default)]
    pub provisional_screening_score: Option<f64>,
    #[serde(default)]
    pub available_dimension_score: Option<f64>,
    #[serde(default)]
    pub critical_coverage_pct: Option<f64>,
    #[serde(default)]
    pub coverage_adjusted_score: Option<f64>,
    #[serde(default)]
    pub score_provenance: Vec<String>,
}
