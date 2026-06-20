use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::backup::{create_archive, restore_archive};
use crate::config::max_db_mtime;
use crate::config::OAuthConfig;
use crate::drive::{DriveClient, RemoteBackupInfo};
use crate::error::{Error, Result};
use crate::oauth::{authenticate, clear_authentication, is_authenticated};
use crate::state::SyncState;
use crate::vault::{self, vault_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRecommendation {
    AlreadyInSync,
    Pull,
    Push,
    Conflict,
    FirstPush,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub authenticated: bool,
    pub vault_configured: bool,
    pub vault_unlocked: bool,
    pub oauth_configured: bool,
    pub local_db_mtime: Option<DateTime<Utc>>,
    pub baseline_at: Option<DateTime<Utc>>,
    pub last_pushed_at: Option<DateTime<Utc>>,
    pub last_pulled_at: Option<DateTime<Utc>>,
    pub remote_exported_at: Option<DateTime<Utc>>,
    pub remote_modified_at: Option<DateTime<Utc>>,
    pub remote_size: Option<u64>,
    pub local_changed: bool,
    pub recommendation: SyncRecommendation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    Pulled,
    Pushed,
    AlreadyInSync,
}

pub fn decide(
    baseline_at: Option<DateTime<Utc>>,
    local_changed: bool,
    remote_exported_at: Option<DateTime<Utc>>,
) -> SyncRecommendation {
    match remote_exported_at {
        None => SyncRecommendation::FirstPush,
        Some(remote_ts) => {
            let baseline = baseline_at.unwrap_or(DateTime::<Utc>::MIN_UTC);
            if remote_ts > baseline && local_changed {
                SyncRecommendation::Conflict
            } else if remote_ts > baseline {
                SyncRecommendation::Pull
            } else if local_changed {
                SyncRecommendation::Push
            } else {
                SyncRecommendation::AlreadyInSync
            }
        }
    }
}

pub async fn status() -> Result<SyncStatus> {
    let vs = vault_status();
    let oauth_configured = oauth_configured();
    let state = if vs.configured && !vs.unlocked {
        SyncState::default()
    } else {
        SyncState::load()?
    };
    let local_db_mtime = max_db_mtime()?;
    let baseline_at = state.baseline_at();
    let local_changed = is_local_changed(&state, local_db_mtime);

    let authenticated = vs.unlocked && is_authenticated();

    let (remote_exported_at, remote_modified_at, remote_size) = if authenticated {
        let drive = DriveClient::new();
        let remote = match state.drive_file_id.as_deref() {
            Some(id) => drive.remote_info(id).await.ok(),
            None => drive.find_backup().await.ok().flatten(),
        };
        match remote {
            Some(info) => {
                let exported = info.exported_at.or(Some(info.modified_time));
                (exported, Some(info.modified_time), Some(info.size))
            }
            None => (None, None, None),
        }
    } else {
        (None, None, None)
    };

    let recommendation = decide(baseline_at, local_changed, remote_exported_at);

    Ok(SyncStatus {
        authenticated,
        vault_configured: vs.configured,
        vault_unlocked: vs.unlocked,
        oauth_configured,
        local_db_mtime,
        baseline_at,
        last_pushed_at: state.last_pushed_at,
        last_pulled_at: state.last_pulled_at,
        remote_exported_at,
        remote_modified_at,
        remote_size,
        local_changed,
        recommendation,
    })
}

fn oauth_configured() -> bool {
    if std::env::var("STOCKER_GOOGLE_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
        && std::env::var("STOCKER_GOOGLE_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .is_some()
    {
        return true;
    }
    if vault::is_configured() {
        return vault::is_unlocked()
            && vault_status()
                .has_oauth;
    }
    crate::config::oauth_config_path().is_file()
}

fn is_local_changed(state: &SyncState, local_db_mtime: Option<DateTime<Utc>>) -> bool {
    let Some(mtime) = local_db_mtime else {
        return false;
    };
    match state.last_pushed_at {
        Some(pushed) => mtime > pushed,
        None => true,
    }
}

pub async fn auth() -> Result<()> {
    ensure_vault_ready()?;
    authenticate().await?;
    Ok(())
}

pub fn logout() -> Result<()> {
    ensure_vault_ready()?;
    clear_authentication()
}

fn ensure_vault_ready() -> Result<()> {
    if vault::is_configured() && !vault::is_unlocked() {
        return Err(Error::VaultLocked);
    }
    OAuthConfig::load().map(|_| ())
}

pub async fn push(force: bool) -> Result<SyncAction> {
    ensure_vault_ready()?;
    let mut state = SyncState::load()?;
    let st = status().await?;
    if !force && st.recommendation == SyncRecommendation::Conflict {
        return Err(Error::Conflict);
    }
    if !force
        && st.recommendation == SyncRecommendation::AlreadyInSync
        && st.remote_exported_at.is_some()
    {
        return Ok(SyncAction::AlreadyInSync);
    }

    let temp_zip = tempfile::NamedTempFile::new()?;
    let manifest = create_archive(state.device_id, temp_zip.path()).await?;

    let drive = DriveClient::new();
    let file_id = drive
        .upload(temp_zip.path(), &manifest, state.drive_file_id.as_deref())
        .await?;

    state.drive_file_id = Some(file_id);
    state.last_pushed_at = Some(manifest.exported_at);
    state.save()?;

    Ok(SyncAction::Pushed)
}

pub async fn pull(force: bool) -> Result<SyncAction> {
    ensure_vault_ready()?;
    let mut state = SyncState::load()?;
    let st = status().await?;
    if !force && st.recommendation == SyncRecommendation::Conflict {
        return Err(Error::Conflict);
    }
    if !force && st.recommendation != SyncRecommendation::Pull {
        if st.recommendation == SyncRecommendation::AlreadyInSync {
            return Ok(SyncAction::AlreadyInSync);
        }
        if st.recommendation == SyncRecommendation::Push
            || st.recommendation == SyncRecommendation::FirstPush
        {
            return Err(Error::Other(
                "local data is newer than remote; use push instead".into(),
            ));
        }
    }

    let drive = DriveClient::new();
    let remote = resolve_remote(&drive, &state).await?;
    let temp_zip = tempfile::NamedTempFile::new()?;
    drive.download(&remote.file_id, temp_zip.path()).await?;
    let manifest = restore_archive(temp_zip.path())?;

    state.drive_file_id = Some(remote.file_id);
    state.last_pulled_at = Some(Utc::now());
    state.last_pushed_at = Some(manifest.exported_at);
    state.save()?;

    Ok(SyncAction::Pulled)
}

pub async fn sync(force: bool) -> Result<SyncAction> {
    ensure_vault_ready()?;
    let st = status().await?;
    match st.recommendation {
        SyncRecommendation::Pull => pull(force).await,
        SyncRecommendation::Push | SyncRecommendation::FirstPush => push(force).await,
        SyncRecommendation::Conflict if force => push(true).await,
        SyncRecommendation::Conflict => Err(Error::Conflict),
        SyncRecommendation::AlreadyInSync => Ok(SyncAction::AlreadyInSync),
    }
}

async fn resolve_remote(drive: &DriveClient, state: &SyncState) -> Result<RemoteBackupInfo> {
    if let Some(id) = state.drive_file_id.as_deref() {
        if let Ok(info) = drive.remote_info(id).await {
            return Ok(info);
        }
    }
    drive
        .find_backup()
        .await?
        .ok_or_else(|| Error::Other("no remote backup found on Google Drive".into()))
}

/// Startup preflight: pull from Drive when remote is newer. Ignores auth/conflict errors.
pub async fn startup_pull_if_newer() -> Result<Option<SyncAction>> {
    if !vault::startup_allowed() {
        return Ok(None);
    }
    if !is_authenticated() {
        return Ok(None);
    }
    let st = status().await?;
    if st.recommendation == SyncRecommendation::Pull {
        return pull(false).await.map(Some);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_first_push() {
        assert_eq!(
            decide(None, true, None),
            SyncRecommendation::FirstPush
        );
    }

    #[test]
    fn decide_pull_when_remote_newer() {
        let remote = Utc::now();
        let baseline = remote - chrono::Duration::hours(1);
        assert_eq!(
            decide(Some(baseline), false, Some(remote)),
            SyncRecommendation::Pull
        );
    }

    #[test]
    fn decide_push_when_local_changed() {
        let remote = Utc::now() - chrono::Duration::hours(2);
        let baseline = Utc::now() - chrono::Duration::hours(1);
        assert_eq!(
            decide(Some(baseline), true, Some(remote)),
            SyncRecommendation::Push
        );
    }

    #[test]
    fn decide_conflict() {
        let remote = Utc::now();
        let baseline = remote - chrono::Duration::hours(1);
        assert_eq!(
            decide(Some(baseline), true, Some(remote)),
            SyncRecommendation::Conflict
        );
    }
}
