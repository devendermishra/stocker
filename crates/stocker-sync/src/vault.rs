use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::config::{OAuthConfig, ensure_config_dir, oauth_config_path, tokens_path};
use crate::error::{Error, Result};
use crate::oauth::StoredTokens;
use crate::state::SyncState;

const VAULT_VERSION: u32 = 1;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

static SESSION: OnceLock<RwLock<Option<VaultSession>>> = OnceLock::new();

fn session_lock() -> &'static RwLock<Option<VaultSession>> {
    SESSION.get_or_init(|| RwLock::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultData {
    oauth: OAuthConfig,
    tokens: Option<StoredTokens>,
    sync_state: SyncState,
}

struct VaultSession {
    key: Zeroizing<[u8; KEY_LEN]>,
    data: VaultData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedVaultFile {
    version: u32,
    salt: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStatus {
    pub configured: bool,
    pub unlocked: bool,
    pub has_oauth: bool,
    pub authenticated: bool,
}

pub fn vault_path() -> PathBuf {
    crate::config::config_dir().join("sync_vault.enc")
}

pub fn is_configured() -> bool {
    vault_path().is_file()
}

pub fn is_unlocked() -> bool {
    session_lock()
        .read()
        .ok()
        .is_some_and(|s| s.is_some())
}

pub fn vault_status() -> VaultStatus {
    let configured = is_configured();
    let unlocked = is_unlocked();
    let (has_oauth, authenticated) = if unlocked {
        if let Ok(guard) = session_lock().read() {
            if let Some(session) = guard.as_ref() {
                let auth = session
                    .data
                    .tokens
                    .as_ref()
                    .is_some_and(|t| t.refresh_token.is_some() || !t.access_token.is_empty());
                (
                    !session.data.oauth.client_id.is_empty()
                        && !session.data.oauth.client_secret.is_empty(),
                    auth,
                )
            } else {
                (false, false)
            }
        } else {
            (false, false)
        }
    } else {
        (false, false)
    };

    VaultStatus {
        configured,
        unlocked,
        has_oauth,
        authenticated,
    }
}

fn validate_password(password: &str) -> Result<()> {
    if password.len() < 6 {
        return Err(Error::Other(
            "password must be at least 6 characters".into(),
        ));
    }
    Ok(())
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| Error::Other(format!("key derivation failed: {e}")))?;
    Ok(key)
}

fn decrypt_with_key(key: &[u8], file: &EncryptedVaultFile) -> Result<Vec<u8>> {
    if file.version != VAULT_VERSION {
        return Err(Error::Other(format!(
            "unsupported vault version {}",
            file.version
        )));
    }
    let nonce_bytes = B64
        .decode(&file.nonce)
        .map_err(|e| Error::Other(format!("invalid vault nonce: {e}")))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(Error::Other("invalid vault nonce length".into()));
    }
    let ciphertext = B64
        .decode(&file.ciphertext)
        .map_err(|e| Error::Other(format!("invalid vault ciphertext: {e}")))?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| Error::VaultWrongPassword)
}

fn decrypt_file(password: &str, file: &EncryptedVaultFile) -> Result<VaultData> {
    let salt = B64
        .decode(&file.salt)
        .map_err(|e| Error::Other(format!("invalid vault salt: {e}")))?;
    if salt.len() != SALT_LEN {
        return Err(Error::Other("invalid vault salt length".into()));
    }
    let key = derive_key(password, &salt)?;
    let plaintext = decrypt_with_key(key.as_ref(), file)?;
    let data: VaultData = serde_json::from_slice(&plaintext)?;
    Ok(data)
}

fn read_encrypted_file() -> Result<EncryptedVaultFile> {
    let text = std::fs::read_to_string(vault_path())?;
    Ok(serde_json::from_str(&text)?)
}

fn write_encrypted_file(file: &EncryptedVaultFile) -> Result<()> {
    ensure_config_dir()?;
    let text = serde_json::to_string_pretty(file)?;
    write_private(&vault_path(), text.as_bytes())
}

fn write_encrypted_with_key(key: &[u8], salt_b64: &str, data: &VaultData) -> Result<()> {
    let plaintext = serde_json::to_vec(data)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| Error::Other(format!("encrypt vault: {e}")))?;

    write_encrypted_file(&EncryptedVaultFile {
        version: VAULT_VERSION,
        salt: salt_b64.to_string(),
        nonce: B64.encode(nonce_bytes),
        ciphertext: B64.encode(ciphertext),
    })
}

fn persist_session(session: &VaultSession) -> Result<()> {
    let existing = read_encrypted_file()?;
    write_encrypted_with_key(session.key.as_ref(), &existing.salt, &session.data)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

fn migrate_legacy(data: &mut VaultData) -> Result<()> {
    let oauth_path = oauth_config_path();
    if oauth_path.is_file() {
        if data.oauth.client_id.is_empty() || data.oauth.client_secret.is_empty() {
            let text = std::fs::read_to_string(&oauth_path)?;
            data.oauth = serde_json::from_str(&text)?;
        }
        std::fs::remove_file(&oauth_path)?;
    }

    let tokens_file = tokens_path();
    if tokens_file.is_file() {
        let text = std::fs::read_to_string(&tokens_file)?;
        data.tokens = Some(serde_json::from_str(&text)?);
        std::fs::remove_file(&tokens_file)?;
    }

    let state_path = crate::config::sync_state_path();
    if state_path.is_file() {
        let text = std::fs::read_to_string(&state_path)?;
        data.sync_state = serde_json::from_str(&text)?;
        std::fs::remove_file(&state_path)?;
    }

    Ok(())
}

pub fn setup_vault(password: &str, oauth: OAuthConfig) -> Result<()> {
    validate_password(password)?;
    if oauth.client_id.is_empty() || oauth.client_secret.is_empty() {
        return Err(Error::MissingOAuthConfig);
    }
    if is_configured() {
        return Err(Error::Other("vault already configured".into()));
    }

    let mut data = VaultData {
        oauth,
        tokens: None,
        sync_state: SyncState::default(),
    };
    migrate_legacy(&mut data)?;

    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let salt_b64 = B64.encode(salt);
    let key = derive_key(password, &salt)?;
    write_encrypted_with_key(key.as_ref(), &salt_b64, &data)?;

    let mut guard = session_lock()
        .write()
        .map_err(|_| Error::Other("vault session lock poisoned".into()))?;
    *guard = Some(VaultSession { key, data });
    Ok(())
}

pub fn unlock_vault(password: &str) -> Result<()> {
    validate_password(password)?;
    if !is_configured() {
        return Err(Error::VaultNotConfigured);
    }

    let file = read_encrypted_file()?;
    let mut data = decrypt_file(password, &file)?;
    let had_legacy = oauth_config_path().is_file()
        || tokens_path().is_file()
        || crate::config::sync_state_path().is_file();
    migrate_legacy(&mut data)?;

    let salt = B64
        .decode(&file.salt)
        .map_err(|e| Error::Other(format!("invalid vault salt: {e}")))?;
    let key = derive_key(password, &salt)?;
    let session = VaultSession { key, data };

    if had_legacy {
        persist_session(&session)?;
    }

    let mut guard = session_lock()
        .write()
        .map_err(|_| Error::Other("vault session lock poisoned".into()))?;
    *guard = Some(session);
    Ok(())
}

pub fn lock_vault() {
    if let Ok(mut guard) = session_lock().write() {
        *guard = None;
    }
}

fn with_session<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut VaultSession) -> Result<T>,
{
    if !is_configured() {
        return Err(Error::VaultNotConfigured);
    }
    if !is_unlocked() {
        return Err(Error::VaultLocked);
    }
    let mut guard = session_lock()
        .write()
        .map_err(|_| Error::Other("vault session lock poisoned".into()))?;
    let session = guard.as_mut().ok_or(Error::VaultLocked)?;
    let result = f(session)?;
    persist_session(session)?;
    Ok(result)
}

fn with_session_read<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&VaultSession) -> Result<T>,
{
    if !is_configured() {
        return Err(Error::VaultNotConfigured);
    }
    if !is_unlocked() {
        return Err(Error::VaultLocked);
    }
    let guard = session_lock()
        .read()
        .map_err(|_| Error::Other("vault session lock poisoned".into()))?;
    let session = guard.as_ref().ok_or(Error::VaultLocked)?;
    f(session)
}

pub fn load_oauth_config() -> Result<OAuthConfig> {
    with_session_read(|s| Ok(s.data.oauth.clone()))
}

pub fn load_tokens() -> Result<Option<StoredTokens>> {
    with_session_read(|s| Ok(s.data.tokens.clone()))
}

pub fn save_tokens(tokens: &StoredTokens) -> Result<()> {
    with_session(|s| {
        s.data.tokens = Some(tokens.clone());
        Ok(())
    })
}

pub fn load_sync_state() -> Result<SyncState> {
    with_session_read(|s| Ok(s.data.sync_state.clone()))
}

pub fn save_sync_state(state: &SyncState) -> Result<()> {
    with_session(|s| {
        s.data.sync_state = state.clone();
        Ok(())
    })
}

pub fn clear_tokens() -> Result<()> {
    if !is_configured() {
        let path = tokens_path();
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    with_session(|s| {
        s.data.tokens = None;
        Ok(())
    })
}

pub fn startup_allowed() -> bool {
    if is_configured() {
        is_unlocked()
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::env::set_var("STOCKER_CONFIG_DIR", dir.path());
        lock_vault();
        dir
    }

    #[test]
    fn setup_unlock_round_trip() {
        let _dir = test_config_dir();
        setup_vault(
            "secret12",
            OAuthConfig {
                client_id: "id.apps.googleusercontent.com".into(),
                client_secret: "secret".into(),
            },
        )
        .unwrap();
        assert!(is_configured());
        assert!(is_unlocked());

        lock_vault();
        assert!(!is_unlocked());

        unlock_vault("secret12").unwrap();
        assert!(is_unlocked());

        let oauth = load_oauth_config().unwrap();
        assert_eq!(oauth.client_id, "id.apps.googleusercontent.com");
    }

    #[test]
    fn wrong_password_fails() {
        let _dir = test_config_dir();
        setup_vault(
            "secret12",
            OAuthConfig {
                client_id: "id".into(),
                client_secret: "sec".into(),
            },
        )
        .unwrap();
        lock_vault();
        assert!(matches!(
            unlock_vault("wrongpass"),
            Err(Error::VaultWrongPassword)
        ));
    }

    #[test]
    fn tokens_persist_across_lock_unlock() {
        let _dir = test_config_dir();
        setup_vault(
            "secret12",
            OAuthConfig {
                client_id: "id".into(),
                client_secret: "sec".into(),
            },
        )
        .unwrap();

        save_tokens(&StoredTokens {
            access_token: "tok".into(),
            refresh_token: Some("ref".into()),
            expires_at: None,
        })
        .unwrap();

        lock_vault();
        unlock_vault("secret12").unwrap();
        let tokens = load_tokens().unwrap().unwrap();
        assert_eq!(tokens.access_token, "tok");
    }

    #[test]
    fn migrate_legacy_oauth_file() {
        let dir = test_config_dir();
        ensure_config_dir().unwrap();
        std::fs::write(
            oauth_config_path(),
            serde_json::to_string_pretty(&OAuthConfig {
                client_id: "legacy-id".into(),
                client_secret: "legacy-sec".into(),
            })
            .unwrap(),
        )
        .unwrap();

        setup_vault(
            "secret12",
            OAuthConfig {
                client_id: "new-id".into(),
                client_secret: "new-sec".into(),
            },
        )
        .unwrap();

        assert!(!oauth_config_path().is_file());
        let oauth = load_oauth_config().unwrap();
        assert_eq!(oauth.client_id, "new-id");
        let _ = dir;
    }
}
