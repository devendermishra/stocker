//! Pure domain rules for research, screening, and the investment ledger.
//!
//! This crate does not fetch market data, open a database, or read the filesystem.
//! Callers pass an `as_of` date when a rule depends on the clock.

mod error;

pub mod analysis;
pub mod canonical;
pub mod financial_company;
pub mod financial_strength_audit;
pub mod fundamental_analysis;
pub mod math;
pub mod mf;
pub mod models;
pub mod portfolio;
pub mod ratios;
pub mod report;
pub mod research_summary;
pub mod screener;
pub mod sector_research;
pub mod series_integrity;
pub mod statements;
pub mod stock_scoring;
pub mod symbol;
pub mod technical_analysis;
pub mod technical_entry_signal;
pub mod valuation_analysis;

pub use canonical::{build_canonical, build_canonical_for, peer_comparability};
pub use error::DomainError;
pub use financial_strength_audit::{
    build_action_guidance, build_financial_strength_audit, build_financial_strength_audit_for,
    cumulative_cfo_pat_for_bundle,
};
pub use math::{cagr, median, pct_change};
pub use models::{
    ActionGuidance, AnalystRecommendationPeriod, AnalystRecommendations, AnnualReport,
    AssetProfile, AuditChecklistItem, AuditStatus, BalanceSheetRow, BankingMetrics, CanonicalMetrics,
    CashFlowQuality, CashflowRow, ChartBar, ChartDividendEvent, ChartEvents, ChartHistory,
    ChartSplitEvent, CompanyOverview, CompetitionNature, CompetitionStructure, CoverageDimension,
    DataCoverage, DemandSupplyAssessment, DemandSupplyGap, EarningsBasedValue, FinancialCompanyType,
    FinancialStrengthAudit, Financials, FinancialsApplicable, FinancialsRawYahoo,
    FundamentalAnalysis, FundamentalSection, GrowthProspects, GrowthProspectsLevel,
    HistoricalMultiples, IncomeStatementRow, IndustryPricingPower, InsiderTransaction,
    InstitutionalHolder, ManagementAnalysis, MarketSignals, MonitorableItem, NewsItem,
    PeerAnalysis, PeerBenchmark, PeerComparisonRow, PeerQuote, PeerValuationCompare,
    PorterFiveForces, PorterForce, PricingPowerLevel, PricingPowerSide, ProfitabilityAssessment,
    ProfitabilityLevel, RawBalanceSheetMetrics, ReportInsights, ResearchRating, ResearchSummary,
    RiskBuckets, RiskCategory, RiskItem, ScenarioAnalysis, ScoreBreakdown, ScoreExplanation,
    ScreenerMetricSnapshot, SectorAnalysisDetail, SectorLifecycle, SectorLifecyclePhase,
    SectorResearchInputs, SectorResearchProfile, SectorTypeAssessment, SectorTypeKind,
    Shareholders, StatementBundle, StockAnalysis, StructuredResearchSections, TechnicalAnalysis,
    TechnicalEntrySignal, TechnicalMomentum, TechnicalState, TechnicalTrend, TechnicalVolatility,
    TechnicalVolume, ValuationAnalysis,
};
pub use ratios::{
    canonical_statement_fcf, earnings_yield_eps_pct, ev_to_ebitda, ev_to_sales, fcf_yield_pct,
    local_roce, normalize_debt_to_equity, normalize_debt_to_equity_opt, report_fcf,
    resolve_enterprise_value, row_statement_fcf,
};
pub use report::ResearchReport;
pub use sector_research::{
    compute_sector_research_profile, sector_inputs_from_aggregates, sector_inputs_from_quotes,
    SectorCohortRow,
};
pub use symbol::{
    india_base_symbol, india_display_ticker, india_exchange_label, normalize_nse_symbol,
    nse_csv_indices, parse_nse_bases_from_csv, resolve_india_symbol, to_yahoo_bse_symbol,
    IndiaSymbolContext, NseCsvIndices,
};
