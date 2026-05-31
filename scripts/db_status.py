import sqlite3
from pathlib import Path

db = Path(__file__).resolve().parent.parent / "stocker.db"
c = sqlite3.connect(db)


def one(sql):
    return c.execute(sql).fetchone()[0]


print("total_symbols", one("SELECT COUNT(*) FROM symbols"))
print("with_snapshots", one("SELECT COUNT(*) FROM snapshots"))
print("never_refreshed", one("SELECT COUNT(*) FROM symbols WHERE last_refreshed_at IS NULL"))
print("refresh_ok", one("SELECT COUNT(*) FROM symbols WHERE last_refresh_status = 'ok'"))
print("refresh_error", one("SELECT COUNT(*) FROM symbols WHERE last_refresh_status = 'error'"))
print("tier0", one("SELECT COUNT(*) FROM symbols WHERE tier = 0"))
print("tier1", one("SELECT COUNT(*) FROM symbols WHERE tier = 1"))

n = one("SELECT COUNT(*) FROM snapshots")
if n == 0:
    print("coverage: no snapshots")
    c.close()
    raise SystemExit(0)

cols = [
    r[1]
    for r in c.execute("PRAGMA table_info(snapshots)").fetchall()
    if r[1] not in ("symbol", "updated_at")
]
full = partial = empty = 0
rows = []
for col in cols:
    filled = one(f"SELECT COUNT(*) FROM snapshots WHERE {col} IS NOT NULL")
    pct = 100.0 * filled / n
    if filled == 0:
        tier = "empty"
        empty += 1
    elif filled >= n:
        tier = "full"
        full += 1
    else:
        tier = "partial"
        partial += 1
    rows.append((pct, filled, tier, col))

rows.sort()
print(f"coverage_summary full={full} partial={partial} empty={empty} snapshots={n}")
print("coverage_empty:")
for _, _, tier, col in rows:
    if tier == "empty":
        print(f"  {col}")
print("coverage_partial_bottom10:")
for pct, filled, tier, col in rows[:10]:
    if tier == "partial":
        print(f"  {col}: {filled}/{n} ({pct:.1f}%)")

c.close()

# Repopulate after compute/fetch fixes:
#   cargo run -p stocker-cli -- reset-jobs
#   (start stocker-api or desktop app so the scheduler refreshes symbols)
