//! mfapi.in HTTP client for mutual fund search and NAV.

use reqwest::Client;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::models::{MfSearchHit, NavPoint, SchemeMeta};

const BASE_URL: &str = "https://api.mfapi.in";

#[derive(Clone)]
pub struct MfFetcher {
    client: Client,
}

impl Default for MfFetcher {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl MfFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn search(&self, query: &str) -> Result<Vec<MfSearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{BASE_URL}/mf/search?q={}", urlencoding(q));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Api(format!(
                "mfapi search failed: HTTP {}",
                resp.status()
            )));
        }
        let raw: Vec<SearchRow> = resp.json().await?;
        Ok(raw
            .into_iter()
            .map(|r| MfSearchHit {
                scheme_code: r.scheme_code,
                scheme_name: r.scheme_name,
            })
            .collect())
    }

    /// Paginated scheme list from `GET /mf` (used to build the local import cache).
    pub async fn fetch_schemes_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<crate::scheme_index::SchemeListEntry>> {
        let url = format!("{BASE_URL}/mf?limit={limit}&offset={offset}");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Api(format!(
                "mfapi scheme list failed: HTTP {}",
                resp.status()
            )));
        }
        let rows: Vec<crate::scheme_index::SchemeListEntry> = resp.json().await?;
        Ok(rows)
    }

    pub async fn fetch_latest(&self, scheme_code: i64) -> Result<(SchemeMeta, f64, String)> {
        let url = format!("{BASE_URL}/mf/{scheme_code}/latest");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Api(format!(
                "mfapi latest failed for {scheme_code}: HTTP {}",
                resp.status()
            )));
        }
        let body: LatestResponse = resp.json().await?;
        if body.status.as_deref() != Some("SUCCESS") {
            return Err(Error::Api(format!(
                "mfapi latest for {scheme_code}: status {:?}",
                body.status
            )));
        }
        let meta = body.meta.ok_or_else(|| {
            Error::Api(format!("mfapi latest for {scheme_code}: missing meta"))
        })?;
        let nav_row = body.data.into_iter().next().ok_or_else(|| {
            Error::Api(format!("mfapi latest for {scheme_code}: empty data"))
        })?;
        let nav: f64 = nav_row
            .nav
            .parse()
            .map_err(|_| Error::Api(format!("invalid nav: {}", nav_row.nav)))?;

        Ok((
            SchemeMeta {
                scheme_code: meta.scheme_code,
                scheme_name: meta.scheme_name,
                fund_house: meta.fund_house,
                scheme_category: meta.scheme_category,
                isin_growth: meta.isin_growth,
            },
            nav,
            nav_row.date,
        ))
    }

    /// NAV history for a date range (`startDate`/`endDate` in `YYYY-MM-DD`).
    pub async fn fetch_nav_range(
        &self,
        scheme_code: i64,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<NavPoint>> {
        let url = format!(
            "{BASE_URL}/mf/{scheme_code}?startDate={start_date}&endDate={end_date}"
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Api(format!(
                "mfapi nav range failed for {scheme_code}: HTTP {}",
                resp.status()
            )));
        }
        let body: LatestResponse = resp.json().await?;
        if body.status.as_deref() != Some("SUCCESS") {
            return Err(Error::Api(format!(
                "mfapi nav range for {scheme_code}: status {:?}",
                body.status
            )));
        }
        let mut points = Vec::with_capacity(body.data.len());
        for row in body.data {
            let nav: f64 = row
                .nav
                .parse()
                .map_err(|_| Error::Api(format!("invalid nav: {}", row.nav)))?;
            let nav_date = parse_mfapi_date(&row.date)?;
            points.push(NavPoint { nav_date, nav });
        }
        Ok(points)
    }
}

/// Parse mfapi `DD-MM-YYYY` into ISO `YYYY-MM-DD`.
pub fn parse_mfapi_date(raw: &str) -> Result<String> {
    let parts: Vec<&str> = raw.trim().split('-').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidInput(format!("invalid mfapi date: {raw}")));
    }
    let day: u32 = parts[0]
        .parse()
        .map_err(|_| Error::InvalidInput(format!("invalid mfapi date day: {raw}")))?;
    let month: u32 = parts[1]
        .parse()
        .map_err(|_| Error::InvalidInput(format!("invalid mfapi date month: {raw}")))?;
    let year: i32 = parts[2]
        .parse()
        .map_err(|_| Error::InvalidInput(format!("invalid mfapi date year: {raw}")))?;
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Pick the earliest NAV point on or after `sip_date` (`YYYY-MM-DD`).
///
/// This is the next available published NAV / trading day after the SIP date.
pub fn first_nav_on_or_after(points: &[NavPoint], sip_date: &str) -> Option<NavPoint> {
    points
        .iter()
        .filter(|p| p.nav_date.as_str() >= sip_date)
        .min_by(|a, b| a.nav_date.cmp(&b.nav_date))
        .cloned()
}

/// Deprecated name retained for callers; same as [`first_nav_on_or_after`].
pub fn latest_nav_on_or_after(points: &[NavPoint], sip_date: &str) -> Option<NavPoint> {
    first_nav_on_or_after(points, sip_date)
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct SearchRow {
    #[serde(rename = "schemeCode")]
    scheme_code: i64,
    #[serde(rename = "schemeName")]
    scheme_name: String,
}

#[derive(Debug, Deserialize)]
struct LatestResponse {
    meta: Option<LatestMeta>,
    data: Vec<NavRow>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LatestMeta {
    #[serde(rename = "fund_house")]
    fund_house: Option<String>,
    #[serde(rename = "scheme_category")]
    scheme_category: Option<String>,
    #[serde(rename = "scheme_code")]
    scheme_code: i64,
    #[serde(rename = "scheme_name")]
    scheme_name: String,
    #[serde(rename = "isin_growth")]
    isin_growth: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NavRow {
    date: String,
    nav: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_latest_json() {
        let json = r#"{
            "meta": {
                "fund_house": "PPFAS Mutual Fund",
                "scheme_category": "Equity Scheme - Flexi Cap Fund",
                "scheme_code": 122639,
                "scheme_name": "Parag Parikh Flexi Cap Fund - Direct Plan - Growth",
                "isin_growth": "INF879O01027"
            },
            "data": [{"date": "12-06-2026", "nav": "89.29450"}],
            "status": "SUCCESS"
        }"#;
        let body: LatestResponse = serde_json::from_str(json).unwrap();
        assert_eq!(body.meta.as_ref().unwrap().scheme_code, 122639);
        assert_eq!(body.data[0].nav, "89.29450");
    }

    #[test]
    fn parse_search_json() {
        let json = r#"[{"schemeCode":122639,"schemeName":"Parag Parikh Flexi Cap Fund - Direct Plan - Growth"}]"#;
        let rows: Vec<SearchRow> = serde_json::from_str(json).unwrap();
        assert_eq!(rows[0].scheme_code, 122639);
    }

    #[test]
    fn parse_mfapi_date_to_iso() {
        assert_eq!(parse_mfapi_date("12-06-2026").unwrap(), "2026-06-12");
        assert_eq!(parse_mfapi_date("08-06-2026").unwrap(), "2026-06-08");
    }

    #[test]
    fn parse_nav_range_json() {
        let json = r#"{
            "data": [
                {"date": "12-06-2026", "nav": "118.41660"},
                {"date": "08-06-2026", "nav": "117.26510"}
            ],
            "status": "SUCCESS"
        }"#;
        let body: LatestResponse = serde_json::from_str(json).unwrap();
        let mut points: Vec<NavPoint> = body
            .data
            .iter()
            .map(|row| NavPoint {
                nav_date: parse_mfapi_date(&row.date).unwrap(),
                nav: row.nav.parse().unwrap(),
            })
            .collect();
        let best = first_nav_on_or_after(&points, "2026-06-08").unwrap();
        assert_eq!(best.nav_date, "2026-06-08");
        assert!((best.nav - 117.26510).abs() < 1e-6);

        let after_weekend = first_nav_on_or_after(&points, "2026-06-09").unwrap();
        assert_eq!(after_weekend.nav_date, "2026-06-12");
        assert!((after_weekend.nav - 118.41660).abs() < 1e-6);

        points.retain(|p| p.nav_date.as_str() >= "2026-06-08");
        assert_eq!(points.len(), 2);
    }
}
