//! Remote data policy for `stocker-core`.
//!
//! **Compliance:** Stocker must not call NSE, BSE, or registrar APIs or scrape exchange
//! websites. Universe symbol lists come from **local CSV files** the user provides
//! (`stocker-screener::universe_csv`). All live market and statement data is fetched
//! from **Yahoo Finance** public endpoints (same family as Python yfinance).

use reqwest::Client;

use crate::error::StockerError;

/// Substrings that must never appear in outbound market-data URLs.
const FORBIDDEN_HOST_FRAGMENTS: &[&str] = &[
    "nseindia",
    "bseindia",
    "nsdl.co.in",
    "cdslindia",
    "nsearchives",
    "api-nse",
];

/// Validate that `url` is an allowed Yahoo Finance endpoint (not NSE/BSE/registrar).
pub fn assert_yahoo_finance_url(url: &str) -> Result<(), StockerError> {
    let lower = url.to_ascii_lowercase();
    for frag in FORBIDDEN_HOST_FRAGMENTS {
        if lower.contains(frag) {
            return Err(StockerError::ForbiddenDataSource(format!(
                "blocked exchange/registrar host in URL (use local CSV + Yahoo only): {url}"
            )));
        }
    }
    if !lower.contains("yahoo.com") {
        return Err(StockerError::ForbiddenDataSource(format!(
            "only Yahoo Finance URLs are permitted for remote market data: {url}"
        )));
    }
    Ok(())
}

/// Issue a GET after [`assert_yahoo_finance_url`].
pub async fn yahoo_get(client: &Client, url: &str) -> Result<reqwest::Response, StockerError> {
    assert_yahoo_finance_url(url)?;
    Ok(client.get(url).send().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_yahoo_endpoints() {
        for url in [
            "https://query1.finance.yahoo.com/v8/finance/chart/RELIANCE.NS",
            "https://query2.finance.yahoo.com/v10/finance/quoteSummary/RELIANCE.NS",
            "https://fc.yahoo.com",
            "https://finance.yahoo.com/quote/%5ENSEI",
        ] {
            assert_yahoo_finance_url(url).unwrap_or_else(|_| panic!("should allow {url}"));
        }
    }

    #[test]
    fn rejects_nse_and_bse_hosts() {
        for url in [
            "https://www.nseindia.com/api/equity-stockIndices?index=NIFTY%20500",
            "https://nsearchives.nseindia.com/content/equities/EQUITY_L.csv",
            "https://api.bseindia.com/BseIndiaAPI/api/StockReachGraph/w",
        ] {
            assert!(assert_yahoo_finance_url(url).is_err(), "must reject {url}");
        }
    }

    #[test]
    fn rejects_non_yahoo_hosts() {
        assert!(assert_yahoo_finance_url("https://example.com/data").is_err());
    }
}
