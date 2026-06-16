use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::HashSet;
use std::sync::OnceLock;

static HOLIDAYS: OnceLock<HashSet<NaiveDate>> = OnceLock::new();

fn holiday_set() -> &'static HashSet<NaiveDate> {
    HOLIDAYS.get_or_init(|| {
        let raw = include_str!("../data/nse_holidays.json");
        let dates: Vec<String> = serde_json::from_str(raw).expect("valid nse_holidays.json");
        dates
            .into_iter()
            .filter_map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
            .collect()
    })
}

/// Indian market business day: Mon–Fri excluding NSE holidays.
pub fn is_trading_day(date: NaiveDate) -> bool {
    matches!(
        date.weekday(),
        Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu | Weekday::Fri
    ) && !holiday_set().contains(&date)
}

/// Whether cached NAV should be refreshed from mfapi.
pub fn should_refresh_nav(fetched_at: Option<i64>, now: i64, today: NaiveDate) -> bool {
    match fetched_at {
        None => true,
        Some(ts) => {
            let stale = now.saturating_sub(ts) > 24 * 3600;
            stale && is_trading_day(today)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn weekend_is_not_trading_day() {
        let sat = NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        assert!(!is_trading_day(sat));
    }

    #[test]
    fn weekday_is_trading_day() {
        let mon = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert!(is_trading_day(mon));
    }

    #[test]
    fn republic_day_is_holiday() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 26).unwrap();
        assert!(!is_trading_day(d));
    }

    #[test]
    fn refresh_when_missing() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert!(should_refresh_nav(None, 0, today));
    }

    #[test]
    fn refresh_when_stale_on_trading_day() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let now = 1_000_000_i64;
        let fetched = now - 25 * 3600;
        assert!(should_refresh_nav(Some(fetched), now, today));
    }

    #[test]
    fn no_refresh_when_stale_on_holiday() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 26).unwrap();
        let now = 1_000_000_i64;
        let fetched = now - 25 * 3600;
        assert!(!should_refresh_nav(Some(fetched), now, today));
    }

    #[test]
    fn no_refresh_when_fresh() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let now = 1_000_000_i64;
        let fetched = now - 3600;
        assert!(!should_refresh_nav(Some(fetched), now, today));
    }
}
