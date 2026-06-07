use crate::types::ResearchReport;

#[cfg(any(feature = "web", feature = "desktop"))]
pub const API_BASE: &str = match option_env!("STOCKER_API_URL") {
    Some(u) => u,
    None => "http://127.0.0.1:8080",
};

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub async fn load_research_report(symbol: String) -> Result<ResearchReport, String> {
    let url = format!(
        "{}/api/v1/symbols/{}/report",
        API_BASE,
        urlencoding::encode(&symbol)
    );
    let resp = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}: {}", status, text));
    }
    serde_json::from_str::<ResearchReport>(&text).map_err(|e| e.to_string())
}

#[cfg(all(feature = "desktop", not(feature = "web")))]
pub async fn load_research_report(symbol: String) -> Result<ResearchReport, String> {
    stocker_core::build_research_report(&symbol, None, None)
        .await
        .map_err(|e| e.to_string())
}
