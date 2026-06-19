-- MF SIP/SWP schedule registry and installment linkage.

CREATE TABLE IF NOT EXISTS mf_schedules (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id             INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    portfolio_id        INTEGER NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    schedule_type       TEXT NOT NULL,
    symbol              TEXT NOT NULL,
    amount              REAL NOT NULL,
    start_date          TEXT NOT NULL,
    end_date            TEXT,
    installment_count   INTEGER,
    sip_day             INTEGER NOT NULL,
    status              TEXT NOT NULL DEFAULT 'active',
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mf_schedules_portfolio ON mf_schedules(portfolio_id);
CREATE INDEX IF NOT EXISTS idx_mf_schedules_status ON mf_schedules(portfolio_id, status);

ALTER TABLE transactions ADD COLUMN schedule_id INTEGER REFERENCES mf_schedules(id);
CREATE INDEX IF NOT EXISTS idx_transactions_schedule ON transactions(schedule_id);
