use reqwest::Client;
use serde_json::Value;
use std::io;
use std::sync::{Mutex, OnceLock};

use crate::models::{
    AnnualReport, AssetProfile, Financials, NewsItem, PeerQuote, Shareholders,
};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
static YAHOO_CLIENT: OnceLock<Client> = OnceLock::new();
static YAHOO_CRUMB: Mutex<Option<String>> = Mutex::new(None);

fn http_client() -> &'static Client {
    YAHOO_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(UA)
            .cookie_store(true)
            .build()
            .expect("reqwest client")
    })
}

pub async fn fetch_quote_summary(symbol: &str, modules: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut url = format!(
        "https://query2.finance.yahoo.com/v10/finance/quoteSummary/{}?modules={}",
        urlencoding::encode(symbol),
        modules
    );
    let client = http_client();
    if let Some(crumb) = ensure_yahoo_crumb(client).await {
        url.push_str("&crumb=");
        url.push_str(&urlencoding::encode(&crumb));
    }
    let res = client.get(&url).send().await?;
    let text = res.text().await?;
    let json: Value = serde_json::from_str(&text)?;
    let err_code = json["quoteSummary"]["error"]["code"].as_str();
    let err_desc = json["quoteSummary"]["error"]["description"].as_str();
    if err_code.is_some() || err_desc.is_some() {
        let code = err_code.unwrap_or("UnknownCode");
        let desc = err_desc.unwrap_or("Unknown quote summary error");
        return Err(Box::new(io::Error::other(format!(
            "Yahoo quoteSummary error [{}]: {}",
            code, desc
        ))));
    }
    if json["quoteSummary"]["result"].is_null() {
        return Err(Box::new(io::Error::other(
            "Yahoo quoteSummary returned empty result",
        )));
    }
    Ok(json)
}

async fn ensure_yahoo_crumb(client: &Client) -> Option<String> {
    if let Ok(guard) = YAHOO_CRUMB.lock() {
        if let Some(crumb) = guard.clone() {
            return Some(crumb);
        }
    }

    // Warm up Yahoo cookie jar before requesting crumb.
    let _ = client
        .get("https://fc.yahoo.com")
        .send()
        .await;
    let _ = client
        .get("https://finance.yahoo.com/quote/%5ENSEI")
        .send()
        .await;
    let response = client
        .get("https://query1.finance.yahoo.com/v1/test/getcrumb")
        .send()
        .await
        .ok()?;
    let text = response.text().await.ok()?;
    let crumb = text.trim().to_string();
    if crumb.is_empty() || crumb.starts_with('{') || crumb.contains('<') {
        return None;
    }

    if let Ok(mut guard) = YAHOO_CRUMB.lock() {
        *guard = Some(crumb.clone());
    }
    Some(crumb)
}

pub async fn fetch_price(symbol: &str) -> f64 {
    match fetch_quote_summary(symbol, "price").await {
        Ok(v) => v["quoteSummary"]["result"][0]["price"]["regularMarketPrice"]["raw"]
            .as_f64()
            .unwrap_or(0.0),
        Err(e) => {
            log::error!("Error fetching price: {}", e);
            0.0
        }
    }
}

fn yahoo_raw_f64(v: &Value) -> f64 {
    v.get("raw").and_then(|x| x.as_f64()).unwrap_or(0.0)
}

pub async fn fetch_financials(symbol: &str) -> Financials {
    match fetch_quote_summary(symbol, "financialData,defaultKeyStatistics,summaryDetail,price").await {
        Ok(v) => {
            let result = &v["quoteSummary"]["result"][0];
            let financial_data = &result["financialData"];
            let summary_detail = &result["summaryDetail"];
            let key_stats = &result["defaultKeyStatistics"];
            let price_mod = &result["price"];

            let trailing_pe = yahoo_raw_f64(&summary_detail["trailingPE"]).max(0.0);
            let forward_pe = yahoo_raw_f64(&summary_detail["forwardPE"]).max(0.0);
            let pe_display = if trailing_pe > 0.0 {
                trailing_pe
            } else {
                forward_pe
            };

            let ex_div = summary_detail["exDividendDate"]
                .get("fmt")
                .and_then(|f| f.as_str())
                .map(String::from);

            let book_value = yahoo_raw_f64(&key_stats["bookValue"]);
            let price_to_book = yahoo_raw_f64(&key_stats["priceToBook"]).max(0.0);
            let trailing_eps = yahoo_raw_f64(&key_stats["trailingEps"]);
            let forward_eps = yahoo_raw_f64(&key_stats["forwardEps"]);

            let div_yield = yahoo_raw_f64(&summary_detail["dividendYield"]);
            let payout = yahoo_raw_f64(&summary_detail["payoutRatio"]);

            let rev_growth = yahoo_raw_f64(&financial_data["revenueGrowth"]);
            let earn_growth = yahoo_raw_f64(&financial_data["earningsGrowth"]);

            let reg_change_pct = yahoo_raw_f64(&price_mod["regularMarketChangePercent"]);
            let prev_close = yahoo_raw_f64(&summary_detail["previousClose"])
                .max(yahoo_raw_f64(&price_mod["regularMarketPreviousClose"]));

            let wk_hi = yahoo_raw_f64(&price_mod["fiftyTwoWeekHigh"]);
            let wk_lo = yahoo_raw_f64(&price_mod["fiftyTwoWeekLow"]);

            let beta_ks = yahoo_raw_f64(&key_stats["beta"]);
            let beta_sd = yahoo_raw_f64(&summary_detail["beta"]);
            let beta = if beta_ks > 0.0 { beta_ks } else { beta_sd };

            Financials {
                revenue: yahoo_raw_f64(&financial_data["totalRevenue"]),
                net_income: yahoo_raw_f64(&financial_data["netIncomeToCommon"]),
                pe_ratio: pe_display,
                forward_pe,
                total_debt: yahoo_raw_f64(&financial_data["totalDebt"]),
                ebitda: yahoo_raw_f64(&financial_data["ebitda"]),
                profit_margins: yahoo_raw_f64(&financial_data["profitMargins"]),
                return_on_equity: yahoo_raw_f64(&financial_data["returnOnEquity"]),
                debt_to_equity: yahoo_raw_f64(&financial_data["debtToEquity"]),
                free_cashflow: yahoo_raw_f64(&financial_data["freeCashflow"]),
                operating_cashflow: yahoo_raw_f64(&financial_data["operatingCashflow"]),
                shares_outstanding: yahoo_raw_f64(&key_stats["sharesOutstanding"]),
                market_cap: yahoo_raw_f64(&price_mod["marketCap"]),
                book_value,
                price_to_book,
                trailing_eps,
                forward_eps,
                dividend_yield: div_yield,
                payout_ratio: payout,
                revenue_growth: rev_growth,
                earnings_growth: earn_growth,
                regular_market_change_percent: reg_change_pct,
                previous_close: prev_close,
                fifty_two_week_high: wk_hi,
                fifty_two_week_low: wk_lo,
                beta: beta.max(0.0),
                ex_dividend_date: ex_div,
            }
        }
        Err(e) => {
            log::error!("Error fetching financials: {}", e);
            Financials::default()
        }
    }
}

pub async fn fetch_shareholders(symbol: &str) -> Shareholders {
    match fetch_quote_summary(symbol, "majorHoldersBreakdown,netSharePurchaseActivity").await {
        Ok(v) => {
            let breakdown = &v["quoteSummary"]["result"][0]["majorHoldersBreakdown"];
            let insider_activity = &v["quoteSummary"]["result"][0]["netSharePurchaseActivity"];
            let net_shares = yahoo_raw_f64(&insider_activity["netInfoShares"]);
            let activity_note = if net_shares > 0.0 {
                Some("Net insider buying reported in recent period.".to_string())
            } else if net_shares < 0.0 {
                Some("Net insider selling reported in recent period.".to_string())
            } else {
                None
            };
            Shareholders {
                insiders_percent: breakdown["insidersPercentHeld"]["raw"].as_f64().unwrap_or(0.0),
                institutions_percent: breakdown["institutionsPercentHeld"]["raw"]
                    .as_f64()
                    .unwrap_or(0.0),
                promoter_percent: breakdown["insidersPercentHeld"]["raw"].as_f64(),
                fii_percent: breakdown["institutionsFloatPercentHeld"]["raw"].as_f64(),
                dii_percent: None,
                mutual_fund_percent: breakdown["institutionsPercentHeld"]["raw"].as_f64(),
                retail_percent: breakdown["heldPercentInsiders"]["raw"].as_f64().map(|v| (1.0 - v).max(0.0)),
                pledge_percent: None,
                insider_activity_note: activity_note,
            }
        }
        Err(e) => {
            log::error!("Error fetching shareholders: {}", e);
            Shareholders::default()
        }
    }
}

pub async fn fetch_annual_reports(symbol: &str) -> Vec<AnnualReport> {
    match fetch_quote_summary(symbol, "incomeStatementHistory").await {
        Ok(v) => {
            let mut reports = Vec::new();
            if let Some(history) = v["quoteSummary"]["result"][0]["incomeStatementHistory"]["incomeStatementHistory"]
                .as_array()
            {
                for item in history {
                    reports.push(AnnualReport {
                        date: item["endDate"]["fmt"].as_str().unwrap_or("Unknown").to_string(),
                        revenue: item["totalRevenue"]["raw"].as_f64().unwrap_or(0.0),
                        net_income: item["netIncome"]["raw"].as_f64().unwrap_or(0.0),
                    });
                }
            }
            reports
        }
        Err(e) => {
            log::error!("Error fetching annual reports: {}", e);
            Vec::new()
        }
    }
}

pub async fn fetch_officer_pay(symbol: &str) -> f64 {
    match fetch_quote_summary(symbol, "assetProfile").await {
        Ok(v) => {
            let mut total_pay = 0.0;
            if let Some(officers) = v["quoteSummary"]["result"][0]["assetProfile"]["companyOfficers"].as_array() {
                for officer in officers {
                    total_pay += officer["totalPay"]["raw"].as_f64().unwrap_or(0.0);
                }
            }
            total_pay
        }
        Err(e) => {
            log::error!("Error fetching officer pay: {}", e);
            0.0
        }
    }
}

pub async fn fetch_asset_profile(symbol: &str) -> AssetProfile {
    match fetch_quote_summary(symbol, "assetProfile,price").await {
        Ok(v) => {
            let result = &v["quoteSummary"]["result"][0];
            let ap = &result["assetProfile"];
            let price = &result["price"];
            AssetProfile {
                long_name: price["longName"]
                    .as_str()
                    .or_else(|| price["shortName"].as_str())
                    .or_else(|| ap["longName"].as_str())
                    .map(String::from),
                sector: ap["sector"].as_str().map(String::from),
                industry: ap["industry"].as_str().map(String::from),
                long_business_summary: ap["longBusinessSummary"].as_str().map(String::from),
            }
        }
        Err(e) => {
            log::error!("Error fetching asset profile: {}", e);
            AssetProfile::default()
        }
    }
}

pub async fn fetch_news(symbol: &str) -> Vec<NewsItem> {
    fetch_news_for_query(symbol, 8).await
}

pub async fn fetch_company_news(
    symbol: &str,
    long_name: Option<&str>,
    sector: Option<&str>,
    industry: Option<&str>,
    max_items: usize,
) -> Vec<NewsItem> {
    let base_symbol = symbol.trim_end_matches(".NS").to_uppercase();
    let mut queries = vec![
        format!("{} NSE India stock earnings", base_symbol),
        format!("{} NSE India quarterly results", base_symbol),
    ];
    if let Some(name) = long_name.filter(|s| !s.trim().is_empty()) {
        queries.push(format!("{} India stock", name.trim()));
        queries.push(format!("{} earnings guidance India", name.trim()));
    }
    if let Some(ind) = industry.filter(|s| !s.trim().is_empty()) {
        queries.push(format!("{} India stock sector news", ind.trim()));
    }
    if let Some(sec) = sector.filter(|s| !s.trim().is_empty()) {
        queries.push(format!("{} India NSE sector news", sec.trim()));
    }

    let mut merged = Vec::new();
    for q in queries {
        merged.extend(fetch_news_for_query(&q, 12).await);
    }

    let mut seen_links = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for item in merged {
        if !item.link.is_empty() && !seen_links.insert(item.link.clone()) {
            continue;
        }
        deduped.push(item);
    }

    let mut tokens = vec![base_symbol.clone()];
    if let Some(name) = long_name {
        for w in name.split_whitespace() {
            let token = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if token.len() >= 4 {
                tokens.push(token.to_uppercase());
            }
        }
    }
    if let Some(ind) = industry {
        for w in ind.split_whitespace() {
            let token = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if token.len() >= 4 {
                tokens.push(token.to_uppercase());
            }
        }
    }
    if let Some(sec) = sector {
        for w in sec.split_whitespace() {
            let token = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if token.len() >= 4 {
                tokens.push(token.to_uppercase());
            }
        }
    }

    deduped.sort_by(|a, b| {
        let sa = news_relevance_score(&a.title, &tokens);
        let sb = news_relevance_score(&b.title, &tokens);
        sb.cmp(&sa)
            .then_with(|| b.published_at.cmp(&a.published_at))
    });
    let mut filtered: Vec<NewsItem> = deduped
        .into_iter()
        .filter(|n| news_relevance_score(&n.title, &tokens) > 0)
        .take(max_items)
        .collect();
    if filtered.len() < max_items.saturating_div(2) {
        let mut fallback_queries = Vec::new();
        if let Some(name) = long_name.filter(|s| !s.trim().is_empty()) {
            fallback_queries.push(format!("{} NSE India", name.trim()));
        }
        if let Some(ind) = industry.filter(|s| !s.trim().is_empty()) {
            fallback_queries.push(format!("{} India listed companies", ind.trim()));
        }
        if let Some(sec) = sector.filter(|s| !s.trim().is_empty()) {
            fallback_queries.push(format!("{} India market news", sec.trim()));
        }
        fallback_queries.push(format!("{} NSE India", base_symbol));

        let mut fallback_items = Vec::new();
        let mut seen_links = std::collections::HashSet::new();
        for query in fallback_queries {
            for item in fetch_news_for_query(&query, max_items as u32).await {
                if !item.link.is_empty() && !seen_links.insert(item.link.clone()) {
                    continue;
                }
                fallback_items.push(item);
                if fallback_items.len() >= max_items {
                    break;
                }
            }
            if fallback_items.len() >= max_items {
                break;
            }
        }
        filtered = fallback_items;
    }
    filtered
}

fn news_relevance_score(title: &str, tokens: &[String]) -> i32 {
    let lower = title.to_lowercase();
    let mut score = 0;
    for t in tokens {
        let t = t.to_lowercase();
        if !t.is_empty() && lower.contains(&t) {
            score += 2;
        }
    }
    if lower.contains("earnings") || lower.contains("results") || lower.contains("guidance") {
        score += 1;
    }
    score
}

pub async fn fetch_sector_news(sector_or_topic: &str) -> Vec<NewsItem> {
    let topic = sector_or_topic.trim();
    let queries = [
        format!("{} India sector news", topic),
        format!("{} India listed companies", topic),
        format!("{} NSE BSE market updates", topic),
    ];
    let mut merged = Vec::new();
    let mut seen_links = std::collections::HashSet::new();
    for q in queries {
        for item in fetch_news_for_query(&q, 10).await {
            if !item.link.is_empty() && !seen_links.insert(item.link.clone()) {
                continue;
            }
            merged.push(item);
        }
    }
    merged
}

async fn fetch_news_for_query(q: &str, news_count: u32) -> Vec<NewsItem> {
    let url = format!(
        "https://query2.finance.yahoo.com/v1/finance/search?q={}&newsCount={}",
        urlencoding::encode(q),
        news_count
    );
    let client = http_client();
    let mut news_list = Vec::new();
    match client.get(&url).send().await {
        Ok(res) => {
            if let Ok(text) = res.text().await {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Some(news_items) = v["news"].as_array() {
                        for item in news_items {
                            news_list.push(NewsItem {
                                title: item["title"].as_str().unwrap_or("").to_string(),
                                link: item["link"].as_str().unwrap_or("").to_string(),
                                published_at: item["providerPublishTime"]
                                    .as_i64()
                                    .map(|t| t.to_string())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        }
        Err(e) => log::error!("Error fetching news: {}", e),
    }
    news_list
}

/// Discover NSE peer symbols via Yahoo search using industry/sector terms.
pub async fn discover_nse_peer_symbols(subject: &str, industry: Option<&str>, sector: Option<&str>, limit: usize) -> Vec<String> {
    let mut query = String::new();
    if let Some(ind) = industry {
        if !ind.is_empty() && ind != "Unknown" {
            query.push_str(ind);
        }
    } else if let Some(sec) = sector {
        if !sec.is_empty() {
            query.push_str(sec);
        }
    }
    if query.is_empty() {
        return fallback_peer_symbols(subject, industry, sector, limit);
    }
    query.push_str(" NSE India");

    let url = format!(
        "https://query2.finance.yahoo.com/v1/finance/search?q={}&quotesCount=25&newsCount=0",
        urlencoding::encode(&query)
    );
    let client = http_client();
    let mut out = Vec::new();
    let Ok(res) = client.get(&url).send().await else {
        return out;
    };
    let Ok(text) = res.text().await else {
        return out;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return out;
    };
    let Some(quotes) = v["quotes"].as_array() else {
        return out;
    };
    for q in quotes {
        let Some(sym) = q["symbol"].as_str() else { continue };
        if !sym.ends_with(".NS") {
            continue;
        }
        if sym.eq_ignore_ascii_case(subject) {
            continue;
        }
        let qtype = q["quoteType"].as_str().unwrap_or("");
        if qtype != "EQUITY" {
            continue;
        }
        out.push(sym.to_string());
        if out.len() >= limit {
            break;
        }
    }
    if out.is_empty() {
        fallback_peer_symbols(subject, industry, sector, limit)
    } else {
        out
    }
}

fn fallback_peer_symbols(subject: &str, industry: Option<&str>, sector: Option<&str>, limit: usize) -> Vec<String> {
    let mut peers: Vec<&str> = Vec::new();
    let ind = industry.unwrap_or("").to_lowercase();
    let sec = sector.unwrap_or("").to_lowercase();
    if ind.contains("information technology") || sec.contains("technology") {
        peers = vec![
            "TCS.NS",
            "HCLTECH.NS",
            "WIPRO.NS",
            "TECHM.NS",
            "LTIM.NS",
            "PERSISTENT.NS",
            "MPHASIS.NS",
            "COFORGE.NS",
        ];
    } else if ind.contains("bank") || sec.contains("financial") {
        peers = vec![
            "HDFCBANK.NS",
            "ICICIBANK.NS",
            "KOTAKBANK.NS",
            "AXISBANK.NS",
            "SBIN.NS",
            "INDUSINDBK.NS",
            "IDFCFIRSTB.NS",
            "BANKBARODA.NS",
        ];
    } else if ind.contains("pharma") || sec.contains("healthcare") {
        peers = vec![
            "SUNPHARMA.NS",
            "DRREDDY.NS",
            "CIPLA.NS",
            "DIVISLAB.NS",
            "TORNTPHARM.NS",
            "LUPIN.NS",
            "AUROPHARMA.NS",
            "ZYDUSLIFE.NS",
        ];
    } else if ind.contains("auto") || sec.contains("consumer cyclical") {
        peers = vec![
            "MARUTI.NS",
            "TATAMOTORS.NS",
            "M&M.NS",
            "BAJAJ-AUTO.NS",
            "EICHERMOT.NS",
            "HEROMOTOCO.NS",
            "TVSMOTOR.NS",
            "ASHOKLEY.NS",
        ];
    } else if ind.contains("energy") || ind.contains("oil") || sec.contains("energy") {
        peers = vec![
            "RELIANCE.NS",
            "ONGC.NS",
            "IOC.NS",
            "BPCL.NS",
            "HINDPETRO.NS",
            "GAIL.NS",
            "OIL.NS",
            "PETRONET.NS",
        ];
    } else if ind.contains("cement") || sec.contains("basic materials") {
        peers = vec![
            "ULTRACEMCO.NS",
            "SHREECEM.NS",
            "AMBUJACEM.NS",
            "ACC.NS",
            "DALBHARAT.NS",
            "RAMCOCEM.NS",
            "JKCEMENT.NS",
            "NUVOCO.NS",
        ];
    }

    peers
        .into_iter()
        .filter(|p| !p.eq_ignore_ascii_case(subject))
        .take(limit)
        .map(String::from)
        .collect()
}

/// Batch quote summary fields for peer comparison via v7 quote endpoint.
pub async fn fetch_peer_quotes(symbols: &[String]) -> Vec<PeerQuote> {
    if symbols.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for symbol in symbols {
        let Ok(v) = fetch_quote_summary(symbol, "price,financialData,summaryDetail,assetProfile").await else {
            continue;
        };
        let result = &v["quoteSummary"]["result"][0];
        let price_mod = &result["price"];
        let financial_data = &result["financialData"];
        let summary_detail = &result["summaryDetail"];
        let symbol = price_mod["symbol"]
            .as_str()
            .unwrap_or(symbol)
            .to_string();
        let price = price_mod["regularMarketPrice"]["raw"].as_f64().unwrap_or(0.0);
        let pe = summary_detail["trailingPE"]["raw"]
            .as_f64()
            .or_else(|| summary_detail["forwardPE"]["raw"].as_f64())
            .unwrap_or(0.0);
        let ebitda = financial_data["ebitda"]["raw"].as_f64().unwrap_or(0.0);
        let ev = financial_data["enterpriseValue"]["raw"].as_f64().unwrap_or(0.0);
        let ev_to_ebitda = if ev > 0.0 && ebitda > 0.0 {
            Some(ev / ebitda)
        } else {
            None
        };
        let mcap = price_mod["marketCap"]["raw"].as_f64().unwrap_or(0.0);
        let revenue = financial_data["totalRevenue"]["raw"].as_f64().unwrap_or(0.0);
        let revenue_growth = financial_data["revenueGrowth"]["raw"].as_f64().unwrap_or(0.0);
        let pat_growth = financial_data["earningsGrowth"]["raw"].as_f64().unwrap_or(0.0);
        let roe = financial_data["returnOnEquity"]["raw"].as_f64().unwrap_or(0.0);
        let roce = financial_data["returnOnAssets"]["raw"].as_f64();
        let margin = financial_data["profitMargins"]["raw"].as_f64().unwrap_or(0.0);
        let ebitda_margin = if revenue > 0.0 && ebitda > 0.0 {
            ebitda / revenue
        } else {
            0.0
        };
        let debt_to_equity = financial_data["debtToEquity"]["raw"].as_f64().unwrap_or(0.0);
        let free_cashflow = financial_data["freeCashflow"]["raw"].as_f64().unwrap_or(0.0);
        let mut officer_pay = 0.0;
        if let Some(officers) = result["assetProfile"]["companyOfficers"].as_array() {
            for officer in officers {
                officer_pay += officer["totalPay"]["raw"].as_f64().unwrap_or(0.0);
            }
        }
        rows.push(PeerQuote {
            symbol,
            short_name: price_mod["shortName"].as_str().map(String::from),
            price,
            pe_ratio: pe,
            ev_to_ebitda,
            market_cap: mcap,
            revenue,
            revenue_growth,
            pat_growth,
            ebitda,
            ebitda_margin,
            return_on_equity: roe,
            return_on_capital_employed: roce,
            profit_margins: margin,
            debt_to_equity,
            free_cashflow,
            officer_pay,
        });
    }
    rows
}
