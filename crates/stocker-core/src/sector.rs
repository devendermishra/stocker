//! Sector helpers — narrative glue; structured analysis lives in [`crate::analysis`].

use crate::fetcher::fetch_quote_summary;

/// Legacy one-line outlook (kept for CLI compatibility if needed).
pub async fn fetch_sector_outlook(symbol: &str) -> String {
    match fetch_quote_summary(symbol, "assetProfile").await {
        Ok(v) => {
            let sector = v["quoteSummary"]["result"][0]["assetProfile"]["sector"]
                .as_str()
                .unwrap_or("Unknown");
            if sector == "Unknown" {
                "No sector data found".to_string()
            } else {
                format!("Sector: {}. General outlook for this sector is Neutral/Positive.", sector)
            }
        }
        Err(e) => {
            log::error!("Error fetching sector: {}", e);
            "Sector data unavailable due to error".to_string()
        }
    }
}
