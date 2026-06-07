-- Earnings quality & working-capital trend metrics
ALTER TABLE snapshots ADD COLUMN cumulative_cfo_pat_3y REAL;
ALTER TABLE snapshots ADD COLUMN cumulative_cfo_pat_5y REAL;
ALTER TABLE snapshots ADD COLUMN cfo_pat_latest_year REAL;
ALTER TABLE snapshots ADD COLUMN days_inventory_outstanding REAL;
ALTER TABLE snapshots ADD COLUMN days_receivable_change_3y REAL;
ALTER TABLE snapshots ADD COLUMN days_inventory_change_3y REAL;
