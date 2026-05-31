//! Live Yahoo smoke test — run with `cargo test -p stocker-core --test statement_fetch -- --ignored --nocapture`

#[tokio::test]
#[ignore = "hits Yahoo Finance"]
async fn reliance_statement_bundle_shape() {
    let bundle = stocker_core::fetcher::fetch_statement_bundle("RELIANCE.NS").await;
    eprintln!(
        "income_a={} income_q={} balance_a={} balance_q={} cash_a={}",
        bundle.income_annual.len(),
        bundle.income_quarterly.len(),
        bundle.balance_annual.len(),
        bundle.balance_quarterly.len(),
        bundle.cashflow_annual.len(),
    );
    // Yahoo often returns balance-sheet period stubs without line items for NSE tickers.
    assert!(!bundle.income_annual.is_empty(), "expected annual income rows");
    assert!(!bundle.income_quarterly.is_empty(), "expected quarterly income rows");

    if let Some(row) = bundle.income_annual.first() {
        eprintln!(
            "sample annual income: revenue={} ebit={} interest={} tax={}",
            row.revenue, row.ebit, row.interest_expense, row.income_tax_expense
        );
    }
    if let Some(row) = bundle.balance_annual.first() {
        eprintln!(
            "sample annual balance: assets={} equity={} current_assets={}",
            row.total_assets, row.total_equity, row.current_assets
        );
    }
    let latest = bundle.balance_annual.last().expect("balance row");
    assert!(
        latest.total_assets > 0.0 || latest.current_assets > 0.0,
        "expected balance line items from fundamentalsTimeSeries"
    );
    let inc = bundle.income_annual.last().expect("income row");
    assert!(inc.revenue > 0.0, "expected revenue");
    assert!(
        inc.ebit > 0.0 || inc.operating_income > 0.0 || inc.ebitda > 0.0,
        "expected EBIT/operating income from fundamentalsTimeSeries"
    );
}
