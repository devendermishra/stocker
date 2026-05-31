-- Face value per share from NSE EQUITY_L.csv (stored on symbols, copied to snapshots on refresh).
ALTER TABLE symbols ADD COLUMN face_value REAL;
