-- Mutual fund scheme metadata and cached NAV (separate from portfolio.db / stocker.db).

CREATE TABLE IF NOT EXISTS mf_schemes (
    scheme_code         INTEGER PRIMARY KEY,
    scheme_name         TEXT NOT NULL,
    fund_house          TEXT,
    scheme_category     TEXT,
    isin_growth         TEXT,
    created_at          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mf_schemes_name ON mf_schemes(scheme_name);

CREATE TABLE IF NOT EXISTS mf_nav (
    scheme_code         INTEGER PRIMARY KEY REFERENCES mf_schemes(scheme_code),
    nav                 REAL NOT NULL,
    nav_date            TEXT NOT NULL,
    fetched_at          INTEGER NOT NULL
);
