//! Global counter bumped when portfolio transactions change so all portfolio views refetch.

use dioxus::prelude::*;

static PORTFOLIO_DATA_REVISION: GlobalSignal<u64> = Signal::global(|| 0);

pub fn portfolio_data_revision() -> u64 {
    PORTFOLIO_DATA_REVISION()
}

pub fn bump_portfolio_data_revision() {
    let mut rev = PORTFOLIO_DATA_REVISION.write();
    *rev = rev.saturating_add(1);
}
