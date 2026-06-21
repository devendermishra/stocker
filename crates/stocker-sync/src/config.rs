use std::path::{Path, PathBuf};

pub const BACKUP_FILENAME: &str = "stocker-backup.zip";
pub const MANIFEST_FILENAME: &str = "manifest.json";
pub const PORTFOLIO_DB_NAME: &str = "portfolio.db";
pub const SCREENER_DB_NAME: &str = "stocker.db";
pub const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
pub const APP_PROPERTY_EXPORTED_AT: &str = "exported_at";

/// Config directory for tokens and sync state (`~/.config/stocker` or platform equivalent).
pub fn config_dir() -> PathBuf {
    stocker_core::paths::config_dir()
}

pub fn tokens_path() -> PathBuf {
    config_dir().join("google_tokens.json")
}

pub fn oauth_config_path() -> PathBuf {
    config_dir().join("google_oauth.json")
}

pub fn sync_state_path() -> PathBuf {
    config_dir().join("sync_state.json")
}

/// Zip downloaded from Drive when databases were locked; applied on next app start.
pub fn pending_restore_path() -> PathBuf {
    config_dir().join("pending_restore.zip")
}

pub fn portfolio_db_path() -> PathBuf {
    stocker_portfolio::default_db_path()
}

pub fn screener_db_path() -> PathBuf {
    stocker_screener::db::default_db_path()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl OAuthConfig {
    pub fn load() -> crate::error::Result<Self> {
        if let (Ok(id), Ok(secret)) = (
            std::env::var("STOCKER_GOOGLE_CLIENT_ID"),
            std::env::var("STOCKER_GOOGLE_CLIENT_SECRET"),
        ) {
            if !id.is_empty() && !secret.is_empty() {
                return Ok(Self {
                    client_id: id,
                    client_secret: secret,
                });
            }
        }

        if crate::vault::is_configured() {
            return crate::vault::load_oauth_config();
        }

        let path = oauth_config_path();
        if path.is_file() {
            let text = std::fs::read_to_string(&path)?;
            return Ok(serde_json::from_str(&text)?);
        }

        Err(crate::error::Error::MissingOAuthConfig)
    }
}

pub fn ensure_config_dir() -> crate::error::Result<PathBuf> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn file_mtime(path: &Path) -> crate::error::Result<Option<chrono::DateTime<chrono::Utc>>> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let modified = meta.modified()?;
    Ok(Some(chrono::DateTime::<chrono::Utc>::from(modified)))
}

pub fn max_db_mtime() -> crate::error::Result<Option<chrono::DateTime<chrono::Utc>>> {
    let mut latest: Option<chrono::DateTime<chrono::Utc>> = None;
    for path in [portfolio_db_path(), screener_db_path()] {
        if let Some(ts) = file_mtime(&path)? {
            latest = Some(match latest {
                Some(cur) if cur >= ts => cur,
                _ => ts,
            });
        }
    }
    Ok(latest)
}
