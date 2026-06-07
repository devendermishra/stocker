//! Shared financial math helpers used by analysis and screener metric compute.

/// Percent change from `prev` to `cur`, as a percentage (e.g. 10.0 = +10%).
pub fn pct_change(cur: f64, prev: f64) -> Option<f64> {
    if !prev.is_finite() || prev.abs() < 1e-9 {
        return None;
    }
    let out = ((cur / prev) - 1.0) * 100.0;
    if out.is_finite() {
        Some(out)
    } else {
        None
    }
}

/// Compound annual growth rate over `years` between `start` (older) and `end` (newer).
/// Returns a percentage (e.g. 12.5 = 12.5% CAGR).
pub fn cagr(start: f64, end: f64, years: f64) -> Option<f64> {
    if !start.is_finite()
        || !end.is_finite()
        || !years.is_finite()
        || years <= 0.0
        || start <= 0.0
        || end <= 0.0
    {
        return None;
    }
    let out = ((end / start).powf(1.0 / years) - 1.0) * 100.0;
    if out.is_finite() {
        Some(out)
    } else {
        None
    }
}

/// Median of finite values. Non-finite inputs are dropped.
pub fn median(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut values: Vec<f64> = values.into_iter().filter(|v| v.is_finite()).collect();
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    Some(if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cagr_basics() {
        let v = cagr(100.0, 200.0, 5.0).unwrap();
        assert!((v - 14.87).abs() < 0.5);
    }

    #[test]
    fn median_basics() {
        assert_eq!(median([1.0, 3.0, 2.0]), Some(2.0));
        assert_eq!(median([1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median(std::iter::empty::<f64>()), None);
        assert_eq!(median([f64::NAN, f64::INFINITY]), None);
    }

    #[test]
    fn pct_change_basics() {
        let v = pct_change(110.0, 100.0).unwrap();
        assert!((v - 10.0).abs() < 1e-9);
        assert_eq!(pct_change(100.0, 0.0), None);
    }
}
