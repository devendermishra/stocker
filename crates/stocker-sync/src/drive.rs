use std::path::Path;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::backup::read_manifest_from_zip;
use crate::config::{APP_PROPERTY_EXPORTED_AT, BACKUP_FILENAME};
use crate::error::{Error, Result};
use crate::manifest::SyncManifest;
use crate::oauth::valid_access_token;

const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";

#[derive(Debug, Clone)]
pub struct RemoteBackupInfo {
    pub file_id: String,
    pub modified_time: DateTime<Utc>,
    pub exported_at: Option<DateTime<Utc>>,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct DriveFileList {
    files: Vec<DriveFile>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    #[serde(default)]
    modified_time: Option<String>,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    app_properties: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct GoogleApiErrorBody {
    error: GoogleApiError,
}

#[derive(Debug, Deserialize)]
struct GoogleApiError {
    message: String,
    #[serde(default)]
    errors: Vec<GoogleApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct GoogleApiErrorDetail {
    reason: Option<String>,
}

pub struct DriveClient {
    http: Client,
}

impl DriveClient {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    pub async fn find_backup(&self) -> Result<Option<RemoteBackupInfo>> {
        let token = valid_access_token().await?;
        let url = format!(
            "{DRIVE_API}/files?spaces=appDataFolder&fields=files(id,name,modifiedTime,size,appProperties)&q=name='{BACKUP_FILENAME}'"
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;
        let resp = check_drive_response(resp).await?;
        let list: DriveFileList = resp.json().await?;
        let file = match list.files.into_iter().next() {
            Some(f) => f,
            None => return Ok(None),
        };
        Ok(Some(parse_remote_file(file)?))
    }

    pub async fn remote_info(&self, file_id: &str) -> Result<RemoteBackupInfo> {
        let token = valid_access_token().await?;
        let url = format!(
            "{DRIVE_API}/files/{file_id}?fields=id,modifiedTime,size,appProperties"
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;
        let resp = check_drive_response(resp).await?;
        let file: DriveFile = resp.json().await?;
        parse_remote_file(file)
    }

    pub async fn download(&self, file_id: &str, dest: &Path) -> Result<()> {
        let token = valid_access_token().await?;
        let url = format!("{DRIVE_API}/files/{file_id}?alt=media");
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;
        let mut resp = check_drive_response(resp).await?;
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut file = std::fs::File::create(dest)?;
        while let Some(chunk) = resp.chunk().await? {
            std::io::Write::write_all(&mut file, &chunk)?;
        }
        Ok(())
    }

    pub async fn upload(
        &self,
        zip_path: &Path,
        manifest: &SyncManifest,
        file_id: Option<&str>,
    ) -> Result<String> {
        let token = valid_access_token().await?;
        let bytes = std::fs::read(zip_path)?;
        let exported_at = manifest.exported_at.to_rfc3339();

        if let Some(id) = file_id {
            let url = format!("{DRIVE_UPLOAD}/files/{id}?uploadType=media");
            let resp = self
                .http
                .patch(&url)
                .bearer_auth(&token)
                .header("Content-Type", "application/zip")
                .body(bytes)
                .send()
                .await?;
            let resp = check_drive_response(resp).await?;
            let updated: DriveFile = resp.json().await?;
            self.patch_metadata(&updated.id, &exported_at).await?;
            return Ok(updated.id);
        }

        let metadata = serde_json::json!({
            "name": BACKUP_FILENAME,
            "mimeType": "application/zip",
            "parents": ["appDataFolder"],
            "appProperties": {
                APP_PROPERTY_EXPORTED_AT: exported_at,
            }
        });

        let part1 = format!(
            "--boundary\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n",
            metadata
        );
        let part2_header = "--boundary\r\nContent-Type: application/zip\r\n\r\n";
        let closing = "\r\n--boundary--";

        let mut body = Vec::new();
        body.extend_from_slice(part1.as_bytes());
        body.extend_from_slice(part2_header.as_bytes());
        body.extend_from_slice(&bytes);
        body.extend_from_slice(closing.as_bytes());

        let url = format!("{DRIVE_UPLOAD}/files?uploadType=multipart&fields=id");
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .header("Content-Type", "multipart/related; boundary=boundary")
            .body(body)
            .send()
            .await?;
        let resp = check_drive_response(resp).await?;
        let created: DriveFile = resp.json().await?;
        Ok(created.id)
    }

    async fn patch_metadata(&self, file_id: &str, exported_at: &str) -> Result<()> {
        let token = valid_access_token().await?;
        let url = format!("{DRIVE_API}/files/{file_id}");
        let body = serde_json::json!({
            "appProperties": {
                APP_PROPERTY_EXPORTED_AT: exported_at,
            }
        });
        let resp = self
            .http
            .patch(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;
        check_drive_response(resp).await?;
        Ok(())
    }

    pub async fn remote_exported_at_from_backup(
        &self,
        info: &RemoteBackupInfo,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
        if let Some(ts) = info.exported_at {
            return Ok(Some(ts));
        }
        let temp = tempfile::NamedTempFile::new()?;
        self.download(&info.file_id, temp.path()).await?;
        let manifest = read_manifest_from_zip(temp.path())?;
        Ok(Some(manifest.exported_at))
    }
}

async fn check_drive_response(resp: reqwest::Response) -> Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(parse_drive_error(status.as_u16(), &body))
}

fn parse_drive_error(status: u16, body: &str) -> Error {
    let mut message = body.to_string();
    let mut reason = None;
    if let Ok(parsed) = serde_json::from_str::<GoogleApiErrorBody>(body) {
        message = parsed.error.message;
        reason = parsed
            .error
            .errors
            .first()
            .and_then(|e| e.reason.clone());
    }

    let hint = if status == 403
        && (reason.as_deref() == Some("insufficientScopes")
            || message.contains("Application Data folder")
            || message.contains("insufficient"))
    {
        " Sign out of Google Drive in the Sync page, then sign in again. Also confirm the Google Drive API is enabled and the OAuth consent screen includes the drive.appdata scope."
    } else if status == 403 {
        " Confirm the Google Drive API is enabled in Google Cloud Console and your Google account is listed as a test user on the OAuth consent screen."
    } else {
        ""
    };

    Error::Other(format!("Drive API {status}: {message}.{hint}"))
}

fn parse_remote_file(file: DriveFile) -> Result<RemoteBackupInfo> {
    let modified_time = file
        .modified_time
        .as_deref()
        .and_then(parse_drive_time)
        .ok_or_else(|| Error::Other("missing modifiedTime from Drive".into()))?;
    let exported_at = file
        .app_properties
        .as_ref()
        .and_then(|p| p.get(APP_PROPERTY_EXPORTED_AT))
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let size = file
        .size
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok(RemoteBackupInfo {
        file_id: file.id,
        modified_time,
        exported_at,
        size,
    })
}

fn parse_drive_time(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
