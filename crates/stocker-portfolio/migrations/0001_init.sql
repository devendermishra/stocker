-- Portfolio module schema. Separate from stocker.db (research/screener).

CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    display_name    TEXT,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL UNIQUE,
    expires_at      INTEGER NOT NULL,
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS portfolios (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,
    base_currency   TEXT NOT NULL DEFAULT 'INR',
    portfolio_type  TEXT NOT NULL DEFAULT 'mixed',
    status          TEXT NOT NULL DEFAULT 'active',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_portfolios_user ON portfolios(user_id);
CREATE INDEX IF NOT EXISTS idx_portfolios_status ON portfolios(status);

CREATE TABLE IF NOT EXISTS labels (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    color           TEXT,
    created_at      INTEGER NOT NULL,
    UNIQUE(user_id, name)
);

CREATE TABLE IF NOT EXISTS label_links (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    label_id        INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    entity_type     TEXT NOT NULL,
    entity_id       TEXT NOT NULL,
    UNIQUE(label_id, entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_label_links_entity ON label_links(entity_type, entity_id);

CREATE TABLE IF NOT EXISTS transactions (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id             INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    portfolio_id        INTEGER NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    txn_type            TEXT NOT NULL,
    trade_date          TEXT NOT NULL,
    symbol              TEXT,
    quantity            REAL,
    price               REAL,
    gross_amount        REAL,
    brokerage           REAL,
    taxes               REAL,
    net_amount          REAL,
    split_ratio_num     REAL,
    split_ratio_den     REAL,
    bonus_ratio_num     REAL,
    bonus_ratio_den     REAL,
    dividend_per_share  REAL,
    tds                 REAL,
    eligible_quantity   REAL,
    notes               TEXT,
    source              TEXT NOT NULL DEFAULT 'manual',
    corporate_action_key TEXT,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transactions_portfolio ON transactions(portfolio_id, trade_date, id);
CREATE INDEX IF NOT EXISTS idx_transactions_user ON transactions(user_id);
CREATE INDEX IF NOT EXISTS idx_transactions_symbol ON transactions(portfolio_id, symbol);
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_corp_action
    ON transactions(portfolio_id, corporate_action_key)
    WHERE corporate_action_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS fifo_lots (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    portfolio_id            INTEGER NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    symbol                  TEXT NOT NULL,
    source_transaction_id   INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    acquired_date           TEXT NOT NULL,
    original_quantity       REAL NOT NULL,
    remaining_quantity      REAL NOT NULL,
    total_cost              REAL NOT NULL,
    cost_per_share          REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fifo_lots_portfolio ON fifo_lots(portfolio_id, symbol);

CREATE TABLE IF NOT EXISTS realized_matches (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    portfolio_id            INTEGER NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    sell_transaction_id     INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    buy_transaction_id      INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    symbol                  TEXT NOT NULL,
    quantity                REAL NOT NULL,
    buy_date                TEXT NOT NULL,
    sell_date               TEXT NOT NULL,
    buy_cost_per_share      REAL NOT NULL,
    sell_price              REAL NOT NULL,
    cost_basis              REAL NOT NULL,
    sell_value              REAL NOT NULL,
    realized_gain           REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_realized_portfolio ON realized_matches(portfolio_id);

CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    portfolio_id    INTEGER PRIMARY KEY REFERENCES portfolios(id) ON DELETE CASCADE,
    summary_json    TEXT NOT NULL,
    rebuilt_at      INTEGER NOT NULL
);
