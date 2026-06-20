use std::time::Duration;

use chrono::{DateTime, Utc};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
    basic::BasicClient,
    reqwest::async_http_client,
};
use serde::{Deserialize, Serialize};

use crate::config::{DRIVE_SCOPE, OAuthConfig, ensure_config_dir, tokens_path};
use crate::error::{Error, Result};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl StoredTokens {
    pub fn load() -> Result<Option<Self>> {
        if crate::vault::is_configured() {
            return crate::vault::load_tokens();
        }

        let path = tokens_path();
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)?;
        Ok(Some(serde_json::from_str(&text)?))
    }

    pub fn save(&self) -> Result<()> {
        if crate::vault::is_configured() {
            return crate::vault::save_tokens(self);
        }

        ensure_config_dir()?;
        let text = serde_json::to_string_pretty(self)?;
        write_tokens_file(&text)?;
        Ok(())
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => Utc::now() >= exp - chrono::Duration::seconds(60),
            None => false,
        }
    }
}

fn write_tokens_file(text: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(tokens_path())?;
        file.write_all(text.as_bytes())?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        std::fs::write(tokens_path(), text)?;
        Ok(())
    }
}

fn oauth_client(redirect_uri: &str) -> Result<BasicClient> {
    let cfg = OAuthConfig::load()?;
    let client = BasicClient::new(
        ClientId::new(cfg.client_id),
        Some(ClientSecret::new(cfg.client_secret)),
        AuthUrl::new(AUTH_URL.to_string()).map_err(|e| Error::Oauth(e.to_string()))?,
        Some(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| Error::Oauth(e.to_string()))?),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect_uri.to_string()).map_err(|e| Error::Oauth(e.to_string()))?,
    );
    Ok(client)
}

fn tokens_from_response(
    token: oauth2::StandardTokenResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
) -> StoredTokens {
    StoredTokens {
        access_token: token.access_token().secret().clone(),
        refresh_token: token.refresh_token().map(|t| t.secret().clone()),
        expires_at: token
            .expires_in()
            .map(|d| Utc::now() + chrono::Duration::seconds(d.as_secs() as i64)),
    }
}

/// Run the desktop OAuth flow (localhost redirect) and persist tokens.
pub async fn authenticate() -> Result<StoredTokens> {
    clear_authentication()?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let client = oauth_client(&redirect_uri)?;

    let (auth_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(DRIVE_SCOPE.to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .url();

    eprintln!("Opening browser for Google sign-in…");
    eprintln!("If the browser does not open, visit:\n{auth_url}");
    if webbrowser::open(auth_url.as_ref()).is_err() {
        eprintln!("Could not open browser automatically.");
    }

    let (code, returned_state) = wait_for_callback(listener).await?;
    if returned_state.secret() != csrf_state.secret() {
        return Err(Error::Oauth("CSRF state mismatch".to_string()));
    }

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await
        .map_err(|e| Error::Oauth(e.to_string()))?;

    let stored = tokens_from_response(token);
    stored.save()?;
    eprintln!("Google Drive authentication saved.");
    Ok(stored)
}

async fn wait_for_callback(listener: tokio::net::TcpListener) -> Result<(String, CsrfToken)> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    loop {
        let accept = tokio::time::timeout_at(deadline, listener.accept()).await;
        let (mut stream, _) = accept.map_err(|_| {
            Error::Oauth("timed out waiting for OAuth callback".to_string())
        })??;

        let mut buf = vec![0u8; 4096];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await?;
        let request = String::from_utf8_lossy(&buf[..n]);

        if let Some(line) = request.lines().next() {
            if let Some(path) = line.split_whitespace().nth(1) {
                if path.starts_with("/callback") {
                    let query = path.split('?').nth(1).unwrap_or("");
                    let mut code = None;
                    let mut state = None;
                    for pair in query.split('&') {
                        let mut parts = pair.splitn(2, '=');
                        match parts.next() {
                            Some("code") => code = parts.next().map(str::to_string),
                            Some("state") => state = parts.next().map(str::to_string),
                            _ => {}
                        }
                    }

                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                        <html><body><p>Authentication complete. You can close this tab.</p></body></html>";
                    tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await?;

                    let code = code.ok_or_else(|| Error::Oauth("missing code in callback".into()))?;
                    let state = state.ok_or_else(|| Error::Oauth("missing state in callback".into()))?;
                    return Ok((code, CsrfToken::new(state)));
                }
            }
        }
    }
}

pub async fn valid_access_token() -> Result<String> {
    let mut tokens = StoredTokens::load()?.ok_or(Error::NotAuthenticated)?;
    if !tokens.is_expired() {
        return Ok(tokens.access_token);
    }

    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or_else(|| Error::Oauth("access token expired and no refresh token".into()))?;

    let cfg = OAuthConfig::load()?;
    let client = BasicClient::new(
        ClientId::new(cfg.client_id),
        Some(ClientSecret::new(cfg.client_secret)),
        AuthUrl::new(AUTH_URL.to_string()).map_err(|e| Error::Oauth(e.to_string()))?,
        Some(TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| Error::Oauth(e.to_string()))?),
    );

    let token = client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh))
        .request_async(async_http_client)
        .await
        .map_err(|e| Error::Oauth(e.to_string()))?;

    let updated = tokens_from_response(token);
    if updated.refresh_token.is_none() {
        tokens.access_token = updated.access_token;
        tokens.expires_at = updated.expires_at;
    } else {
        tokens = updated;
    }
    tokens.save()?;
    Ok(tokens.access_token)
}

pub fn is_authenticated() -> bool {
    StoredTokens::load()
        .ok()
        .flatten()
        .is_some_and(|t| t.refresh_token.is_some() || !t.access_token.is_empty())
}

/// Remove stored Google tokens so the user can re-authorize with fresh scopes.
pub fn clear_authentication() -> Result<()> {
    if crate::vault::is_configured() {
        return crate::vault::clear_tokens();
    }
    let path = tokens_path();
    if path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
