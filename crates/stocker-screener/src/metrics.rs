//! Single source of truth for the screener metric catalog.
//!
//! Every column in `snapshots` and every filter the API exposes derives from
//! this module. The migration in `migrations/0001_init.sql` is hand-mirrored
//! against [`MetricId::ALL`]; [`validate_schema`] checks the runtime DB columns
//! match this enum so the two cannot drift unnoticed.

use serde::{Deserialize, Serialize};

/// Logical category surfaced in the UI's filter builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricCategory {
    PriceRange,
    MarketStructure,
    Valuation,
    IncomeMargins,
    ReturnsEfficiency,
    BalanceSheet,
    CashFlow,
    Technical,
    Composite,
}

impl MetricCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PriceRange => "Price & range",
            Self::MarketStructure => "Market structure",
            Self::Valuation => "Valuation",
            Self::IncomeMargins => "Income & margins",
            Self::ReturnsEfficiency => "Returns / efficiency",
            Self::BalanceSheet => "Balance sheet",
            Self::CashFlow => "Cash flow",
            Self::Technical => "Technical",
            Self::Composite => "Composite scores",
        }
    }
}

/// Where the metric ultimately comes from. Informational only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Direct field from Yahoo's `financialData` / `summaryDetail` / `price`.
    Live,
    /// Derived from `StatementBundle` (annual / quarterly statements).
    Statement,
    /// Derived from daily chart bars.
    Chart,
    /// Composite formula combining the above.
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Indian rupees (per share).
    Rupees,
    /// Indian rupees in crores (10^7) — Yahoo gives raw rupees, we convert at read time when shown to the user.
    RupeesCr,
    /// Decimal percent expressed with `_pct` suffix (5.0 = 5%).
    Percent,
    /// Decimal ratio (0.05 = 5%) — typically Yahoo's native form for yields/margins.
    Ratio,
    /// Pure ratio with no unit (PE, PB).
    Multiple,
    /// Number of shares / contracts.
    Count,
    /// Score / index (Altman Z, F-score).
    Score,
    /// Days.
    Days,
}

/// Closed catalog of every metric exposed in v1.
///
/// Adding a metric is: enum variant + `metric_spec` row + migration column +
/// compute extractor binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricId {
    // -------- Price & range
    CurrentPrice,
    PreviousClose,
    FiftyTwoWeekHigh,
    FiftyTwoWeekLow,
    From52wHighPct,
    UpFrom52wLowPct,
    RegularMarketChangePercent,
    Volume,
    AverageVolume10Day,
    Volume1yAvg,
    Return1wPct,
    Return3mPct,
    Return6mPct,
    Return1yPct,
    Return3yCagrPct,
    Return5yCagrPct,

    // -------- Market structure
    MarketCap,
    EnterpriseValue,
    SharesOutstanding,
    FaceValue,
    McapToSales,
    McapToCfo,
    McapToQuarterlyProfit,

    // -------- Valuation
    PeRatio,
    ForwardPe,
    PriceToBook,
    PriceToSales,
    PriceToFcf,
    PriceToQuarterlyEarning,
    EvToEbitda,
    EvToSales,
    PegRatio,
    EarningsYieldPct,
    DividendYield,
    PbXPe,
    GrahamNumber,
    IntrinsicValue,
    Ncavps,
    EarningPowerPct,
    EpsTtm,
    BookValue,
    BookValuePrecedingYear,
    BookValue3yBack,
    BookValue5yBack,
    HistoricalPe3y,
    HistoricalPe5y,
    HistoricalPe7y,

    // -------- Income & margins
    RevenueTtm,
    SalesLastYear,
    SalesLatestQuarter,
    SalesGrowthTtmPct,
    SalesGrowth3yCagrPct,
    SalesGrowth5yCagrPct,
    SalesGrowth7yCagrPct,
    YoyQuarterlySalesGrowthPct,
    QoqSalesGrowthPct,
    ProfitAfterTaxTtm,
    NetProfitLastYear,
    ProfitAfterTaxLatestQuarter,
    NetProfitPrecedingYearQuarter,
    ProfitBeforeTaxLastYear,
    ProfitGrowthTtmPct,
    ProfitGrowth3yCagrPct,
    ProfitGrowth5yCagrPct,
    YoyQuarterlyProfitGrowthPct,
    QoqProfitGrowthPct,
    Ebitda,
    EbitdaMargins,
    OperatingProfitPrecedingYearQuarter,
    OpmPct,
    NpmLastYearPct,
    NpmPrecedingYearPct,
    NpmLatestQuarterPct,
    NpmPrecedingQuarterPct,
    NpmPrecedingYearQuarterPct,
    GrossMargins,
    DepreciationTtm,
    InterestTtm,
    TaxTtm,
    TaxLastYear,
    TaxPrecedingYearQuarter,
    AvgEbit5y,

    // -------- Returns / efficiency
    ReturnOnEquity,
    ReturnOnAssets,
    ReturnOnCapitalEmployed,
    AvgRoe3y,
    AvgRoe5y,

    // -------- Balance sheet
    TotalAssets,
    NetWorth,
    TotalDebt,
    DebtToEquity,
    CurrentRatio,
    QuickRatio,
    Inventory,
    WorkingCapital,
    WorkingCapitalPrecedingYear,
    WorkingCapital3yBack,
    WorkingCapital5yBack,
    WorkingCapitalDays,
    AvgWorkingCapitalDays3y,
    WorkingCapitalToSalesPct,
    DaysReceivableOutstanding,
    DaysInventoryOutstanding,
    DaysReceivableChange3y,
    DaysInventoryChange3y,
    FinancialLeverage,
    InterestCoverageRatio,

    // -------- Cash flow
    OperatingCashflowTtm,
    FreeCashflowLastYear,
    FreeCashflowTtm,
    FreeCashflow3ySum,
    FreeCashflow5ySum,
    CumulativeCfoPat3y,
    CumulativeCfoPat5y,
    CfoPatLatestYear,

    // -------- Technical
    Dma50,
    Dma200,
    Macd,
    MacdSignal,
    MacdPreviousDay,
    MacdSignalPreviousDay,
    Rsi14,

    // -------- Composite scores
    AltmanZScore,
    PiotroskiFScore,
    GFactor,
    CroicPct,
    DebtCapacity,
    McapToDebtCapacity,
}

/// Metadata for one [`MetricId`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSpec {
    pub id: MetricId,
    pub label: &'static str,
    pub description: &'static str,
    pub category: MetricCategory,
    pub unit: Unit,
    pub source_kind: SourceKind,
    pub column: &'static str,
    /// True if the formula is documented-default and pending user review.
    pub needs_review: bool,
}

impl MetricId {
    /// Snake-case column / serde tag — also the SQL column name in `snapshots`.
    pub fn column(&self) -> &'static str {
        self.spec().column
    }

    pub fn spec(&self) -> &'static MetricSpec {
        // Linear lookup over ~110 specs is fine; called rarely outside of catalog endpoints.
        for spec in CATALOG {
            if spec.id == *self {
                return spec;
            }
        }
        unreachable!("MetricId without spec — catalog out of sync")
    }

    /// Every catalog entry, in catalog order.
    pub const ALL: &'static [MetricId] = &[
        MetricId::CurrentPrice,
        MetricId::PreviousClose,
        MetricId::FiftyTwoWeekHigh,
        MetricId::FiftyTwoWeekLow,
        MetricId::From52wHighPct,
        MetricId::UpFrom52wLowPct,
        MetricId::RegularMarketChangePercent,
        MetricId::Volume,
        MetricId::AverageVolume10Day,
        MetricId::Volume1yAvg,
        MetricId::Return1wPct,
        MetricId::Return3mPct,
        MetricId::Return6mPct,
        MetricId::Return1yPct,
        MetricId::Return3yCagrPct,
        MetricId::Return5yCagrPct,
        MetricId::MarketCap,
        MetricId::EnterpriseValue,
        MetricId::SharesOutstanding,
        MetricId::FaceValue,
        MetricId::McapToSales,
        MetricId::McapToCfo,
        MetricId::McapToQuarterlyProfit,
        MetricId::PeRatio,
        MetricId::ForwardPe,
        MetricId::PriceToBook,
        MetricId::PriceToSales,
        MetricId::PriceToFcf,
        MetricId::PriceToQuarterlyEarning,
        MetricId::EvToEbitda,
        MetricId::EvToSales,
        MetricId::PegRatio,
        MetricId::EarningsYieldPct,
        MetricId::DividendYield,
        MetricId::PbXPe,
        MetricId::GrahamNumber,
        MetricId::IntrinsicValue,
        MetricId::Ncavps,
        MetricId::EarningPowerPct,
        MetricId::EpsTtm,
        MetricId::BookValue,
        MetricId::BookValuePrecedingYear,
        MetricId::BookValue3yBack,
        MetricId::BookValue5yBack,
        MetricId::HistoricalPe3y,
        MetricId::HistoricalPe5y,
        MetricId::HistoricalPe7y,
        MetricId::RevenueTtm,
        MetricId::SalesLastYear,
        MetricId::SalesLatestQuarter,
        MetricId::SalesGrowthTtmPct,
        MetricId::SalesGrowth3yCagrPct,
        MetricId::SalesGrowth5yCagrPct,
        MetricId::SalesGrowth7yCagrPct,
        MetricId::YoyQuarterlySalesGrowthPct,
        MetricId::QoqSalesGrowthPct,
        MetricId::ProfitAfterTaxTtm,
        MetricId::NetProfitLastYear,
        MetricId::ProfitAfterTaxLatestQuarter,
        MetricId::NetProfitPrecedingYearQuarter,
        MetricId::ProfitBeforeTaxLastYear,
        MetricId::ProfitGrowthTtmPct,
        MetricId::ProfitGrowth3yCagrPct,
        MetricId::ProfitGrowth5yCagrPct,
        MetricId::YoyQuarterlyProfitGrowthPct,
        MetricId::QoqProfitGrowthPct,
        MetricId::Ebitda,
        MetricId::EbitdaMargins,
        MetricId::OperatingProfitPrecedingYearQuarter,
        MetricId::OpmPct,
        MetricId::NpmLastYearPct,
        MetricId::NpmPrecedingYearPct,
        MetricId::NpmLatestQuarterPct,
        MetricId::NpmPrecedingQuarterPct,
        MetricId::NpmPrecedingYearQuarterPct,
        MetricId::GrossMargins,
        MetricId::DepreciationTtm,
        MetricId::InterestTtm,
        MetricId::TaxTtm,
        MetricId::TaxLastYear,
        MetricId::TaxPrecedingYearQuarter,
        MetricId::AvgEbit5y,
        MetricId::ReturnOnEquity,
        MetricId::ReturnOnAssets,
        MetricId::ReturnOnCapitalEmployed,
        MetricId::AvgRoe3y,
        MetricId::AvgRoe5y,
        MetricId::TotalAssets,
        MetricId::NetWorth,
        MetricId::TotalDebt,
        MetricId::DebtToEquity,
        MetricId::CurrentRatio,
        MetricId::QuickRatio,
        MetricId::Inventory,
        MetricId::WorkingCapital,
        MetricId::WorkingCapitalPrecedingYear,
        MetricId::WorkingCapital3yBack,
        MetricId::WorkingCapital5yBack,
        MetricId::WorkingCapitalDays,
        MetricId::AvgWorkingCapitalDays3y,
        MetricId::WorkingCapitalToSalesPct,
        MetricId::DaysReceivableOutstanding,
        MetricId::DaysInventoryOutstanding,
        MetricId::DaysReceivableChange3y,
        MetricId::DaysInventoryChange3y,
        MetricId::FinancialLeverage,
        MetricId::InterestCoverageRatio,
        MetricId::OperatingCashflowTtm,
        MetricId::FreeCashflowLastYear,
        MetricId::FreeCashflowTtm,
        MetricId::FreeCashflow3ySum,
        MetricId::FreeCashflow5ySum,
        MetricId::CumulativeCfoPat3y,
        MetricId::CumulativeCfoPat5y,
        MetricId::CfoPatLatestYear,
        MetricId::Dma50,
        MetricId::Dma200,
        MetricId::Macd,
        MetricId::MacdSignal,
        MetricId::MacdPreviousDay,
        MetricId::MacdSignalPreviousDay,
        MetricId::Rsi14,
        MetricId::AltmanZScore,
        MetricId::PiotroskiFScore,
        MetricId::GFactor,
        MetricId::CroicPct,
        MetricId::DebtCapacity,
        MetricId::McapToDebtCapacity,
    ];
}

pub const CATALOG: &[MetricSpec] = &[
    // ===== Price & range =====
    MetricSpec { id: MetricId::CurrentPrice, label: "Current price", description: "Last closing price of the stock.", category: MetricCategory::PriceRange, unit: Unit::Rupees, source_kind: SourceKind::Live, column: "current_price", needs_review: false },
    MetricSpec { id: MetricId::PreviousClose, label: "Previous close", description: "Previous trading session's close.", category: MetricCategory::PriceRange, unit: Unit::Rupees, source_kind: SourceKind::Live, column: "previous_close", needs_review: false },
    MetricSpec { id: MetricId::FiftyTwoWeekHigh, label: "52w high", description: "52 week high price.", category: MetricCategory::PriceRange, unit: Unit::Rupees, source_kind: SourceKind::Chart, column: "fifty_two_week_high", needs_review: false },
    MetricSpec { id: MetricId::FiftyTwoWeekLow, label: "52w low", description: "52 week low price.", category: MetricCategory::PriceRange, unit: Unit::Rupees, source_kind: SourceKind::Chart, column: "fifty_two_week_low", needs_review: false },
    MetricSpec { id: MetricId::From52wHighPct, label: "From 52w high", description: "Distance below the 52 week high (positive percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "from_52w_high_pct", needs_review: false },
    MetricSpec { id: MetricId::UpFrom52wLowPct, label: "Up from 52w low", description: "Percent rise above the 52 week low.", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "up_from_52w_low_pct", needs_review: false },
    MetricSpec { id: MetricId::RegularMarketChangePercent, label: "Day change %", description: "Session move vs previous close (decimal percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "regular_market_change_percent", needs_review: false },
    MetricSpec { id: MetricId::Volume, label: "Volume", description: "Quantity traded on last trade date.", category: MetricCategory::PriceRange, unit: Unit::Count, source_kind: SourceKind::Live, column: "volume", needs_review: false },
    MetricSpec { id: MetricId::AverageVolume10Day, label: "Avg volume 10D", description: "Average daily volume over the last 10 sessions.", category: MetricCategory::PriceRange, unit: Unit::Count, source_kind: SourceKind::Live, column: "average_volume_10_day", needs_review: false },
    MetricSpec { id: MetricId::Volume1yAvg, label: "Volume 1Y average", description: "Average daily volume over the last 1 year.", category: MetricCategory::PriceRange, unit: Unit::Count, source_kind: SourceKind::Chart, column: "volume_1y_avg", needs_review: false },
    MetricSpec { id: MetricId::Return1wPct, label: "Return 1W", description: "Price change over last 1 week (percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_1w_pct", needs_review: false },
    MetricSpec { id: MetricId::Return3mPct, label: "Return 3M", description: "Price change over last 3 months (percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_3m_pct", needs_review: false },
    MetricSpec { id: MetricId::Return6mPct, label: "Return 6M", description: "Price change over last 6 months (percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_6m_pct", needs_review: false },
    MetricSpec { id: MetricId::Return1yPct, label: "Return 1Y", description: "Price change over last 1 year (percent).", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_1y_pct", needs_review: false },
    MetricSpec { id: MetricId::Return3yCagrPct, label: "Return 3Y CAGR", description: "Price CAGR over last 3 years.", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_3y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::Return5yCagrPct, label: "Return 5Y CAGR", description: "Price CAGR over last 5 years.", category: MetricCategory::PriceRange, unit: Unit::Percent, source_kind: SourceKind::Chart, column: "return_5y_cagr_pct", needs_review: false },

    // ===== Market structure =====
    MetricSpec { id: MetricId::MarketCap, label: "Market Cap", description: "Market Capitalization at current price.", category: MetricCategory::MarketStructure, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "market_cap", needs_review: false },
    MetricSpec { id: MetricId::EnterpriseValue, label: "Enterprise Value", description: "Market cap + debt - cash.", category: MetricCategory::MarketStructure, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "enterprise_value", needs_review: false },
    MetricSpec { id: MetricId::SharesOutstanding, label: "Shares Outstanding", description: "Number of equity shares outstanding.", category: MetricCategory::MarketStructure, unit: Unit::Count, source_kind: SourceKind::Live, column: "shares_outstanding", needs_review: false },
    MetricSpec { id: MetricId::FaceValue, label: "Face value", description: "Face value per share from NSE EQUITY_L.csv (falls back to Yahoo when CSV missing).", category: MetricCategory::MarketStructure, unit: Unit::Rupees, source_kind: SourceKind::Statement, column: "face_value", needs_review: false },
    MetricSpec { id: MetricId::McapToSales, label: "Market Cap to Sales", description: "Market Cap to Sales ratio.", category: MetricCategory::MarketStructure, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "mcap_to_sales", needs_review: false },
    MetricSpec { id: MetricId::McapToCfo, label: "Market Cap to CFO", description: "Market cap / cash from operating activities.", category: MetricCategory::MarketStructure, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "mcap_to_cfo", needs_review: false },
    MetricSpec { id: MetricId::McapToQuarterlyProfit, label: "Market cap to quarterly profit", description: "Market cap / latest quarter PAT.", category: MetricCategory::MarketStructure, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "mcap_to_quarterly_profit", needs_review: false },

    // ===== Valuation =====
    MetricSpec { id: MetricId::PeRatio, label: "Price to Earning", description: "Trailing 12M PE.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "pe_ratio", needs_review: false },
    MetricSpec { id: MetricId::ForwardPe, label: "Forward PE", description: "Forward PE from analyst estimates.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "forward_pe", needs_review: false },
    MetricSpec { id: MetricId::PriceToBook, label: "Price to Book", description: "Price / book value per share.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "price_to_book", needs_review: false },
    MetricSpec { id: MetricId::PriceToSales, label: "Price to Sales", description: "Price / TTM sales per share.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "price_to_sales", needs_review: false },
    MetricSpec { id: MetricId::PriceToFcf, label: "Price to FCF", description: "Price / 3-year average free cash flow per share.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "price_to_fcf", needs_review: false },
    MetricSpec { id: MetricId::PriceToQuarterlyEarning, label: "Price to Quarterly Earning", description: "Price / (latest quarterly earning x 4).", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "price_to_quarterly_earning", needs_review: false },
    MetricSpec { id: MetricId::EvToEbitda, label: "EV/EBITDA", description: "Enterprise Value / EBITDA.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "ev_to_ebitda", needs_review: false },
    MetricSpec { id: MetricId::EvToSales, label: "EV/Sales", description: "Enterprise Value / TTM revenue.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "ev_to_sales", needs_review: false },
    MetricSpec { id: MetricId::PegRatio, label: "PEG Ratio", description: "PE / 3y profit growth.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "peg_ratio", needs_review: false },
    MetricSpec { id: MetricId::EarningsYieldPct, label: "Earnings yield", description: "Greenblatt-style earnings yield (Trailing EBIT / Enterprise Value).", category: MetricCategory::Valuation, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "earnings_yield_pct", needs_review: false },
    MetricSpec { id: MetricId::DividendYield, label: "Dividend yield", description: "Dividend yield excluding special dividends (decimal ratio).", category: MetricCategory::Valuation, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "dividend_yield", needs_review: false },
    MetricSpec { id: MetricId::PbXPe, label: "P/B x P/E", description: "Graham's heuristic: P/B times P/E. Cap of 22.5 considered safe.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "pb_x_pe", needs_review: false },
    MetricSpec { id: MetricId::GrahamNumber, label: "Graham number", description: "sqrt(22.5 x EPS x BV).", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Composite, column: "graham_number", needs_review: false },
    MetricSpec { id: MetricId::IntrinsicValue, label: "Intrinsic Value", description: "Intrinsic value based on modified Graham.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Composite, column: "intrinsic_value", needs_review: true },
    MetricSpec { id: MetricId::Ncavps, label: "NCAVPS", description: "Net Current Asset Value Per Share = working capital / shares.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Composite, column: "ncavps", needs_review: false },
    MetricSpec { id: MetricId::EarningPowerPct, label: "Earning Power", description: "Basic Earning Power = EBIT / Total Assets.", category: MetricCategory::Valuation, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "earning_power_pct", needs_review: false },
    MetricSpec { id: MetricId::EpsTtm, label: "EPS (TTM)", description: "Earning per share over last 4 quarters.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Live, column: "eps_ttm", needs_review: false },
    MetricSpec { id: MetricId::BookValue, label: "Book value", description: "Book value per share.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Live, column: "book_value", needs_review: false },
    MetricSpec { id: MetricId::BookValuePrecedingYear, label: "Book value preceding year", description: "Book value per share one year ago.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Statement, column: "book_value_preceding_year", needs_review: false },
    MetricSpec { id: MetricId::BookValue3yBack, label: "Book value 3Y back", description: "Book value per share 3 years back.", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Statement, column: "book_value_3y_back", needs_review: false },
    MetricSpec { id: MetricId::BookValue5yBack, label: "Book value 5Y back", description: "Book value per share 5 years back (when statement depth allows).", category: MetricCategory::Valuation, unit: Unit::Rupees, source_kind: SourceKind::Statement, column: "book_value_5y_back", needs_review: false },
    MetricSpec { id: MetricId::HistoricalPe3y, label: "Historical PE 3Y", description: "Median price-to-earnings during last 3 years.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "historical_pe_3y", needs_review: false },
    MetricSpec { id: MetricId::HistoricalPe5y, label: "Historical PE 5Y", description: "Median price-to-earnings during last 5 years.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "historical_pe_5y", needs_review: false },
    MetricSpec { id: MetricId::HistoricalPe7y, label: "Historical PE 7Y", description: "Median price-to-earnings during last 7 years.", category: MetricCategory::Valuation, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "historical_pe_7y", needs_review: false },

    // ===== Income & margins =====
    MetricSpec { id: MetricId::RevenueTtm, label: "Sales (TTM)", description: "Trailing 12 months sales.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "revenue_ttm", needs_review: false },
    MetricSpec { id: MetricId::SalesLastYear, label: "Sales last year", description: "Sales as per last annual report.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "sales_last_year", needs_review: false },
    MetricSpec { id: MetricId::SalesLatestQuarter, label: "Sales latest quarter", description: "Sales as per latest quarterly results.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "sales_latest_quarter", needs_review: false },
    MetricSpec { id: MetricId::SalesGrowthTtmPct, label: "Sales growth (TTM)", description: "TTM sales growth: last 4Q vs preceding 4Q.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "sales_growth_ttm_pct", needs_review: false },
    MetricSpec { id: MetricId::SalesGrowth3yCagrPct, label: "Sales growth 3Y CAGR", description: "Compounded sales growth over 3 years.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "sales_growth_3y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::SalesGrowth5yCagrPct, label: "Sales growth 5Y CAGR", description: "Compounded sales growth over 5 years.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "sales_growth_5y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::SalesGrowth7yCagrPct, label: "Sales growth 7Y CAGR", description: "Compounded sales growth over 7 years.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "sales_growth_7y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::YoyQuarterlySalesGrowthPct, label: "YOY quarterly sales growth", description: "Year on year growth of latest quarter sales vs same quarter previous year.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "yoy_quarterly_sales_growth_pct", needs_review: false },
    MetricSpec { id: MetricId::QoqSalesGrowthPct, label: "QoQ sales growth", description: "Quarter on quarter sales growth.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "qoq_sales_growth_pct", needs_review: false },
    MetricSpec { id: MetricId::ProfitAfterTaxTtm, label: "Profit after tax", description: "PAT excluding extra-ordinary items during last 12 months.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "profit_after_tax_ttm", needs_review: false },
    MetricSpec { id: MetricId::NetProfitLastYear, label: "Net profit last year", description: "Net profit per last annual report.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "net_profit_last_year", needs_review: false },
    MetricSpec { id: MetricId::ProfitAfterTaxLatestQuarter, label: "PAT latest quarter", description: "PAT (excluding extraordinary items) as per latest quarterly results.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "profit_after_tax_latest_quarter", needs_review: false },
    MetricSpec { id: MetricId::NetProfitPrecedingYearQuarter, label: "Net profit preceding year quarter", description: "Net profit as per previous year's corresponding quarter.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "net_profit_preceding_year_quarter", needs_review: false },
    MetricSpec { id: MetricId::ProfitBeforeTaxLastYear, label: "Profit before tax last year", description: "PBT in last annual report.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "profit_before_tax_last_year", needs_review: false },
    MetricSpec { id: MetricId::ProfitGrowthTtmPct, label: "Profit growth (TTM)", description: "TTM profit growth: last 4Q vs preceding 4Q.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "profit_growth_ttm_pct", needs_review: false },
    MetricSpec { id: MetricId::ProfitGrowth3yCagrPct, label: "Profit growth 3Y CAGR", description: "Compounded profit growth over 3 years.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "profit_growth_3y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::ProfitGrowth5yCagrPct, label: "Profit growth 5Y CAGR", description: "Compounded profit growth over 5 years.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "profit_growth_5y_cagr_pct", needs_review: false },
    MetricSpec { id: MetricId::YoyQuarterlyProfitGrowthPct, label: "YOY quarterly profit growth", description: "YoY growth in quarterly profit after tax.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "yoy_quarterly_profit_growth_pct", needs_review: false },
    MetricSpec { id: MetricId::QoqProfitGrowthPct, label: "QoQ profit growth", description: "Quarter on quarter profit growth.", category: MetricCategory::IncomeMargins, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "qoq_profit_growth_pct", needs_review: false },
    MetricSpec { id: MetricId::Ebitda, label: "EBITDA", description: "Earnings before interest, tax, depreciation, amortisation.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "ebitda", needs_review: false },
    MetricSpec { id: MetricId::EbitdaMargins, label: "EBITDA margin", description: "EBITDA / revenue (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "ebitda_margins", needs_review: false },
    MetricSpec { id: MetricId::OperatingProfitPrecedingYearQuarter, label: "Operating profit preceding year quarter", description: "Operating profit in previous year's corresponding quarter.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "operating_profit_preceding_year_quarter", needs_review: false },
    MetricSpec { id: MetricId::OpmPct, label: "OPM", description: "Operating profit margin (TTM, decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "opm_pct", needs_review: false },
    MetricSpec { id: MetricId::NpmLastYearPct, label: "NPM last year", description: "Net profit margin in last annual report (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "npm_last_year_pct", needs_review: false },
    MetricSpec { id: MetricId::NpmPrecedingYearPct, label: "NPM preceding year", description: "Net profit margin two annual reports ago (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "npm_preceding_year_pct", needs_review: false },
    MetricSpec { id: MetricId::NpmLatestQuarterPct, label: "NPM latest quarter", description: "Net profit margin in latest quarterly result (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "npm_latest_quarter_pct", needs_review: false },
    MetricSpec { id: MetricId::NpmPrecedingQuarterPct, label: "NPM preceding quarter", description: "Net profit margin in preceding quarterly result (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "npm_preceding_quarter_pct", needs_review: false },
    MetricSpec { id: MetricId::NpmPrecedingYearQuarterPct, label: "NPM preceding year quarter", description: "Net profit margin in previous year's corresponding quarter.", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "npm_preceding_year_quarter_pct", needs_review: false },
    MetricSpec { id: MetricId::GrossMargins, label: "Gross margin", description: "Gross margin (decimal).", category: MetricCategory::IncomeMargins, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "gross_margins", needs_review: false },
    MetricSpec { id: MetricId::DepreciationTtm, label: "Depreciation (TTM)", description: "Sum of depreciation in last 4 quarters.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "depreciation_ttm", needs_review: false },
    MetricSpec { id: MetricId::InterestTtm, label: "Interest (TTM)", description: "Sum of interest expense in last 4 quarters.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "interest_ttm", needs_review: false },
    MetricSpec { id: MetricId::TaxTtm, label: "Tax (TTM)", description: "Tax expense over last 4 quarters.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "tax_ttm", needs_review: false },
    MetricSpec { id: MetricId::TaxLastYear, label: "Tax last year", description: "Tax expense in last annual report.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "tax_last_year", needs_review: false },
    MetricSpec { id: MetricId::TaxPrecedingYearQuarter, label: "Tax preceding year quarter", description: "Tax expense in previous year's corresponding quarter.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "tax_preceding_year_quarter", needs_review: false },
    MetricSpec { id: MetricId::AvgEbit5y, label: "Average EBIT 5Y", description: "Average EBIT of last 5 annual results.", category: MetricCategory::IncomeMargins, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "avg_ebit_5y", needs_review: false },

    // ===== Returns / efficiency =====
    MetricSpec { id: MetricId::ReturnOnEquity, label: "Return on equity", description: "ROE on average equity (decimal).", category: MetricCategory::ReturnsEfficiency, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "return_on_equity", needs_review: false },
    MetricSpec { id: MetricId::ReturnOnAssets, label: "Return on assets", description: "Net profit / average total assets.", category: MetricCategory::ReturnsEfficiency, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "return_on_assets", needs_review: false },
    MetricSpec { id: MetricId::ReturnOnCapitalEmployed, label: "Return on capital employed", description: "EBIT / annual average capital employed (decimal).", category: MetricCategory::ReturnsEfficiency, unit: Unit::Ratio, source_kind: SourceKind::Live, column: "return_on_capital_employed", needs_review: false },
    MetricSpec { id: MetricId::AvgRoe3y, label: "Average ROE 3Y", description: "Weighted average of ROE over last 3 years (decimal).", category: MetricCategory::ReturnsEfficiency, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "avg_roe_3y", needs_review: false },
    MetricSpec { id: MetricId::AvgRoe5y, label: "Average ROE 5Y", description: "Weighted average of ROE over last 5 years (decimal).", category: MetricCategory::ReturnsEfficiency, unit: Unit::Ratio, source_kind: SourceKind::Statement, column: "avg_roe_5y", needs_review: false },

    // ===== Balance sheet =====
    MetricSpec { id: MetricId::TotalAssets, label: "Total Assets", description: "Total assets per latest balance sheet (alias 'Balance sheet total').", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "total_assets", needs_review: false },
    MetricSpec { id: MetricId::NetWorth, label: "Net worth", description: "Total book value (equity + reserves).", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "net_worth", needs_review: false },
    MetricSpec { id: MetricId::TotalDebt, label: "Total debt", description: "Total borrowings per latest annual numbers.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "total_debt", needs_review: false },
    MetricSpec { id: MetricId::DebtToEquity, label: "Debt to equity", description: "Total debt / shareholder equity (multiple).", category: MetricCategory::BalanceSheet, unit: Unit::Multiple, source_kind: SourceKind::Live, column: "debt_to_equity", needs_review: false },
    MetricSpec { id: MetricId::CurrentRatio, label: "Current ratio", description: "Current assets / current liabilities.", category: MetricCategory::BalanceSheet, unit: Unit::Multiple, source_kind: SourceKind::Statement, column: "current_ratio", needs_review: false },
    MetricSpec { id: MetricId::QuickRatio, label: "Quick ratio", description: "(Current assets - inventory) / current liabilities.", category: MetricCategory::BalanceSheet, unit: Unit::Multiple, source_kind: SourceKind::Statement, column: "quick_ratio", needs_review: false },
    MetricSpec { id: MetricId::Inventory, label: "Inventory", description: "Inventory in latest annual report.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "inventory", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapital, label: "Working capital", description: "Current assets - current liabilities.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "working_capital", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapitalPrecedingYear, label: "Working capital preceding year", description: "Working capital one annual report ago.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "working_capital_preceding_year", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapital3yBack, label: "Working capital 3Y back", description: "Working capital 3 years back.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "working_capital_3y_back", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapital5yBack, label: "Working capital 5Y back", description: "Working capital 5 years back.", category: MetricCategory::BalanceSheet, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "working_capital_5y_back", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapitalDays, label: "Working Capital Days", description: "(Working capital / sales) x 365.", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "working_capital_days", needs_review: false },
    MetricSpec { id: MetricId::AvgWorkingCapitalDays3y, label: "Avg working capital days 3Y", description: "Average working capital days over last 3 years.", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "avg_working_capital_days_3y", needs_review: false },
    MetricSpec { id: MetricId::WorkingCapitalToSalesPct, label: "Working capital to sales", description: "Working capital as percent of sales.", category: MetricCategory::BalanceSheet, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "working_capital_to_sales_pct", needs_review: false },
    MetricSpec { id: MetricId::DaysReceivableOutstanding, label: "Days receivable outstanding", description: "Net receivables / sales x 365 (alias 'debtor days').", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "days_receivable_outstanding", needs_review: false },
    MetricSpec { id: MetricId::DaysInventoryOutstanding, label: "Days inventory outstanding", description: "Inventory / cost of revenue x 365.", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "days_inventory_outstanding", needs_review: false },
    MetricSpec { id: MetricId::DaysReceivableChange3y, label: "Receivable days change (3Y)", description: "Latest receivable days minus value ~2 years earlier (positive = slower collections).", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "days_receivable_change_3y", needs_review: false },
    MetricSpec { id: MetricId::DaysInventoryChange3y, label: "Inventory days change (3Y)", description: "Latest inventory days minus value ~2 years earlier (positive = slower turnover).", category: MetricCategory::BalanceSheet, unit: Unit::Days, source_kind: SourceKind::Composite, column: "days_inventory_change_3y", needs_review: false },
    MetricSpec { id: MetricId::FinancialLeverage, label: "Financial leverage", description: "Average total assets / net worth.", category: MetricCategory::BalanceSheet, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "financial_leverage", needs_review: false },
    MetricSpec { id: MetricId::InterestCoverageRatio, label: "Interest coverage ratio", description: "EBIT / interest expense.", category: MetricCategory::BalanceSheet, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "interest_coverage_ratio", needs_review: false },

    // ===== Cash flow =====
    MetricSpec { id: MetricId::OperatingCashflowTtm, label: "CFO (TTM)", description: "Cash from operations, trailing 12 months.", category: MetricCategory::CashFlow, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "operating_cashflow_ttm", needs_review: false },
    MetricSpec { id: MetricId::FreeCashflowLastYear, label: "Free cash flow last year", description: "FCF in latest annual cash flow statement.", category: MetricCategory::CashFlow, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "free_cashflow_last_year", needs_review: false },
    MetricSpec { id: MetricId::FreeCashflowTtm, label: "Free cash flow (TTM)", description: "Trailing 12 month free cash flow.", category: MetricCategory::CashFlow, unit: Unit::RupeesCr, source_kind: SourceKind::Live, column: "free_cashflow_ttm", needs_review: false },
    MetricSpec { id: MetricId::FreeCashflow3ySum, label: "Free cash flow 3Y sum", description: "Sum of free cash flow over last 3 years.", category: MetricCategory::CashFlow, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "free_cashflow_3y_sum", needs_review: false },
    MetricSpec { id: MetricId::FreeCashflow5ySum, label: "Free cash flow 5Y sum", description: "Sum of free cash flow over last 5 years.", category: MetricCategory::CashFlow, unit: Unit::RupeesCr, source_kind: SourceKind::Statement, column: "free_cashflow_5y_sum", needs_review: false },
    MetricSpec { id: MetricId::CumulativeCfoPat3y, label: "Cumulative CFO / PAT (3Y)", description: "Sum of operating cash flow / sum of PAT over last 3 annual reports. ≥1.0 indicates earnings backed by cash.", category: MetricCategory::CashFlow, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "cumulative_cfo_pat_3y", needs_review: false },
    MetricSpec { id: MetricId::CumulativeCfoPat5y, label: "Cumulative CFO / PAT (5Y)", description: "Sum of operating cash flow / sum of PAT over last 5 annual reports.", category: MetricCategory::CashFlow, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "cumulative_cfo_pat_5y", needs_review: false },
    MetricSpec { id: MetricId::CfoPatLatestYear, label: "CFO / PAT (latest year)", description: "Operating cash flow / PAT in the latest annual report.", category: MetricCategory::CashFlow, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "cfo_pat_latest_year", needs_review: false },

    // ===== Technical =====
    MetricSpec { id: MetricId::Dma50, label: "DMA 50", description: "50-day exponential moving average of close.", category: MetricCategory::Technical, unit: Unit::Rupees, source_kind: SourceKind::Chart, column: "dma_50", needs_review: false },
    MetricSpec { id: MetricId::Dma200, label: "DMA 200", description: "200-day exponential moving average of close.", category: MetricCategory::Technical, unit: Unit::Rupees, source_kind: SourceKind::Chart, column: "dma_200", needs_review: false },
    MetricSpec { id: MetricId::Macd, label: "MACD", description: "Last value of MACD line.", category: MetricCategory::Technical, unit: Unit::Multiple, source_kind: SourceKind::Chart, column: "macd", needs_review: false },
    MetricSpec { id: MetricId::MacdSignal, label: "MACD signal", description: "Last value of MACD signal line.", category: MetricCategory::Technical, unit: Unit::Multiple, source_kind: SourceKind::Chart, column: "macd_signal", needs_review: false },
    MetricSpec { id: MetricId::MacdPreviousDay, label: "MACD previous day", description: "MACD value of previous day.", category: MetricCategory::Technical, unit: Unit::Multiple, source_kind: SourceKind::Chart, column: "macd_previous_day", needs_review: false },
    MetricSpec { id: MetricId::MacdSignalPreviousDay, label: "MACD signal previous day", description: "MACD signal of previous day.", category: MetricCategory::Technical, unit: Unit::Multiple, source_kind: SourceKind::Chart, column: "macd_signal_previous_day", needs_review: false },
    MetricSpec { id: MetricId::Rsi14, label: "RSI 14", description: "14-day relative strength index.", category: MetricCategory::Technical, unit: Unit::Multiple, source_kind: SourceKind::Chart, column: "rsi_14", needs_review: false },

    // ===== Composite scores =====
    MetricSpec { id: MetricId::AltmanZScore, label: "Altman Z Score", description: "Classic 5-factor Altman Z (manufacturing variant).", category: MetricCategory::Composite, unit: Unit::Score, source_kind: SourceKind::Composite, column: "altman_z_score", needs_review: false },
    MetricSpec { id: MetricId::PiotroskiFScore, label: "Piotroski F-Score", description: "Standard 9-point Piotroski F-Score.", category: MetricCategory::Composite, unit: Unit::Score, source_kind: SourceKind::Composite, column: "piotroski_f_score", needs_review: false },
    MetricSpec { id: MetricId::GFactor, label: "G Factor", description: "Pabrai-style 10-point growth+quality score; >=7 considered healthy. Default formula.", category: MetricCategory::Composite, unit: Unit::Score, source_kind: SourceKind::Composite, column: "g_factor", needs_review: true },
    MetricSpec { id: MetricId::CroicPct, label: "CROIC", description: "Cash return on invested capital: 3y avg FCF / (Net Worth + Total Debt - Cash).", category: MetricCategory::Composite, unit: Unit::Percent, source_kind: SourceKind::Composite, column: "croic_pct", needs_review: false },
    MetricSpec { id: MetricId::DebtCapacity, label: "Debt Capacity", description: "Default: (EBITDA x 5 - Total Debt) / Net Worth. >1 indicates a debt-capacity bargain.", category: MetricCategory::Composite, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "debt_capacity", needs_review: true },
    MetricSpec { id: MetricId::McapToDebtCapacity, label: "Market Cap to Debt Capacity", description: "Default: Market Cap / (EBITDA x 5). <1 considered favourable.", category: MetricCategory::Composite, unit: Unit::Multiple, source_kind: SourceKind::Composite, column: "mcap_to_debt_capacity", needs_review: true },
];

/// Verify there is exactly one [`MetricSpec`] per [`MetricId`] and column names are unique.
/// Used by tests and at process start in `ScreenerService::open`.
pub fn validate_catalog() -> Result<(), String> {
    if CATALOG.len() != MetricId::ALL.len() {
        return Err(format!(
            "catalog length {} does not match MetricId::ALL ({})",
            CATALOG.len(),
            MetricId::ALL.len()
        ));
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_cols = std::collections::HashSet::new();
    for spec in CATALOG {
        if !seen_ids.insert(spec.id) {
            return Err(format!("duplicate MetricId in catalog: {:?}", spec.id));
        }
        if !seen_cols.insert(spec.column) {
            return Err(format!("duplicate column name in catalog: {}", spec.column));
        }
    }
    for id in MetricId::ALL {
        if !seen_ids.contains(id) {
            return Err(format!("MetricId::ALL contains {:?} which is missing from CATALOG", id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_internally_consistent() {
        validate_catalog().unwrap();
    }

    #[test]
    fn every_metric_has_a_spec() {
        for id in MetricId::ALL {
            let _ = id.spec();
            assert!(!id.column().is_empty());
        }
    }
}
