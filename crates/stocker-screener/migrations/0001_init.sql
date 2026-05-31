-- Initial schema for the stocker screener.
-- Column list MUST stay in lockstep with `crates/stocker-screener/src/metrics.rs`.
-- `ScreenerService::open()` runs `metrics::validate_schema(...)` on boot, which
-- checks every catalog column exists in `snapshots`.

CREATE TABLE IF NOT EXISTS symbols (
    symbol               TEXT PRIMARY KEY,
    short_name           TEXT,
    sector               TEXT,
    industry             TEXT,
    exchange             TEXT,
    currency             TEXT,
    country              TEXT,
    tier                 INTEGER NOT NULL DEFAULT 1, -- 0 = NIFTY 500, 1 = rest
    last_refreshed_at    INTEGER,
    last_refresh_status  TEXT,
    last_refresh_error   TEXT
);

CREATE INDEX IF NOT EXISTS idx_symbols_tier ON symbols(tier);
CREATE INDEX IF NOT EXISTS idx_symbols_last_refreshed ON symbols(last_refreshed_at);

CREATE TABLE IF NOT EXISTS snapshots (
    symbol TEXT PRIMARY KEY REFERENCES symbols(symbol) ON DELETE CASCADE,
    -- price & range
    current_price REAL, previous_close REAL,
    fifty_two_week_high REAL, fifty_two_week_low REAL,
    from_52w_high_pct REAL, up_from_52w_low_pct REAL,
    regular_market_change_percent REAL,
    volume REAL, average_volume_10_day REAL, volume_1y_avg REAL,
    return_1w_pct REAL, return_3m_pct REAL, return_6m_pct REAL,
    return_1y_pct REAL, return_3y_cagr_pct REAL, return_5y_cagr_pct REAL,
    -- market structure
    market_cap REAL, enterprise_value REAL, shares_outstanding REAL,
    face_value REAL, mcap_to_sales REAL, mcap_to_cfo REAL,
    mcap_to_quarterly_profit REAL,
    -- valuation
    pe_ratio REAL, forward_pe REAL, price_to_book REAL, price_to_sales REAL,
    price_to_fcf REAL, price_to_quarterly_earning REAL,
    ev_to_ebitda REAL, ev_to_sales REAL, peg_ratio REAL,
    earnings_yield_pct REAL, dividend_yield REAL, pb_x_pe REAL,
    graham_number REAL, intrinsic_value REAL, ncavps REAL,
    earning_power_pct REAL, eps_ttm REAL, book_value REAL,
    book_value_preceding_year REAL, book_value_3y_back REAL,
    book_value_5y_back REAL, historical_pe_3y REAL, historical_pe_5y REAL,
    historical_pe_7y REAL,
    -- income & margins
    revenue_ttm REAL, sales_last_year REAL, sales_latest_quarter REAL,
    sales_growth_ttm_pct REAL, sales_growth_3y_cagr_pct REAL,
    sales_growth_5y_cagr_pct REAL, sales_growth_7y_cagr_pct REAL,
    yoy_quarterly_sales_growth_pct REAL, qoq_sales_growth_pct REAL,
    profit_after_tax_ttm REAL, net_profit_last_year REAL,
    profit_after_tax_latest_quarter REAL,
    net_profit_preceding_year_quarter REAL,
    profit_before_tax_last_year REAL,
    profit_growth_ttm_pct REAL, profit_growth_3y_cagr_pct REAL,
    profit_growth_5y_cagr_pct REAL,
    yoy_quarterly_profit_growth_pct REAL, qoq_profit_growth_pct REAL,
    ebitda REAL, ebitda_margins REAL,
    operating_profit_preceding_year_quarter REAL,
    opm_pct REAL, npm_last_year_pct REAL, npm_preceding_year_pct REAL,
    npm_latest_quarter_pct REAL, npm_preceding_quarter_pct REAL,
    npm_preceding_year_quarter_pct REAL, gross_margins REAL,
    depreciation_ttm REAL, interest_ttm REAL,
    tax_ttm REAL, tax_last_year REAL, tax_preceding_year_quarter REAL,
    avg_ebit_5y REAL,
    -- returns / efficiency
    return_on_equity REAL, return_on_assets REAL,
    return_on_capital_employed REAL, avg_roe_3y REAL, avg_roe_5y REAL,
    -- balance sheet
    total_assets REAL, net_worth REAL, total_debt REAL, debt_to_equity REAL,
    current_ratio REAL, quick_ratio REAL, inventory REAL,
    working_capital REAL, working_capital_preceding_year REAL,
    working_capital_3y_back REAL, working_capital_5y_back REAL,
    working_capital_days REAL, avg_working_capital_days_3y REAL,
    working_capital_to_sales_pct REAL, days_receivable_outstanding REAL,
    financial_leverage REAL, interest_coverage_ratio REAL,
    -- cash flow
    operating_cashflow_ttm REAL, free_cashflow_last_year REAL,
    free_cashflow_ttm REAL, free_cashflow_3y_sum REAL,
    free_cashflow_5y_sum REAL,
    -- technical
    dma_50 REAL, dma_200 REAL, macd REAL, macd_signal REAL,
    macd_previous_day REAL, macd_signal_previous_day REAL, rsi_14 REAL,
    -- composites
    altman_z_score REAL, piotroski_f_score REAL, g_factor REAL,
    croic_pct REAL, debt_capacity REAL, mcap_to_debt_capacity REAL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_snapshots_market_cap ON snapshots(market_cap);
CREATE INDEX IF NOT EXISTS idx_snapshots_pe ON snapshots(pe_ratio);
CREATE INDEX IF NOT EXISTS idx_snapshots_roe ON snapshots(return_on_equity);
CREATE INDEX IF NOT EXISTS idx_snapshots_roce ON snapshots(return_on_capital_employed);
CREATE INDEX IF NOT EXISTS idx_snapshots_de ON snapshots(debt_to_equity);
CREATE INDEX IF NOT EXISTS idx_snapshots_pb ON snapshots(price_to_book);
CREATE INDEX IF NOT EXISTS idx_snapshots_div_yield ON snapshots(dividend_yield);
CREATE INDEX IF NOT EXISTS idx_snapshots_sales_growth_3y ON snapshots(sales_growth_3y_cagr_pct);
CREATE INDEX IF NOT EXISTS idx_snapshots_profit_growth_3y ON snapshots(profit_growth_3y_cagr_pct);

CREATE TABLE IF NOT EXISTS saved_screens (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL UNIQUE,
    filters_json TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
