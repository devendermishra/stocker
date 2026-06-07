ALTER TABLE symbols ADD COLUMN isin TEXT;
CREATE INDEX IF NOT EXISTS idx_symbols_isin ON symbols(isin);
CREATE INDEX IF NOT EXISTS idx_symbols_short_name ON symbols(short_name);
CREATE INDEX IF NOT EXISTS idx_symbols_exchange ON symbols(exchange);
