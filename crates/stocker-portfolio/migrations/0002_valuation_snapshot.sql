-- Enriched portfolio valuation cache (holdings + prices).

ALTER TABLE portfolio_snapshots ADD COLUMN holdings_json TEXT;
ALTER TABLE portfolio_snapshots ADD COLUMN valuation_summary_json TEXT;
ALTER TABLE portfolio_snapshots ADD COLUMN priced_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE portfolio_snapshots ADD COLUMN symbol_prices_json TEXT;
