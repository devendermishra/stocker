use chrono::{Datelike, NaiveDate, Weekday};
use std::collections::HashSet;
use std::sync::OnceLock;

static HOLIDAYS: OnceLock<HashSet<NaiveDate>> = OnceLock::new();

fn parse_holiday_dates(raw: &str) -> Result<HashSet<NaiveDate>, String> {
    let dates: Vec<String> = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let mut out = HashSet::with_capacity(dates.len());
    for date in dates {
        let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| format!("nse_holidays.json has unparseable date {date}"))?;
        out.insert(parsed);
    }
    Ok(out)
}

fn holiday_set() -> &'static HashSet<NaiveDate> {
    HOLIDAYS.get_or_init(|| {
        parse_holiday_dates(include_str!("../../data/nse_holidays.json"))
            .expect("crates/stocker-domain/data/nse_holidays.json")
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
    fn holiday_file_parses_every_date() {
        let dates = parse_holiday_dates(include_str!("../../data/nse_holidays.json")).unwrap();
        assert!(dates.contains(&NaiveDate::from_ymd_opt(2026, 1, 26).unwrap()));
    }

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
