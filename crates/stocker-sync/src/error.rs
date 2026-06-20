use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("oauth: {0}")]
    Oauth(String),
    #[error("not authenticated — run `stocker-cli sync auth` first")]
    NotAuthenticated,
    #[error("missing Google OAuth credentials — set STOCKER_GOOGLE_CLIENT_ID and STOCKER_GOOGLE_CLIENT_SECRET")]
    MissingOAuthConfig,
    #[error("sync vault is not configured — set up Google OAuth in the Sync page")]
    VaultNotConfigured,
    #[error("sync vault is locked — enter your master password")]
    VaultLocked,
    #[error("wrong master password")]
    VaultWrongPassword,
    #[error("sync conflict: local and remote both changed; use --force push or --force pull")]
    Conflict,
    #[error("checksum mismatch for {0}")]
    ChecksumMismatch(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
