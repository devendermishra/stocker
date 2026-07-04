-- Berkshire Lens composite metrics for screener filtering.
ALTER TABLE snapshots ADD COLUMN owner_earnings_ttm REAL;
ALTER TABLE snapshots ADD COLUMN owner_earnings_yield_pct REAL;
ALTER TABLE snapshots ADD COLUMN moat_score REAL;
ALTER TABLE snapshots ADD COLUMN earnings_durability_score REAL;
ALTER TABLE snapshots ADD COLUMN capital_intensity_score REAL;
ALTER TABLE snapshots ADD COLUMN management_trust_score REAL;
ALTER TABLE snapshots ADD COLUMN margin_of_safety_pct REAL;
ALTER TABLE snapshots ADD COLUMN business_tier REAL;
