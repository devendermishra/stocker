//! Normalize user input to Yahoo NSE tickers (`*.NS`).

/// Returns uppercase Yahoo symbol ending in `.NS`.
/// Accepts `RELIANCE`, `reliance.ns`, `RELIANCE.NS`.
pub fn normalize_nse_symbol(input: &str) -> Result<String, crate::StockerError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    let upper = s.to_uppercase();
    let base = upper.strip_suffix(".NS").unwrap_or(&upper).trim();
    if base.is_empty() {
        return Err(crate::StockerError::InvalidSymbol(input.to_string()));
    }
    // Yahoo NSE: simple Latin ticker + .NS
    let normalized = format!("{}.NS", base);
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_ns_suffix() {
        assert_eq!(normalize_nse_symbol("RELIANCE").unwrap(), "RELIANCE.NS");
    }

    #[test]
    fn preserves_existing_ns() {
        assert_eq!(normalize_nse_symbol("TCS.NS").unwrap(), "TCS.NS");
    }
}
