use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinancialCompanyType {
    #[default]
    Industrial,
    Bank,
    Nbfc,
    NbfcProjectFinance,
    HousingFinance,
    Insurance,
    Amc,
    Broker,
    Exchange,
    Payments,
}

impl std::fmt::Display for FinancialCompanyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FinancialCompanyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Industrial => "INDUSTRIAL",
            Self::Bank => "BANK",
            Self::Nbfc => "NBFC",
            Self::NbfcProjectFinance => "NBFC_PROJECT_FINANCE",
            Self::HousingFinance => "HOUSING_FINANCE",
            Self::Insurance => "INSURANCE",
            Self::Amc => "AMC",
            Self::Broker => "BROKER",
            Self::Exchange => "EXCHANGE",
            Self::Payments => "PAYMENTS",
        }
    }
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
pub struct Financials {
    pub revenue: f64,
    /// TTM net income from Yahoo `netIncomeToCommon` only (`None` when missing).
    #[serde(default)]
    pub net_income: Option<f64>,
    /// Yahoo `netIncomeToCommon` when present — not annual statement `Net Income`.
    #[serde(default)]
    pub net_income_to_common: Option<f64>,
    /// Trailing P/E only (never silently replaced by forward P/E)
    pub pe_ratio: f64,
    #[serde(default)]
    pub forward_pe: Option<f64>,
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
    #[serde(default)]
    pub return_on_equity: Option<f64>,
    /// Return on assets when Yahoo provides it (not ROCE)
    #[serde(default)]
    pub return_on_assets: Option<f64>,
    /// Return on capital employed when available; never aliased from ROA
    #[serde(default)]
    pub return_on_capital_employed: Option<f64>,
    #[serde(default)]
    pub debt_to_equity: Option<f64>,
    /// Yahoo `financialData.freeCashflow` snapshot (not statement CFO−capex).
    #[serde(default)]
    pub free_cashflow: Option<f64>,
    #[serde(default)]
    pub operating_cashflow: Option<f64>,
    pub shares_outstanding: f64,
    pub market_cap: f64,
    #[serde(default)]
    pub enterprise_value: Option<f64>,
    /// Yahoo `enterpriseToEbitda` from quote statistics (not a statement-cash EV).
    #[serde(default)]
    pub yahoo_ev_to_ebitda: Option<f64>,
    #[serde(default)]
    pub total_cash: f64,
    /// Per share (Yahoo `bookValue`); 0 if missing
    pub book_value: f64,
    pub price_to_book: f64,
    #[serde(default)]
    pub price_to_sales: f64,
    /// Trailing EPS; 0 if missing
    pub trailing_eps: f64,
    #[serde(default)]
    pub forward_eps: Option<f64>,
    /// 0.0 to 1.0 style (e.g. 0.02 = 2%)
    pub dividend_yield: f64,
    pub payout_ratio: f64,
    /// Yahoo `revenueGrowth` when present (decimal). Missing stays `None` — not 0.
    #[serde(default)]
    pub revenue_growth: Option<f64>,
    /// Yahoo `earningsGrowth` when present (decimal). Missing stays `None` — not 0.
    #[serde(default)]
    pub earnings_growth: Option<f64>,
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
    /// False for lenders: EBITDA, gross/operating margins, P/S, EV are raw Yahoo, not analysis inputs.
    #[serde(default = "default_true")]
    pub industrial_yahoo_fields_analysis_applicable: bool,
}

/// Lender-safe Yahoo quote fields (valuation and identity). Not industrial P&L.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FinancialsApplicable {
    pub pe_ratio: f64,
    pub price_to_book: f64,
    pub trailing_eps: f64,
    pub book_value: f64,
    pub dividend_yield: f64,
    pub market_cap: f64,
    pub beta: f64,
    #[serde(default)]
    pub return_on_equity: Option<f64>,
    #[serde(default)]
    pub return_on_assets: Option<f64>,
    #[serde(default)]
    pub net_income: Option<f64>,
    #[serde(default)]
    pub earnings_growth: Option<f64>,
    #[serde(default)]
    pub forward_pe: Option<f64>,
    #[serde(default)]
    pub forward_eps: Option<f64>,
}

/// Raw Yahoo snapshot fields that are often meaningless for lenders (kept for audit, not scoring).
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    #[serde(default)]
    pub analysis_applicable: bool,
    #[serde(default)]
    pub note: String,
}

impl Financials {
    pub fn applicable_view(&self) -> FinancialsApplicable {
        FinancialsApplicable {
            pe_ratio: self.pe_ratio,
            price_to_book: self.price_to_book,
            trailing_eps: self.trailing_eps,
            book_value: self.book_value,
            dividend_yield: self.dividend_yield,
            market_cap: self.market_cap,
            beta: self.beta,
            return_on_equity: self.return_on_equity,
            return_on_assets: self.return_on_assets,
            net_income: self.net_income,
            earnings_growth: self.earnings_growth,
            forward_pe: self.forward_pe,
            forward_eps: self.forward_eps,
        }
    }

    pub fn raw_yahoo_view(&self) -> FinancialsRawYahoo {
        FinancialsRawYahoo {
            revenue: self.revenue,
            revenue_growth: self.revenue_growth,
            ebitda: self.ebitda,
            gross_margins: self.gross_margins,
            operating_margins: self.operating_margins,
            ebitda_margins: self.ebitda_margins,
            profit_margins: self.profit_margins,
            price_to_sales: self.price_to_sales,
            enterprise_value: self.enterprise_value,
            yahoo_ev_to_ebitda: self.yahoo_ev_to_ebitda,
            free_cashflow: self.free_cashflow,
            operating_cashflow: self.operating_cashflow,
            debt_to_equity: self.debt_to_equity,
            current_ratio: self.current_ratio,
            total_debt: self.total_debt,
            total_cash: self.total_cash,
            analysis_applicable: self.industrial_yahoo_fields_analysis_applicable,
            note: if self.industrial_yahoo_fields_analysis_applicable {
                String::new()
            } else {
                "Raw Yahoo industrial fields. Do not use EBITDA, gross/operating margin, P/S, EV, or revenue growth for lender analysis.".to_string()
            },
        }
    }
}

fn default_true() -> bool {
    true
}

/// Statement-first metrics. Scorers and cash-flow quality must read this, not Yahoo zeros.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CanonicalMetrics {
    pub cfo: Option<f64>,
    pub capex: Option<f64>,
    pub fcf: Option<f64>,
    /// Selected PAT used in scoring (`ttm_pat` if present, else `fy_pat`, else Yahoo TTM).
    pub pat: Option<f64>,
    /// Latest annual statement net income.
    #[serde(default)]
    pub fy_pat: Option<f64>,
    /// Sum of last four quarterly net income rows when available.
    #[serde(default)]
    pub ttm_pat: Option<f64>,
    /// Newest PAT on Yahoo's quarterly income series (not necessarily the company's latest published quarter).
    #[serde(default, alias = "latest_quarter_pat")]
    pub latest_yahoo_quarter_pat: Option<f64>,
    #[serde(default, alias = "latest_quarter_pat_period")]
    pub latest_yahoo_quarter_pat_period: String,
    #[serde(default, alias = "latest_quarter_pat_source_column")]
    pub latest_yahoo_quarter_pat_source_column: String,
    /// Filing/company latest reported quarter-end when available. Yahoo-only reports leave this empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_reported_quarter_end: Option<String>,
    #[serde(default)]
    pub quarterly_statement_stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarterly_statement_age_days: Option<i64>,
    #[serde(default)]
    pub quarterly_statement_stale_note: String,
    /// `ttm`, `fy`, or `yahoo_quote_ttm`.
    #[serde(default)]
    pub pat_period: String,
    /// Yahoo does not label standalone vs consolidated; stays `unknown` unless inferred.
    #[serde(default)]
    pub pat_scope: String,
    #[serde(default)]
    pub pat_yahoo_row: Option<String>,
    /// Industrial revenue. `None` for lenders — use `yahoo_revenue_field`.
    pub revenue: Option<f64>,
    pub roce: Option<f64>,
    pub current_ratio: Option<f64>,
    pub interest_coverage: Option<f64>,
    pub cash_and_cash_equivalents: Option<f64>,
    pub short_term_investments: Option<f64>,
    pub gross_cash_and_liquid_investments: Option<f64>,
    pub total_debt: Option<f64>,
    /// Industrial net-debt concept. `None` for lenders — see `raw_balance_sheet`.
    pub net_debt_vs_cash_equivalents: Option<f64>,
    pub net_debt_vs_liquid: Option<f64>,
    pub is_net_cash_equivalents: bool,
    /// Statement cash/debt arithmetic parked here for lenders (not a liquidity score).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_balance_sheet: Option<RawBalanceSheetMetrics>,
    pub revenue_cagr_3y_pct: Option<f64>,
    pub pat_cagr_3y_pct: Option<f64>,
    /// Latest two annual statement years (not Yahoo `revenueGrowth`).
    #[serde(default)]
    pub fy_revenue_yoy_pct: Option<f64>,
    /// Latest two annual statement years (not Yahoo `earningsGrowth`).
    #[serde(default)]
    pub fy_pat_yoy_pct: Option<f64>,
    #[serde(default)]
    pub interest_income: Option<f64>,
    /// Yahoo `totalRevenue` — not verified as NBFC “total income”; often lower than interest income.
    #[serde(default, alias = "total_income")]
    pub yahoo_revenue_field: Option<f64>,
    #[serde(default)]
    pub interest_expense: Option<f64>,
    /// Preferred NII: reported Yahoo `netInterestIncome` when present, else II − IE.
    #[serde(default)]
    pub net_interest_income: Option<f64>,
    /// Same value as `net_interest_income`; explicit canonical name.
    #[serde(default)]
    pub canonical_nii: Option<f64>,
    /// `yahoo_reported_nii` or `calculated_nii`.
    #[serde(default)]
    pub canonical_nii_source: String,
    /// `calculated_nii − yahoo_reported_nii` when both exist.
    #[serde(default)]
    pub nii_reconciliation_difference: Option<f64>,
    #[serde(default)]
    pub calculated_nii: Option<f64>,
    /// Yahoo `netInterestIncome` row — not necessarily company-presented NII.
    #[serde(default, alias = "reported_nii")]
    pub yahoo_reported_nii: Option<f64>,
    /// `Yahoo Net Interest Income statement row` or `calculated`.
    #[serde(default)]
    pub nii_definition: String,
    #[serde(default)]
    pub other_income: Option<f64>,
    /// Yahoo loans/receivables row — not verified as company-reported gross advances.
    #[serde(default)]
    pub yahoo_loan_book_field: Option<f64>,
    /// Yahoo statement key used for `yahoo_loan_book_field` (e.g. `Net Loan`).
    #[serde(default)]
    pub yahoo_loan_book_row: String,
    #[serde(default)]
    pub yahoo_loan_book_growth_yoy_pct: Option<f64>,
    /// Filing-verified gross advances only. Never filled from an unverified Yahoo loan row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_advances: Option<f64>,
    /// NBFC loan-book when Yahoo is used as a proxy. `None` for banks (see `yahoo_loan_book_field`).
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
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RawBalanceSheetMetrics {
    pub cash_and_cash_equivalents: Option<f64>,
    pub short_term_investments: Option<f64>,
    pub total_debt: Option<f64>,
    pub net_debt_vs_cash_equivalents: Option<f64>,
    pub net_debt_vs_liquid: Option<f64>,
    #[serde(default)]
    pub note: String,
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
    /// Industrial sales. `None` for lenders — use `yahoo_total_revenue_raw`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revenue: Option<f64>,
    #[serde(default)]
    pub yahoo_total_revenue_raw: f64,
    /// False for NBFCs/banks: do not treat `revenue` as product sales.
    #[serde(default = "default_true")]
    pub revenue_represents_sales: bool,
    pub net_income: f64,
    #[serde(default)]
    pub net_income_yahoo_row: Option<String>,
    /// Yahoo does not label standalone vs consolidated.
    #[serde(default)]
    pub pat_scope: String,
    /// Set when this year sits outside the consistent statement-scope suffix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_warning: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub published_at: String,
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
    /// Net bullish score: (strong_buy + buy) - (sell + strong_sell) for latest month
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

/// Optional banking-specific metrics (typically from annual report / investor presentation).
///
/// Stocker does not scrape NSE/RBI for these; instead they are intended to be
/// loaded from a user-provided local CSV (see `STOCKER_BANK_METRICS_CSV`).
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BankingMetrics {
    /// Gross NPA %, e.g. 2.34 for 2.34%
    pub gnpa_pct: Option<f64>,
    /// Net NPA %, e.g. 0.65 for 0.65%
    pub nnpa_pct: Option<f64>,
    /// Provision coverage ratio %, e.g. 78.0
    pub provision_coverage_ratio_pct: Option<f64>,
    pub credit_growth_yoy_pct: Option<f64>,
    pub deposit_growth_yoy_pct: Option<f64>,
    pub casa_ratio_pct: Option<f64>,
    pub as_of_date: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub credit_cost_pct: Option<f64>,
    #[serde(default)]
    pub slippages_pct: Option<f64>,
    #[serde(default)]
    pub recoveries: Option<f64>,
    #[serde(default)]
    pub write_offs: Option<f64>,
    #[serde(default)]
    pub sma1_pct: Option<f64>,
    #[serde(default)]
    pub sma2_pct: Option<f64>,
    #[serde(default)]
    pub restructured_pct: Option<f64>,
    #[serde(default)]
    pub stage2_pct: Option<f64>,
    #[serde(default)]
    pub crar_pct: Option<f64>,
    #[serde(default)]
    pub tier1_pct: Option<f64>,
    #[serde(default)]
    pub net_worth: Option<f64>,
    #[serde(default)]
    pub gearing: Option<f64>,
    #[serde(default)]
    pub credit_rating: Option<String>,
    #[serde(default)]
    pub alm_lcr_pct: Option<f64>,
    #[serde(default)]
    pub yield_on_assets_pct: Option<f64>,
    #[serde(default)]
    pub cost_of_funds_pct: Option<f64>,
    #[serde(default)]
    pub spread_pct: Option<f64>,
    #[serde(default)]
    pub nim_pct: Option<f64>,
    #[serde(default)]
    pub incremental_borrowing_cost_pct: Option<f64>,
    #[serde(default)]
    pub loan_book: Option<f64>,
    #[serde(default)]
    pub loan_book_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub sanctions: Option<f64>,
    #[serde(default)]
    pub disbursements: Option<f64>,
    #[serde(default)]
    pub repayments: Option<f64>,
    #[serde(default)]
    pub disbursement_growth_yoy_pct: Option<f64>,
    #[serde(default)]
    pub renewable_loan_book: Option<f64>,
    #[serde(default)]
    pub infrastructure_loan_book: Option<f64>,
    #[serde(default)]
    pub private_sector_pct: Option<f64>,
    #[serde(default)]
    pub state_sector_pct: Option<f64>,
}

/// Lightweight screener metrics merged into research reports when DB snapshot is fresh.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ScreenerMetricSnapshot {
    pub operating_cashflow_ttm: Option<f64>,
    pub profit_after_tax_ttm: Option<f64>,
    pub interest_coverage_ratio: Option<f64>,
    pub days_receivable_outstanding: Option<f64>,
    pub days_inventory_outstanding: Option<f64>,
    pub days_receivable_change_3y: Option<f64>,
    pub days_inventory_change_3y: Option<f64>,
    pub cumulative_cfo_pat_3y: Option<f64>,
    pub cumulative_cfo_pat_5y: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub piotroski_f_score: Option<f64>,
    pub altman_z_score: Option<f64>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditStatus {
    Pass,
    Watch,
    Fail,
    InsufficientData,
}

impl AuditStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Watch => "watch",
            Self::Fail => "fail",
            Self::InsufficientData => "insufficient_data",
        }
    }
}

impl std::fmt::Display for AuditStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditChecklistItem {
    pub metric: String,
    pub value: Option<f64>,
    pub value_display: String,
    pub benchmark: String,
    pub status: AuditStatus,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FinancialStrengthAudit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earnings_quality_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_sheet_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
pub struct ActionGuidance {
    pub if_holding: String,
    pub if_considering_entry: String,
    pub wait_for_events: Vec<String>,
    pub headline: String,
    pub rationale_bullets: Vec<String>,
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
    /// Always `NSE` or `BSE` when populated (Yahoo NSI/YHD codes are normalized in the fetcher).
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
    pub forward_pe: Option<f64>,
    #[serde(default)]
    pub price_to_book: f64,
    #[serde(default)]
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    #[serde(default)]
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
    #[serde(default)]
    pub return_on_equity: Option<f64>,
    pub return_on_capital_employed: Option<f64>,
    #[serde(default)]
    pub return_on_assets: Option<f64>,
    pub profit_margins: f64,
    #[serde(default)]
    pub debt_to_equity: Option<f64>,
    #[serde(default)]
    pub free_cashflow: Option<f64>,
    pub officer_pay: f64,
    #[serde(default)]
    pub average_volume_10_day: f64,
    #[serde(default)]
    pub dividend_yield: f64,
    /// False for NBFC/bank peers: EBITDA, P/S, EV/Sales are raw Yahoo only.
    #[serde(default = "default_true")]
    pub industrial_metrics_analysis_applicable: bool,
}

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
    /// `None` until actual business economics are scored (not Yahoo data completeness).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_quality: Option<f64>,
    /// `None` for lenders until observed industry cycle data exists (not a template constant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub industry_tailwind: Option<f64>,
    /// `None` when the dimension is not assessed (do not treat as 0 or a placeholder).
    #[serde(default)]
    pub financial_strength: Option<f64>,
    #[serde(default)]
    pub management_quality: Option<f64>,
    pub valuation_comfort: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub growth_triggers: Option<f64>,
    /// `None` when risk is unassessed (gated lender) — not a numeric “low risk”.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_reward: Option<f64>,
    /// Investment ranking score. `None` when recommendation is gated (use `provisional_screening_score`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_score: Option<f64>,
    pub interpretation: String,
    /// Screening-model sum of the known factor columns (not the investment score).
    #[serde(default)]
    pub screening_score: f64,
    #[serde(default)]
    pub score_provisional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisional_screening_score: Option<f64>,
    /// Known dimensions only, renormalized to 0–100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_dimension_score: Option<f64>,
    /// Critical lender metric-group coverage (same basis as recommendation gating).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_coverage_pct: Option<f64>,
    /// Not a ranking substitute. `None` when the investment rating is withheld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage_adjusted_score: Option<f64>,
    /// How each scorecard column was derived or why it was excluded.
    #[serde(default)]
    pub score_provenance: Vec<String>,
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
    #[serde(default)]
    pub data_notes: Vec<String>,
    #[serde(default)]
    pub data_strengths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StockAnalysis {
    /// Yahoo snapshot heuristic — not financial quality.
    #[serde(alias = "quality_score")]
    pub data_quality_score: f64,
    /// Yahoo quote-snapshot heuristic (not financial-strength audit).
    #[serde(default)]
    pub quality_score_kind: String,
    pub valuation_label: String,
    pub revenue_cagr_full_series_pct: Option<f64>,
    pub net_income_cagr_full_series_pct: Option<f64>,
    pub revenue_cagr_trailing_3y_pct: Option<f64>,
    pub net_income_cagr_trailing_3y_pct: Option<f64>,
    pub revenue_cagr_trailing_5y_pct: Option<f64>,
    pub net_income_cagr_trailing_5y_pct: Option<f64>,
    /// Industrial net-margin trend. `None` for lenders (use NIM/spread/credit cost).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_trend: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pay_vs_revenue_score: Option<f64>,
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
    /// Heuristic Porter + lifecycle + industry profile (when enough cohort data)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research: Option<SectorResearchProfile>,
}

// --- Sector research (Porter, lifecycle, industry profile) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PorterForce {
    pub name: String,
    /// 0–100; higher = more hostile / intense for incumbents
    pub intensity: f64,
    pub label: String,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PorterFiveForces {
    pub rivalry: PorterForce,
    pub new_entrants: PorterForce,
    pub supplier_power: PorterForce,
    pub buyer_power: PorterForce,
    pub substitutes: PorterForce,
    /// 0–100; higher = more attractive industry structure
    pub attractiveness: f64,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectorLifecyclePhase {
    Startup,
    Growth,
    Consolidation,
    MaturityOrDecline,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SectorLifecycle {
    pub phase: SectorLifecyclePhase,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectorTypeKind {
    Growth,
    Cyclical,
    Defensive,
    CyclicalGrowth,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SectorTypeAssessment {
    pub sector_type: SectorTypeKind,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DemandSupplyGap {
    Shortage,
    Balanced,
    Oversupply,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DemandSupplyAssessment {
    pub gap_label: DemandSupplyGap,
    pub intensity: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompetitionStructure {
    Fragmented,
    ModeratelyConcentrated,
    Oligopolistic,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompetitionNature {
    pub structure: CompetitionStructure,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfitabilityLevel {
    High,
    Moderate,
    Low,
    Mixed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfitabilityAssessment {
    pub level: ProfitabilityLevel,
    pub score: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrowthProspectsLevel {
    Strong,
    Moderate,
    Weak,
    Contracting,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrowthProspects {
    pub level: GrowthProspectsLevel,
    pub score: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PricingPowerLevel {
    Low,
    Moderate,
    High,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PricingPowerSide {
    pub level: PricingPowerLevel,
    pub score: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndustryPricingPower {
    pub supplier: PricingPowerSide,
    pub customer: PricingPowerSide,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SectorResearchProfile {
    pub sector: String,
    pub company_count: usize,
    pub porter: PorterFiveForces,
    pub lifecycle: SectorLifecycle,
    pub sector_type: SectorTypeAssessment,
    pub demand_supply: DemandSupplyAssessment,
    pub competition: CompetitionNature,
    pub profitability: ProfitabilityAssessment,
    pub growth_prospects: GrowthProspects,
    pub pricing_power: IndustryPricingPower,
    /// Always low for automated Porter/demand inferences from listed financials.
    #[serde(default)]
    pub interpretation_confidence: String,
}

/// Cohort stats used to compute [`SectorResearchProfile`]. Growth fields are percentages
/// (e.g. 12.5 = 12.5% CAGR). Margins and ROE are decimals (0.15 = 15%).
#[derive(Debug, Clone, Default)]
pub struct SectorResearchInputs {
    pub sector: String,
    pub company_count: usize,
    pub with_snapshot_count: usize,
    pub hhi: f64,
    pub top3_mcap_share: f64,
    pub median_gross_margin: Option<f64>,
    pub median_op_margin: Option<f64>,
    pub median_net_margin: Option<f64>,
    pub median_roe: Option<f64>,
    pub median_debt_to_equity: Option<f64>,
    pub median_sales_growth_ttm_pct: Option<f64>,
    pub median_sales_growth_3y_pct: Option<f64>,
    pub median_sales_growth_5y_pct: Option<f64>,
    pub median_profit_growth_3y_pct: Option<f64>,
    /// Dispersion of 3Y sales CAGR across names (IQR or similar), in percentage points
    pub growth_dispersion_pct: Option<f64>,
    /// Fraction of cohort with positive PAT (0–1)
    pub share_profitable: Option<f64>,
    pub margin_trend: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerBenchmark {
    pub symbol: String,
    pub short_name: Option<String>,
    pub data_quality_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pay_vs_revenue_score: Option<f64>,
    pub pay_to_revenue_pct: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeerAnalysis {
    pub peers: Vec<PeerQuote>,
    pub benchmarks: Vec<PeerBenchmark>,
    pub subject_data_quality_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
pub struct ChartDividendEvent {
    pub date: String,
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ChartSplitEvent {
    pub date: String,
    pub numerator: f64,
    pub denominator: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ChartEvents {
    pub dividends: Vec<ChartDividendEvent>,
    pub splits: Vec<ChartSplitEvent>,
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
    /// Exact Yahoo row used for `net_income` (e.g. "Net Income").
    #[serde(default)]
    pub net_income_yahoo_row: Option<String>,
    /// Yahoo FTS `periodType` (`3M`, `12M`, …). Empty on legacy quoteSummary rows.
    #[serde(default)]
    pub period_type: String,
    pub diluted_eps: Option<f64>,
    #[serde(default)]
    pub other_income_expense: f64,
    #[serde(default)]
    pub net_interest_income: f64,
    #[serde(default)]
    pub interest_income: f64,
    #[serde(default)]
    pub other_income: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct BalanceSheetRow {
    pub end_date_fmt: String,
    pub end_ts: Option<i64>,
    pub cash: f64,
    /// Cash and cash equivalents only (not short-term investments).
    #[serde(default)]
    pub cash_and_cash_equivalents: f64,
    #[serde(default)]
    pub short_term_investments: f64,
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
    #[serde(default)]
    pub goodwill: f64,
    #[serde(default)]
    pub intangible_assets: f64,
    #[serde(default)]
    pub net_loans: f64,
    /// Yahoo row used for `net_loans` (`Net Loan`, `Gross Loan`, …).
    #[serde(default)]
    pub net_loans_yahoo_row: Option<String>,
    #[serde(default)]
    pub total_deposits: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CashflowRow {
    pub end_date_fmt: String,
    pub end_ts: Option<i64>,
    pub operating_cashflow: f64,
    pub capital_expenditure: f64,
    /// Canonical FCF for statement consumers: Yahoo FCF else CFO−capex.
    pub free_cashflow: f64,
    #[serde(default)]
    pub yahoo_free_cashflow: Option<f64>,
    #[serde(default)]
    pub calculated_fcf: Option<f64>,
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
    #[serde(default)]
    pub is_model_assumption: bool,
    #[serde(default)]
    pub assumption_note: String,
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
    /// Newest quarter-end on Yahoo's quarterly income series.
    #[serde(default, alias = "latest_quarter_end")]
    pub latest_yahoo_quarter_end: Option<String>,
    /// Company-reported latest quarter when filing data exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_reported_quarter_end: Option<String>,
    #[serde(default)]
    pub quarterly_statement_stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarterly_statement_age_days: Option<i64>,
    #[serde(default)]
    pub financial_company_type: FinancialCompanyType,
    #[serde(default)]
    pub financial_company_type_label: String,
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
    #[serde(default)]
    pub forward_pe: Option<f64>,
    pub price_to_book: f64,
    pub price_to_sales: f64,
    pub ev_to_ebitda: Option<f64>,
    /// Yahoo statistics `enterpriseToEbitda` when the API exposes it.
    #[serde(default)]
    pub yahoo_ev_to_ebitda: Option<f64>,
    /// Market cap + debt − Yahoo `totalCash` (not statement liquid investments).
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
pub struct TechnicalState {
    pub above_dma50: Option<bool>,
    pub above_dma200: Option<bool>,
    /// RSI < 30
    pub rsi_oversold: Option<bool>,
    /// RSI < 40
    pub rsi_weak: Option<bool>,
    /// RSI < 35 (entry-zone heuristic)
    pub rsi_below_35: Option<bool>,
    pub macd_bullish: Option<bool>,
    pub price_stretched_vs_50: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub financial_quality_score: Option<f64>,
    pub valuation_score: f64,
    pub technical_score: f64,
    /// Risk penalty when assessed. `None` = unassessed (not “no risk”).
    #[serde(alias = "risk_score", default, skip_serializing_if = "Option::is_none")]
    pub risk_penalty: Option<f64>,
    /// Ranking score. `None` when gated — see `provisional_screening_score`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_score: Option<f64>,
    /// True when critical lender coverage is too low for ranking.
    #[serde(default)]
    pub overall_score_provisional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisional_screening_score: Option<f64>,
    /// Yahoo snapshot heuristic (margins/ROE) — not financial quality.
    #[serde(default)]
    pub data_quality_score: f64,
    /// Screening quality used in the composite; `None` when gated.
    #[serde(default)]
    pub screening_quality_score: Option<f64>,
    pub rating_label: String,
    pub fundamental_rating: String,
    pub valuation_rating: String,
    pub technical_rating: String,
    /// Fundamental credit/capital risk. Not market beta.
    pub risk_rating: String,
    /// Same as `risk_rating` — explicit name so it is not confused with beta.
    #[serde(default)]
    pub fundamental_risk_rating: String,
    /// Relative volatility from Yahoo beta only. Not a bank credit-risk call.
    #[serde(default)]
    pub market_beta_risk: String,
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
    #[serde(default)]
    pub action_guidance: ActionGuidance,
    pub disclaimer: String,
    #[serde(default)]
    pub company_type_headline: String,
    #[serde(default)]
    pub executive_blocks: Vec<(String, String)>,
}
