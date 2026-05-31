use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Financials {
    pub revenue: f64,
    pub net_income: f64,
    /// Trailing P/E (falls back to forward in fetcher if missing)
    pub pe_ratio: f64,
    pub forward_pe: f64,
    pub total_debt: f64,
    pub ebitda: f64,
    pub profit_margins: f64,
    /// Gross margin (decimal 0–1) from Yahoo `financialData.grossMargins`
    #[serde(default)]
    pub gross_margins: f64,
    /// Operating margin (decimal)
    #[serde(default)]
    pub operating_margins: f64,
    /// EBITDA margin (decimal), may be computed from EBITDA/revenue
    #[serde(default)]
    pub ebitda_margins: f64,
    pub return_on_equity: f64,
    /// Return on assets when Yahoo provides it (not ROCE)
    #[serde(default)]
    pub return_on_assets: Option<f64>,
    /// Return on capital employed when available; never aliased from ROA
    #[serde(default)]
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: f64,
    pub free_cashflow: f64,
    pub operating_cashflow: f64,
    pub shares_outstanding: f64,
    pub market_cap: f64,
    #[serde(default)]
    pub enterprise_value: f64,
    #[serde(default)]
    pub total_cash: f64,
    /// Per share (Yahoo `bookValue`); 0 if missing
    pub book_value: f64,
    pub price_to_book: f64,
    #[serde(default)]
    pub price_to_sales: f64,
    /// Trailing EPS; 0 if missing
    pub trailing_eps: f64,
    pub forward_eps: f64,
    /// 0.0 to 1.0 style (e.g. 0.02 = 2%)
    pub dividend_yield: f64,
    pub payout_ratio: f64,
    /// Y/Y growth; decimal from Yahoo (e.g. 0.12 = 12%)
    pub revenue_growth: f64,
    pub earnings_growth: f64,
    /// Session move vs previous close, decimal (e.g. 0.015 = 1.5%)
    pub regular_market_change_percent: f64,
    pub previous_close: f64,
    pub fifty_two_week_high: f64,
    pub fifty_two_week_low: f64,
    pub beta: f64,
    /// ISO-ish date string if present (Yahoo `fmt`)
    pub ex_dividend_date: Option<String>,
    #[serde(default)]
    pub regular_market_volume: f64,
    #[serde(default)]
    pub average_volume_10_day: f64,
    #[serde(default)]
    pub current_ratio: Option<f64>,
    #[serde(default)]
    pub quick_ratio: Option<f64>,
    /// Par / face value per share when Yahoo reports it (often missing for NSE).
    #[serde(default)]
    pub face_value: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    pub revenue: f64,
    pub net_income: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub published_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AssetProfile {
    pub long_name: Option<String>,
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub long_business_summary: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerQuote {
    pub symbol: String,
    pub short_name: Option<String>,
    pub price: f64,
    pub pe_ratio: f64,
    #[serde(default)]
    pub forward_pe: f64,
    #[serde(default)]
    pub price_to_book: f64,
    #[serde(default)]
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub ev_to_sales: Option<f64>,
    pub market_cap: f64,
    pub revenue: f64,
    pub revenue_growth: f64,
    pub pat_growth: f64,
    pub ebitda: f64,
    pub ebitda_margin: f64,
    pub return_on_equity: f64,
    pub return_on_capital_employed: Option<f64>,
    #[serde(default)]
    pub return_on_assets: Option<f64>,
    pub profit_margins: f64,
    pub debt_to_equity: f64,
    pub free_cashflow: f64,
    pub officer_pay: f64,
    #[serde(default)]
    pub average_volume_10_day: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorableItem {
    pub area: String,
    pub what_to_track: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RiskCategory {
    Business,
    Financial,
    Management,
    Valuation,
    Regulatory,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RiskItem {
    pub category: RiskCategory,
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
pub struct ReportInsights {
    pub executive_summary: String,
    pub strengths: Vec<String>,
    pub watch_items: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    /// Short note, e.g. PEG-style when growth data exists; empty if not applicable
    pub peg_style_note: String,
    /// 1.0 = at 52W high, 0.0 = at 52W low (by price in range). None if range missing
    pub price_in_52w_range: Option<f64>,
    pub distance_from_52w_high_pct: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManagementAnalysis {
    pub pay_vs_revenue_score: f64,
    pub tone_score: f64,
    pub tone_label: String,
    pub narrative: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SectorAnalysisDetail {
    pub sector: Option<String>,
    pub industry: Option<String>,
    pub outlook_narrative: String,
    pub sector_news_summary: String,
    /// First few sector news titles for the UI
    pub sample_headlines: Vec<String>,
    /// Heuristic one-line read on headline wording (Yahoo data only, not investment advice)
    pub sector_headline_themes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerBenchmark {
    pub symbol: String,
    pub short_name: Option<String>,
    pub quality_score: f64,
    pub pay_vs_revenue_score: f64,
    pub pay_to_revenue_pct: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerAnalysis {
    pub peers: Vec<PeerQuote>,
    pub benchmarks: Vec<PeerBenchmark>,
    pub subject_quality_score: f64,
    pub subject_pay_vs_revenue_score: f64,
    pub subject_percentile_pe: Option<f64>,
    pub subject_percentile_roe: Option<f64>,
    pub subject_percentile_quality: Option<f64>,
    pub subject_percentile_pay_efficiency: Option<f64>,
    pub narrative: String,
}

// --- Chart & statements (fetcher → analysis) ---

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ChartBar {
    pub ts: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ChartHistory {
    pub bars: Vec<ChartBar>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct IncomeStatementRow {
    pub end_date_fmt: String,
    pub end_ts: Option<i64>,
    pub revenue: f64,
    pub cost_of_revenue: f64,
    pub gross_profit: f64,
    pub ebitda: f64,
    pub operating_income: f64,
    /// EBIT when Yahoo exposes it separately from operating income.
    #[serde(default)]
    pub ebit: f64,
    #[serde(default)]
    pub pretax_income: f64,
    #[serde(default)]
    pub interest_expense: f64,
    #[serde(default)]
    pub income_tax_expense: f64,
    /// Depreciation from FTS (`reconciledDepreciation` / income-statement depreciation lines).
    #[serde(default)]
    pub depreciation: f64,
    pub net_income: f64,
    pub diluted_eps: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BalanceSheetRow {
    pub end_date_fmt: String,
    pub end_ts: Option<i64>,
    pub cash: f64,
    pub total_debt: f64,
    pub total_equity: f64,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub current_assets: f64,
    pub current_liabilities: f64,
    pub interest_expense: f64,
    pub inventory: f64,
    pub net_receivables: f64,
    #[serde(default)]
    pub retained_earnings: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CashflowRow {
    pub end_date_fmt: String,
    pub end_ts: Option<i64>,
    pub operating_cashflow: f64,
    pub capital_expenditure: f64,
    pub free_cashflow: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct StatementBundle {
    pub income_annual: Vec<IncomeStatementRow>,
    pub income_quarterly: Vec<IncomeStatementRow>,
    pub balance_annual: Vec<BalanceSheetRow>,
    pub balance_quarterly: Vec<BalanceSheetRow>,
    pub cashflow_annual: Vec<CashflowRow>,
    pub cashflow_quarterly: Vec<CashflowRow>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HistoricalMultiples {
    pub median_pe_3y: Option<f64>,
    pub median_pe_5y: Option<f64>,
    pub median_pb_3y: Option<f64>,
    pub median_pb_5y: Option<f64>,
    pub median_ev_ebitda_3y: Option<f64>,
    pub median_ev_ebitda_5y: Option<f64>,
    pub pe_points_used: usize,
    pub pb_points_used: usize,
    pub ev_ebitda_points_used: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PeerValuationCompare {
    pub median_pe: Option<f64>,
    pub median_pb: Option<f64>,
    pub median_ev_ebitda: Option<f64>,
    pub median_ps: Option<f64>,
    pub median_roe: Option<f64>,
    pub median_roce: Option<f64>,
    pub median_revenue_growth: Option<f64>,
    pub median_profit_growth: Option<f64>,
    pub subject_pe_vs_median_pct: Option<f64>,
    pub subject_pb_vs_median_pct: Option<f64>,
    pub subject_ev_ebitda_vs_median_pct: Option<f64>,
    pub subject_ps_vs_median_pct: Option<f64>,
    pub interpretation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    pub latest_quarter_end: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FundamentalSection {
    pub interpretation: String,
    pub flags: Vec<String>,
    pub confidence: String,
    pub lines: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FundamentalAnalysis {
    pub growth: FundamentalSection,
    pub profitability: FundamentalSection,
    pub balance_sheet: FundamentalSection,
    pub cash_flow: FundamentalSection,
    pub efficiency: FundamentalSection,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TechnicalTrend {
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub sma_100: Option<f64>,
    pub sma_200: Option<f64>,
    pub price_vs_sma50_pct: Option<f64>,
    pub price_vs_sma200_pct: Option<f64>,
    pub trend_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
pub struct TechnicalVolatility {
    pub fifty_two_week_high: f64,
    pub fifty_two_week_low: f64,
    pub dist_from_high_pct: Option<f64>,
    pub dist_from_low_pct: Option<f64>,
    pub vol_1y_ann_pct: Option<f64>,
    pub max_drawdown_1y_pct: Option<f64>,
    pub atr_14: Option<f64>,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TechnicalVolume {
    pub average_volume: f64,
    pub current_volume: f64,
    pub vs_20d_avg_pct: Option<f64>,
    pub delivery_pct: Option<f64>,
    pub volume_breakout: bool,
    pub interpretation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TechnicalAnalysis {
    pub trend: TechnicalTrend,
    pub momentum: TechnicalMomentum,
    pub volatility: TechnicalVolatility,
    pub volume: TechnicalVolume,
    pub confidence: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TechnicalEntrySignal {
    pub zone: String,
    pub detail_label: String,
    pub rationale: Vec<String>,
    pub fundamental_vs_technical: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ScoreExplanation {
    pub factor: String,
    pub impact: String,
    pub points: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    pub weights: (f64, f64, f64, f64, f64),
    pub explain: Vec<ScoreExplanation>,
    pub cheap_fair_expensive_fundamental: String,
    pub technical_entry_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
