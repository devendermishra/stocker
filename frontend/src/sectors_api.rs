//! Sector research API client (web HTTP / desktop in-process).

use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use crate::api::API_BASE;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectorListItem {
    pub sector: String,
    pub company_count: i64,
    pub with_snapshot_count: i64,
    pub total_market_cap: Option<f64>,
    pub lifecycle: String,
    pub sector_type: String,
    pub attractiveness: f64,
    pub growth_prospects: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectorListResponse {
    pub sectors: Vec<SectorListItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectorMember {
    pub symbol: String,
    pub short_name: Option<String>,
    pub market_cap: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PorterForceView {
    pub name: String,
    pub intensity: f64,
    pub label: String,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PorterFiveForcesView {
    pub rivalry: PorterForceView,
    pub new_entrants: PorterForceView,
    pub supplier_power: PorterForceView,
    pub buyer_power: PorterForceView,
    pub substitutes: PorterForceView,
    pub attractiveness: f64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SectorLifecycleView {
    pub phase: String,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SectorTypeView {
    pub sector_type: String,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DemandSupplyView {
    pub gap_label: String,
    pub intensity: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompetitionView {
    pub structure: String,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScoredLevelView {
    pub level: String,
    pub score: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PricingSideView {
    pub level: String,
    pub score: f64,
    pub narrative: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PricingPowerView {
    pub supplier: PricingSideView,
    pub customer: PricingSideView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SectorResearchProfileView {
    pub sector: String,
    pub company_count: usize,
    pub porter: PorterFiveForcesView,
    pub lifecycle: SectorLifecycleView,
    pub sector_type: SectorTypeView,
    pub demand_supply: DemandSupplyView,
    pub competition: CompetitionView,
    pub profitability: ScoredLevelView,
    pub growth_prospects: ScoredLevelView,
    pub pricing_power: PricingPowerView,
    #[serde(default)]
    pub interpretation_confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectorDetail {
    pub sector: String,
    pub company_count: i64,
    pub with_snapshot_count: i64,
    pub total_market_cap: Option<f64>,
    pub research: SectorResearchProfileView,
    pub members: Vec<SectorMember>,
}

pub fn encode_sector_path(sector: &str) -> String {
    urlencoding::encode(sector).into_owned()
}

#[cfg(feature = "web")]
mod web_backend {
    use super::*;

    pub async fn list_sectors() -> Result<Vec<SectorListItem>, String> {
        let url = format!("{}/api/v1/sectors", API_BASE);
        let resp = gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("sectors list HTTP {}", resp.status()));
        }
        let body: SectorListResponse = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.sectors)
    }

    pub async fn get_sector(sector: &str) -> Result<SectorDetail, String> {
        let url = format!(
            "{}/api/v1/sectors/{}",
            API_BASE,
            encode_sector_path(sector)
        );
        let resp = gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status() == 404 {
            return Err("Sector not found".into());
        }
        if !resp.ok() {
            return Err(format!("sector detail HTTP {}", resp.status()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }
}

#[cfg(feature = "desktop")]
mod desktop_backend {
    use super::*;
    use crate::screener_api::shared_screener;

    fn via_json<T: serde::de::DeserializeOwned, U: Serialize>(v: U) -> Result<T, String> {
        let j = serde_json::to_value(v).map_err(|e| e.to_string())?;
        serde_json::from_value(j).map_err(|e| e.to_string())
    }

    pub async fn list_sectors() -> Result<Vec<SectorListItem>, String> {
        let svc = shared_screener().await?;
        let items = svc.list_sectors().await.map_err(|e| e.to_string())?;
        via_json(items)
    }

    pub async fn get_sector(sector: &str) -> Result<SectorDetail, String> {
        let svc = shared_screener().await?;
        let detail = svc.sector_detail(sector).await.map_err(|e| e.to_string())?;
        via_json(detail)
    }
}

#[cfg(feature = "web")]
pub use web_backend::*;
#[cfg(feature = "desktop")]
pub use desktop_backend::*;
