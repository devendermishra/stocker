//! Mutual-fund purchase rules. NAV fetching and `mf.db` stay in `stocker-mf`.

pub mod stamp_duty;
pub mod symbol;
pub mod trading_day;

pub use stamp_duty::{
    allot_mf_purchase, round_paise, MfPurchaseAllotment, MF_PURCHASE_STAMP_DUTY_RATE,
};
pub use symbol::{is_mutual_fund_symbol, mf_symbol, parse_mf_symbol};
pub use trading_day::{is_trading_day, should_refresh_nav};
