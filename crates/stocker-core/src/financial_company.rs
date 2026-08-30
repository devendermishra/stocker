//! Classify listed companies so financials (banks/NBFCs) are not scored as industrials.

use crate::models::{
    AssetProfile, BankingMetrics, CanonicalMetrics, CoverageDimension, DataCoverage, FinancialCompanyType,
    Financials,
};

pub const NA_NOT_MEANINGFUL: &str = "N/A — not meaningful for lending companies";
pub const NA_NOT_USED: &str = "N/A — not used for lenders";
pub const NA_NOT_APPLICABLE: &str = "N/A — not applicable";
pub const NA_NOT_PRIMARY: &str = "N/A — not a primary metric";
pub const NA_YAHOO: &str = "N/A in Yahoo";
pub const NA_FILING: &str = "N/A in Yahoo — filing verification required";
pub const NA_YAHOO_FILINGS_MAY_EXIST: &str =
    "Unavailable in Yahoo — not unavailable in public filings";

pub fn ticker_base(symbol: &str) -> String {
    let u = symbol.trim().to_uppercase();
    u.strip_suffix(".NS")
        .or_else(|| u.strip_suffix(".BO"))
        .unwrap_or(&u)
        .to_string()
}

const PROJECT_FINANCE_TICKERS: &[&str] = &["RECLTD", "PFC", "IREDA", "IRFC"];
const HOUSING_FINANCE_TICKERS: &[&str] = &["LICHSGFIN", "CANFINHOME", "PNBHOUSING", "AAVAS", "HOMEFIRST"];
const BANK_TICKERS: &[&str] = &[
    "HDFCBANK",
    "ICICIBANK",
    "KOTAKBANK",
    "AXISBANK",
    "SBIN",
    "INDUSINDBK",
    "IDFCFIRSTB",
    "BANKBARODA",
    "PNB",
    "CANBK",
    "UNIONBANK",
    "BANKINDIA",
    "YESBANK",
    "FEDERALBNK",
    "AUBANK",
];

pub fn classify_financial_company(symbol: &str, profile: &AssetProfile) -> FinancialCompanyType {
    let base = ticker_base(symbol);
    let sector = profile.sector.as_deref().unwrap_or("").to_lowercase();
    let industry = profile.industry.as_deref().unwrap_or("").to_lowercase();
    let summary = profile
        .long_business_summary
        .as_deref()
        .unwrap_or("")
        .to_lowercase();
    let name = profile.long_name.as_deref().unwrap_or("").to_lowercase();
    let blob = format!("{sector} {industry} {summary} {name}");

    let looks_financial = sector.contains("financial")
        || industry.contains("bank")
        || industry.contains("nbfc")
        || industry.contains("credit")
        || industry.contains("insurance")
        || industry.contains("capital market")
        || industry.contains("asset management")
        || industry.contains("mortgage")
        || industry.contains("housing")
        || industry.contains("payment")
        || industry.contains("exchange");

    if !looks_financial {
        return FinancialCompanyType::Industrial;
    }

    if industry.contains("insurance") || sector.contains("insurance") {
        return FinancialCompanyType::Insurance;
    }
    if industry.contains("asset management") || industry.contains("investment management") {
        return FinancialCompanyType::Amc;
    }
    if industry.contains("exchange")
        || base == "MCX"
        || base == "BSE"
        || base == "CDSL"
        || base == "NSDL"
    {
        return FinancialCompanyType::Exchange;
    }
    if industry.contains("payment") || industry.contains("fintech") && summary.contains("payment") {
        return FinancialCompanyType::Payments;
    }
    if industry.contains("capital market")
        || industry.contains("broker")
        || industry.contains("wealth")
    {
        return FinancialCompanyType::Broker;
    }

    if BANK_TICKERS.contains(&base.as_str())
        || ((industry.contains("banks") || industry.contains("bank -") || industry.contains("banks -"))
            && !industry.contains("nbfc")
            && !industry.contains("credit"))
    {
        return FinancialCompanyType::Bank;
    }

    if HOUSING_FINANCE_TICKERS.contains(&base.as_str())
        || industry.contains("mortgage")
        || industry.contains("housing")
        || name.contains("housing finance")
    {
        if PROJECT_FINANCE_TICKERS.contains(&base.as_str()) {
            return FinancialCompanyType::NbfcProjectFinance;
        }
        return FinancialCompanyType::HousingFinance;
    }

    if PROJECT_FINANCE_TICKERS.contains(&base.as_str())
        || blob.contains("project finance")
        || blob.contains("rural electrification")
        || blob.contains("power finance")
        || blob.contains("infrastructure finance")
        || blob.contains("railway finance")
        || (industry.contains("credit")
            && (blob.contains("power")
                || blob.contains("electrif")
                || blob.contains("infrastructure")
                || blob.contains("renewable")))
    {
        return FinancialCompanyType::NbfcProjectFinance;
    }

    if industry.contains("credit")
        || industry.contains("nbfc")
        || name.contains("nbfc")
        || summary.contains("non-banking")
        || summary.contains("non banking")
    {
        return FinancialCompanyType::Nbfc;
    }

    if industry.contains("bank") {
        return FinancialCompanyType::Bank;
    }

    FinancialCompanyType::Nbfc
}

pub fn company_type_label(t: FinancialCompanyType) -> &'static str {
    match t {
        FinancialCompanyType::Industrial => "Industrial / operating company",
        FinancialCompanyType::Bank => "Bank",
        FinancialCompanyType::Nbfc => "NBFC",
        FinancialCompanyType::NbfcProjectFinance => "NBFC / Project Finance",
        FinancialCompanyType::HousingFinance => "Housing Finance",
        FinancialCompanyType::Insurance => "Insurance",
        FinancialCompanyType::Amc => "Asset manager",
        FinancialCompanyType::Broker => "Broker",
        FinancialCompanyType::Exchange => "Exchange",
        FinancialCompanyType::Payments => "Payments",
    }
}

impl FinancialCompanyType {
    pub fn is_financial(self) -> bool {
        self != FinancialCompanyType::Industrial
    }

    pub fn is_lender(self) -> bool {
        matches!(
            self,
            FinancialCompanyType::Bank
                | FinancialCompanyType::Nbfc
                | FinancialCompanyType::NbfcProjectFinance
                | FinancialCompanyType::HousingFinance
        )
    }

    pub fn is_bank(self) -> bool {
        self == FinancialCompanyType::Bank
    }

    pub fn is_nbfc_family(self) -> bool {
        matches!(
            self,
            FinancialCompanyType::Nbfc
                | FinancialCompanyType::NbfcProjectFinance
                | FinancialCompanyType::HousingFinance
        )
    }
}

pub fn project_finance_peer_symbols() -> Vec<&'static str> {
    vec!["PFC.NS", "IREDA.NS", "IRFC.NS", "HUDCO.NS"]
}

pub fn nbfc_peer_symbols() -> Vec<&'static str> {
    vec![
        "BAJFINANCE.NS",
        "CHOLAFIN.NS",
        "MUTHOOTFIN.NS",
        "SHRIRAMFIN.NS",
        "SUNDARMFIN.NS",
        "ABCAPITAL.NS",
        "POONAWALLA.NS",
        "MANAPPURAM.NS",
    ]
}

pub fn housing_finance_peer_symbols() -> Vec<&'static str> {
    vec![
        "LICHSGFIN.NS",
        "CANFINHOME.NS",
        "PNBHOUSING.NS",
        "HUDCO.NS",
        "AAVAS.NS",
        "HOMEFIRST.NS",
    ]
}

pub fn bank_reference_peer_symbols() -> Vec<&'static str> {
    vec![
        "HDFCBANK.NS",
        "ICICIBANK.NS",
        "SBIN.NS",
        "AXISBANK.NS",
        "KOTAKBANK.NS",
    ]
}

pub fn curated_direct_peers(symbol: &str, t: FinancialCompanyType) -> Vec<String> {
    let base = ticker_base(symbol);
    let list = match t {
        FinancialCompanyType::NbfcProjectFinance => project_finance_peer_symbols(),
        FinancialCompanyType::HousingFinance => housing_finance_peer_symbols(),
        FinancialCompanyType::Nbfc => nbfc_peer_symbols(),
        FinancialCompanyType::Bank => bank_reference_peer_symbols(),
        _ => Vec::new(),
    };
    list.into_iter()
        .filter(|s| ticker_base(s) != base)
        .map(|s| s.to_string())
        .collect()
}

pub fn peer_comparability_for(t: FinancialCompanyType) -> (String, String, String) {
    match t {
        FinancialCompanyType::NbfcProjectFinance => (
            "high".to_string(),
            "Direct peer comparability: High (PFC / IREDA / IRFC / HUDCO)".to_string(),
            "Bank comparability: Low".to_string(),
        ),
        FinancialCompanyType::Nbfc | FinancialCompanyType::HousingFinance => (
            "high".to_string(),
            "Direct peer comparability: High (NBFC / HFC names)".to_string(),
            "Bank comparability: Low".to_string(),
        ),
        FinancialCompanyType::Bank => (
            "high".to_string(),
            "Direct peer comparability: High (banks)".to_string(),
            "Bank comparability: High".to_string(),
        ),
        FinancialCompanyType::Industrial => ("medium".to_string(), String::new(), String::new()),
        _ => (
            "medium".to_string(),
            "Direct peer comparability: Medium".to_string(),
            String::new(),
        ),
    }
}

fn dim(name: &str, present: usize, total: usize) -> CoverageDimension {
    let coverage_pct = if total == 0 {
        0.0
    } else {
        (present as f64 / total as f64) * 100.0
    };
    CoverageDimension {
        name: name.to_string(),
        coverage_pct,
        present,
        total,
    }
}

fn present(v: Option<f64>) -> usize {
    usize::from(v.filter(|x| x.is_finite()).is_some())
}

fn present_f(v: f64) -> usize {
    usize::from(v.is_finite() && v.abs() > 1e-9)
}

/// Coverage of Yahoo + optional filing metrics. Critical lender buckets gate recommendations.
pub fn build_data_coverage(
    t: FinancialCompanyType,
    financials: &Financials,
    canonical: &CanonicalMetrics,
    bank: Option<&BankingMetrics>,
) -> DataCoverage {
    let b = bank.cloned().unwrap_or_default();

    let valuation = dim(
        "Valuation",
        present_f(financials.pe_ratio) + present_f(financials.price_to_book) + present(financials.return_on_equity),
        3,
    );
    let industrial_growth = dim(
        "Growth",
        present(canonical.fy_pat_yoy_pct)
            + present(canonical.pat_cagr_3y_pct)
            + present(financials.earnings_growth)
            + present(canonical.fy_revenue_yoy_pct),
        4,
    );

    if !t.is_lender() {
        let dims = vec![valuation.clone(), industrial_growth];
        let overall = dims.iter().map(|d| d.coverage_pct).sum::<f64>() / dims.len() as f64;
        return DataCoverage {
            overall_pct: overall,
            critical_pct: 100.0,
            confidence: if overall >= 70.0 { "High" } else { "Medium" }.to_string(),
            dimensions: dims,
            recommendation_gated: false,
            gate_reason: None,
            critical_present: 1,
            critical_total: 1,
        };
    }

    let asset_quality = dim(
        "Asset Quality",
        present(b.gnpa_pct) + present(b.nnpa_pct) + present(b.provision_coverage_ratio_pct),
        3,
    );
    let capital = dim(
        "Capital Adequacy",
        present(b.crar_pct) + present(b.tier1_pct),
        2,
    );
    let nim = dim(
        "NIM/Spread",
        present(b.nim_pct)
            + present(b.spread_pct)
            + present(b.yield_on_assets_pct)
            + present(b.cost_of_funds_pct)
            + present(canonical.net_interest_income),
        5,
    );
    let loan = if t.is_bank() {
        dim(
            "Loan book",
            present(canonical.canonical_advances)
                + present(b.loan_book)
                + present(b.loan_book_growth_yoy_pct)
                + present(b.credit_growth_yoy_pct)
                + present(b.disbursement_growth_yoy_pct),
            5,
        )
    } else {
        dim(
            "Loan book",
            present(canonical.loan_book)
                + present(canonical.loan_book_growth_yoy_pct)
                + present(b.loan_book)
                + present(b.loan_book_growth_yoy_pct)
                + present(b.disbursement_growth_yoy_pct),
            5,
        )
    };
    let roe_roa = dim(
        "ROE/ROA",
        present(financials.return_on_equity) + present(financials.return_on_assets),
        2,
    );

    let mut critical_present = 0usize;
    let mut critical_total = 0usize;
    for d in [&asset_quality, &capital, &roe_roa, &loan, &nim] {
        critical_total += 1;
        if d.coverage_pct > 0.0 {
            critical_present += 1;
        }
    }
    // A dimension "present" for gating means at least one metric is filled.
    // User asked coverage(critical) < 0.6 of metric groups.
    let critical_pct = if critical_total == 0 {
        0.0
    } else {
        (critical_present as f64 / critical_total as f64) * 100.0
    };

    let growth = dim(
        "Growth",
        present(canonical.fy_pat_yoy_pct)
            + present(canonical.pat_cagr_3y_pct)
            + present(canonical.interest_income_yoy_pct)
            + present(canonical.nii_yoy_pct)
            + present(canonical.loan_book_growth_yoy_pct),
        5,
    );

    let dims = vec![
        valuation,
        growth,
        asset_quality,
        capital,
        nim,
        loan,
        roe_roa,
    ];
    let overall = dims.iter().map(|d| d.coverage_pct).sum::<f64>() / dims.len() as f64;
    let gated = critical_pct < 60.0;
    let confidence = if !gated && overall >= 70.0 {
        "High"
    } else if overall >= 40.0 && critical_pct >= 40.0 {
        "Medium"
    } else {
        "Low"
    }
    .to_string();

    DataCoverage {
        overall_pct: overall,
        critical_pct,
        confidence: confidence.clone(),
        dimensions: dims,
        recommendation_gated: gated,
        gate_reason: if gated {
            Some(format!(
                "Critical lender coverage {:.0}% (need ≥60% across asset quality, capital, ROE/ROA, loan growth, NIM/spread). Yahoo lacks filing metrics — recommendation withheld.",
                critical_pct
            ))
        } else {
            None
        },
        critical_present,
        critical_total,
    }
}

pub fn coverage_dimension_pct(coverage: &DataCoverage, name: &str) -> f64 {
    coverage
        .dimensions
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name))
        .map(|d| d.coverage_pct)
        .unwrap_or(0.0)
}

/// True when GNPA/capital groups are empty — Yahoo beta must not fill this gap.
pub fn lender_fundamental_risk_unassessed(coverage: &DataCoverage, t: FinancialCompanyType) -> bool {
    if !t.is_lender() {
        return false;
    }
    coverage_dimension_pct(coverage, "Asset Quality") < 1e-9
        || coverage_dimension_pct(coverage, "Capital Adequacy") < 1e-9
}

pub fn market_beta_risk_label(beta: f64) -> String {
    if !beta.is_finite() || beta <= 0.0 {
        "N/A — Yahoo beta missing".to_string()
    } else if beta < 0.7 {
        "Low relative volatility".to_string()
    } else if beta < 1.2 {
        "Moderate relative volatility".to_string()
    } else {
        "High relative volatility".to_string()
    }
}

pub fn pb_roe_interpretation(roe: Option<f64>, pb: f64) -> String {
    let roe_pct = roe.map(|r| if r.abs() <= 1.5 { r * 100.0 } else { r });
    match (roe_pct, (pb > 0.0).then_some(pb)) {
        (Some(roe), Some(pb)) if roe > 18.0 && pb < 1.2 => {
            "Low P/B with high ROE — potentially attractive (verify asset quality)".to_string()
        }
        (Some(roe), Some(pb)) if (15.0..18.0).contains(&roe) && (1.0..=2.0).contains(&pb) => {
            "P/B vs ROE in a fair/attractive band for Indian lenders".to_string()
        }
        (Some(roe), Some(pb)) if roe < 10.0 && pb > 1.0 => {
            "P/B above 1x with weak ROE — expensive vs profitability".to_string()
        }
        (Some(roe), Some(pb)) if roe < 10.0 && pb < 0.7 => {
            "Low P/B with weak ROE — possible value trap if asset quality is poor".to_string()
        }
        (Some(roe), Some(pb)) if roe >= 15.0 && pb > 2.0 => {
            "High P/B with solid ROE — premium may be justified if growth/quality hold".to_string()
        }
        (Some(roe), Some(pb)) => format!("P/B {:.2}x vs ROE {:.1}% — interpret with asset quality and capital", pb, roe),
        (None, Some(pb)) => format!("P/B {:.2}x; ROE missing — incomplete P/B–ROE read", pb),
        _ => "P/B–ROE matrix unavailable".to_string(),
    }
}

pub fn lender_sector_outlook(
    t: FinancialCompanyType,
    sector: Option<&str>,
    industry: Option<&str>,
) -> String {
    let header = match (sector, industry) {
        (Some(s), Some(i)) if !s.is_empty() && !i.is_empty() => {
            format!("Sector: {}. Industry: {}. ", s, i)
        }
        (Some(s), _) if !s.is_empty() => format!("Sector: {}. ", s),
        _ => String::new(),
    };
    let body = if t == FinancialCompanyType::NbfcProjectFinance {
        "For project-finance NBFCs, the useful cycle is credit growth, power and infrastructure capex, government capex programmes, bond yields and the RBI rate path (cost of funds), power-sector asset quality and state-utility health, renewables financing demand, and competition from banks and the bond market. Porter scores from Yahoo margins are not used for this archetype."
    } else {
        "For lenders, watch credit growth, NIM/spread, cost of funds, asset quality, capital, and the rate cycle — not industrial Porter analysis from Yahoo margins."
    };
    format!("{header}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rec_is_nbfc_project_finance() {
        let profile = AssetProfile {
            sector: Some("Financial Services".into()),
            industry: Some("Credit Services".into()),
            long_name: Some("REC Limited".into()),
            long_business_summary: Some(
                "REC Limited finances and promotes rural electrification projects across India.".into(),
            ),
            ..Default::default()
        };
        assert_eq!(
            classify_financial_company("RECLTD.NS", &profile),
            FinancialCompanyType::NbfcProjectFinance
        );
    }

    #[test]
    fn sbin_is_bank() {
        let profile = AssetProfile {
            sector: Some("Financial Services".into()),
            industry: Some("Banks - Regional".into()),
            ..Default::default()
        };
        assert_eq!(
            classify_financial_company("SBIN.NS", &profile),
            FinancialCompanyType::Bank
        );
    }

    #[test]
    fn project_finance_sector_outlook_is_not_porter() {
        let s = lender_sector_outlook(
            FinancialCompanyType::NbfcProjectFinance,
            Some("Financial Services"),
            Some("Credit Services"),
        );
        assert!(s.contains("credit growth"));
        assert!(s.contains("discom") || s.contains("state-utility") || s.contains("state utility"));
        assert!(s.contains("Porter"));
    }

    #[test]
    fn reliance_is_industrial() {
        let profile = AssetProfile {
            sector: Some("Energy".into()),
            industry: Some("Oil & Gas Refining & Marketing".into()),
            ..Default::default()
        };
        assert_eq!(
            classify_financial_company("RELIANCE.NS", &profile),
            FinancialCompanyType::Industrial
        );
    }

    #[test]
    fn bank_yahoo_loans_do_not_fill_loan_or_credit_risk_coverage() {
        let canonical = CanonicalMetrics {
            yahoo_loan_book_field: Some(30.99e12),
            yahoo_loan_book_row: "Net Loan".into(),
            yahoo_loan_book_growth_yoy_pct: Some(10.3),
            net_interest_income: Some(1.0),
            ..Default::default()
        };
        let fin = Financials {
            return_on_equity: Some(0.17),
            return_on_assets: Some(0.02),
            pe_ratio: 20.0,
            price_to_book: 2.8,
            ..Default::default()
        };
        let cov = build_data_coverage(FinancialCompanyType::Bank, &fin, &canonical, None);
        let loan = cov.dimensions.iter().find(|d| d.name == "Loan book").unwrap();
        assert_eq!(loan.present, 0);
        assert!(lender_fundamental_risk_unassessed(&cov, FinancialCompanyType::Bank));
        assert!(cov.recommendation_gated);
        assert_eq!(market_beta_risk_label(0.414), "Low relative volatility");
    }
}
