use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stocker_portfolio::{
    auth::LOCAL_USER_EMAIL, analytics::PortfolioViewOptions, db, portfolios, PortfolioService,
    TransactionFilter,
};
use stocker_portfolio::models::{Dashboard, Portfolio, PortfolioStatus, Transaction};
use tokio::sync::{Mutex, OnceCell};

use crate::backup::{extract_file_from_zip, read_manifest_from_zip};
use crate::config::{portfolio_db_path, PORTFOLIO_DB_NAME};
use crate::drive::{DriveClient, RemoteBackupInfo};
use crate::error::{Error, Result};
use crate::manifest::SyncManifest;
use crate::oauth::is_authenticated;
use crate::service::{ensure_vault_ready, resolve_remote};
use crate::state::SyncState;
use crate::vault::vault_status;

const REMOTE_INSPECT_MAX_BYTES: u64 = 200 * 1024 * 1024;

fn map_portfolio_err(err: stocker_portfolio::Error) -> Error {
    Error::Other(err.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioSyncState {
    Matched,
    DriveOnly,
    LocalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSyncEntry {
    pub name: String,
    pub state: PortfolioSyncState,
    pub local_id: Option<i64>,
    pub remote_id: Option<i64>,
    pub status: Option<PortfolioStatus>,
    pub transaction_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPortfolioRef {
    pub id: i64,
    pub name: String,
    pub status: Option<PortfolioStatus>,
}

impl From<Portfolio> for LocalPortfolioRef {
    fn from(p: Portfolio) -> Self {
        Self {
            id: p.id,
            name: p.name,
            status: Some(p.status),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RemoteBrowseSummary {
    /// Portfolios present in the Google Drive backup.
    pub on_drive: u32,
    /// Same name exists locally and on Drive.
    pub synced: u32,
    /// On Drive but not local yet — pull to get them.
    pub pending_pull: u32,
    /// Local only — push to upload to Drive.
    pub pending_push: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBrowseIndex {
    pub remote_exported_at: Option<DateTime<Utc>>,
    pub remote_modified_at: Option<DateTime<Utc>>,
    pub has_portfolio_db: bool,
    pub too_large: bool,
    pub summary: RemoteBrowseSummary,
    pub entries: Vec<PortfolioSyncEntry>,
    pub error: Option<String>,
}

pub fn summarize_entries(entries: &[PortfolioSyncEntry]) -> RemoteBrowseSummary {
    let mut summary = RemoteBrowseSummary::default();
    for entry in entries {
        if entry.remote_id.is_some() {
            summary.on_drive += 1;
        }
        match entry.state {
            PortfolioSyncState::Matched => summary.synced += 1,
            PortfolioSyncState::DriveOnly => summary.pending_pull += 1,
            PortfolioSyncState::LocalOnly => summary.pending_push += 1,
        }
    }
    summary
}

struct RemoteSnapshot {
    remote: RemoteBackupInfo,
    manifest: SyncManifest,
    portfolio_db: PathBuf,
    service: OnceCell<Arc<PortfolioService>>,
}

static REMOTE_CACHE: Mutex<Option<Arc<RemoteSnapshot>>> = Mutex::const_new(None);

pub fn invalidate_remote_cache() {
    if let Ok(mut guard) = REMOTE_CACHE.try_lock() {
        *guard = None;
    }
}

pub async fn remote_browse_index(
    force_refresh: bool,
    local_portfolios: Vec<LocalPortfolioRef>,
) -> Result<RemoteBrowseIndex> {
    ensure_vault_ready()?;
    let vs = vault_status();
    if !vs.unlocked || !is_authenticated() {
        return Err(Error::Other(
            "sign in to Google Drive to browse remote portfolios".into(),
        ));
    }

    match load_snapshot(force_refresh).await {
        Ok(snapshot) => {
            let remote_exported_at = Some(snapshot.manifest.exported_at);
            let remote_modified_at = Some(snapshot.remote.modified_time);
            let has_portfolio_db = snapshot.manifest.files.contains_key(PORTFOLIO_DB_NAME);
            let entries = if has_portfolio_db {
                build_diff_index(&snapshot, &local_portfolios).await?
            } else {
                local_only_entries(&local_portfolios)
            };
            Ok(RemoteBrowseIndex {
                remote_exported_at,
                remote_modified_at,
                has_portfolio_db,
                too_large: false,
                summary: summarize_entries(&entries),
                entries,
                error: None,
            })
        }
        Err(e) if is_too_large_error(&e) => {
            let entries = local_only_entries(&local_portfolios);
            Ok(RemoteBrowseIndex {
                remote_exported_at: None,
                remote_modified_at: None,
                has_portfolio_db: false,
                too_large: true,
                summary: summarize_entries(&entries),
                entries,
                error: Some(e.to_string()),
            })
        }
        Err(Error::Other(msg)) if msg.contains("no remote backup") => {
            let entries = local_only_entries(&local_portfolios);
            Ok(RemoteBrowseIndex {
                remote_exported_at: None,
                remote_modified_at: None,
                has_portfolio_db: false,
                too_large: false,
                summary: summarize_entries(&entries),
                entries,
                error: None,
            })
        }
        Err(e) => Err(e),
    }
}

pub async fn remote_exported_at() -> Result<Option<DateTime<Utc>>> {
    let snapshot = load_snapshot(false).await?;
    Ok(Some(snapshot.manifest.exported_at))
}

async fn remote_service(force_refresh: bool) -> Result<(Arc<RemoteSnapshot>, Arc<PortfolioService>)> {
    let snapshot = load_snapshot(force_refresh).await?;
    let svc = snapshot
        .service
        .get_or_try_init(|| async {
            PortfolioService::open_readonly(&snapshot.portfolio_db)
                .await
                .map(Arc::new)
                .map_err(map_portfolio_err)
        })
        .await
        .cloned()
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok((snapshot, svc))
}

async fn remote_user_id(svc: &PortfolioService) -> Result<i64> {
    if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(LOCAL_USER_EMAIL)
        .fetch_optional(svc.pool())
        .await
        .map_err(|e| Error::Other(e.to_string()))?
    {
        return Ok(id);
    }
    sqlx::query_scalar::<_, i64>("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_optional(svc.pool())
        .await
        .map_err(|e| Error::Other(e.to_string()))?
        .ok_or_else(|| Error::Other("remote backup has no users".into()))
}

pub async fn remote_portfolio_dashboard(portfolio_id: i64) -> Result<Dashboard> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    svc.dashboard(uid, portfolio_id, PortfolioViewOptions::default())
        .await
        .map_err(map_portfolio_err)
}

pub async fn remote_portfolio_holdings(portfolio_id: i64) -> Result<Vec<stocker_portfolio::models::Holding>> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    svc.holdings(uid, portfolio_id, PortfolioViewOptions::default())
        .await
        .map_err(map_portfolio_err)
}

pub async fn remote_portfolio_transactions(
    portfolio_id: i64,
    filter: &TransactionFilter,
) -> Result<Vec<Transaction>> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    let mut f = filter.clone();
    f.portfolio_id = Some(portfolio_id);
    svc.list_transactions(uid, &f)
        .await
        .map_err(map_portfolio_err)
}

pub async fn remote_portfolio_allocation_stock(
    portfolio_id: i64,
) -> Result<Vec<stocker_portfolio::models::AllocationRow>> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    svc.allocation_by_stock(uid, portfolio_id, PortfolioViewOptions::default())
        .await
        .map_err(map_portfolio_err)
}

pub async fn remote_portfolio_allocation_label(
    portfolio_id: i64,
) -> Result<Vec<stocker_portfolio::models::AllocationRow>> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    svc.allocation_by_label(uid, portfolio_id, PortfolioViewOptions::default())
        .await
        .map_err(map_portfolio_err)
}

pub async fn remote_portfolio_stock_lots(
    portfolio_id: i64,
    symbol: &str,
) -> Result<Vec<stocker_portfolio::models::FifoLot>> {
    let (snapshot, svc) = remote_service(false).await?;
    ensure_remote_portfolio_db(&snapshot)?;
    let uid = remote_user_id(&svc).await?;
    svc.fifo_lots(uid, portfolio_id, symbol)
        .await
        .map_err(map_portfolio_err)
}

fn ensure_remote_portfolio_db(snapshot: &RemoteSnapshot) -> Result<()> {
    if snapshot.portfolio_db.is_file() {
        Ok(())
    } else {
        Err(Error::Other(
            "remote backup does not contain portfolio.db".into(),
        ))
    }
}

async fn load_snapshot(force_refresh: bool) -> Result<Arc<RemoteSnapshot>> {
    let drive = DriveClient::new();
    let state = SyncState::load()?;
    let remote = resolve_remote(&drive, &state).await?;

    if !force_refresh {
        let guard = REMOTE_CACHE.lock().await;
        if let Some(cached) = guard.as_ref() {
            if cached.remote.file_id == remote.file_id
                && cached.remote.modified_time == remote.modified_time
            {
                return Ok(Arc::clone(cached));
            }
        }
    }

    if remote.size > REMOTE_INSPECT_MAX_BYTES {
        return Err(Error::Other(format!(
            "Drive backup is {} bytes — remote browse supports backups up to {} bytes",
            remote.size, REMOTE_INSPECT_MAX_BYTES
        )));
    }

    let temp_dir = tempfile::tempdir()?;
    let zip_path = temp_dir.path().join("stocker-backup.zip");
    drive.download(&remote.file_id, &zip_path).await?;
    let manifest = read_manifest_from_zip(&zip_path)?;

    let portfolio_db = temp_dir.path().join(PORTFOLIO_DB_NAME);
    if manifest.files.contains_key(PORTFOLIO_DB_NAME) {
        extract_file_from_zip(&zip_path, PORTFOLIO_DB_NAME, &portfolio_db)?;
    }

    let snapshot = Arc::new(RemoteSnapshot {
        remote,
        manifest,
        portfolio_db,
        service: OnceCell::new(),
    });

    std::mem::forget(temp_dir);

    let mut guard = REMOTE_CACHE.lock().await;
    *guard = Some(Arc::clone(&snapshot));
    Ok(snapshot)
}

async fn build_diff_index(
    snapshot: &RemoteSnapshot,
    local: &[LocalPortfolioRef],
) -> Result<Vec<PortfolioSyncEntry>> {
    let remote_portfolios = list_remote_portfolio_rows(snapshot).await?;
    Ok(merge_diff(remote_portfolios, local))
}

fn local_only_entries(local: &[LocalPortfolioRef]) -> Vec<PortfolioSyncEntry> {
    local
        .iter()
        .map(|p| PortfolioSyncEntry {
            name: p.name.clone(),
            state: PortfolioSyncState::LocalOnly,
            local_id: Some(p.id),
            remote_id: None,
            status: p.status,
            transaction_count: None,
        })
        .collect()
}

/// Load local portfolio names for CLI use (does not run from the desktop UI path).
pub async fn load_local_portfolio_refs() -> Result<Vec<LocalPortfolioRef>> {
    Ok(list_local_portfolios()
        .await?
        .into_iter()
        .map(LocalPortfolioRef::from)
        .collect())
}

#[derive(Clone)]
struct RemotePortfolioRow {
    id: i64,
    name: String,
    status: PortfolioStatus,
    transaction_count: i64,
}

async fn list_remote_portfolio_rows(snapshot: &RemoteSnapshot) -> Result<Vec<RemotePortfolioRow>> {
    if !snapshot.portfolio_db.is_file() {
        return Ok(Vec::new());
    }
    let pool = db::open_existing_readonly(&snapshot.portfolio_db)
        .await
        .map_err(map_portfolio_err)?;
    let user_id = remote_user_id_from_pool(&pool).await?;
    let list = portfolios::list(&pool, user_id, true)
        .await
        .map_err(map_portfolio_err)?;
    let mut rows = Vec::with_capacity(list.len());
    for p in list {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE portfolio_id = ?")
            .bind(p.id)
            .fetch_one(&pool)
            .await?;
        rows.push(RemotePortfolioRow {
            id: p.id,
            name: p.name,
            status: p.status,
            transaction_count: count,
        });
    }
    pool.close().await;
    Ok(rows)
}

async fn list_local_portfolios() -> Result<Vec<Portfolio>> {
    let path = portfolio_db_path();
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let pool = db::open_existing_readonly(&path)
        .await
        .map_err(map_portfolio_err)?;
    let user_id = remote_user_id_from_pool(&pool).await?;
    let list = portfolios::list(&pool, user_id, true)
        .await
        .map_err(map_portfolio_err)?;
    pool.close().await;
    Ok(list)
}

async fn remote_user_id_from_pool(pool: &sqlx::SqlitePool) -> Result<i64> {
    if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(LOCAL_USER_EMAIL)
        .fetch_optional(pool)
        .await?
    {
        return Ok(id);
    }
    sqlx::query_scalar::<_, i64>("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::Other("portfolio database has no users".into()))
}

fn merge_diff(
    remote: Vec<RemotePortfolioRow>,
    local: &[LocalPortfolioRef],
) -> Vec<PortfolioSyncEntry> {
    let mut remote_by_name: BTreeMap<String, RemotePortfolioRow> = BTreeMap::new();
    for row in remote {
        remote_by_name.insert(row.name.clone(), row);
    }
    let mut local_by_name: HashMap<String, &LocalPortfolioRef> = HashMap::new();
    for p in local {
        local_by_name.insert(p.name.clone(), p);
    }

    let mut names: BTreeMap<String, ()> = BTreeMap::new();
    for name in remote_by_name.keys() {
        names.insert(name.clone(), ());
    }
    for name in local_by_name.keys() {
        names.insert(name.clone(), ());
    }

    names
        .into_keys()
        .map(|name| {
            let remote_row = remote_by_name.get(&name);
            let local_row = local_by_name.get(&name);
            let state = match (remote_row, local_row) {
                (Some(_), Some(_)) => PortfolioSyncState::Matched,
                (Some(_), None) => PortfolioSyncState::DriveOnly,
                (None, Some(_)) => PortfolioSyncState::LocalOnly,
                (None, None) => unreachable!(),
            };
            PortfolioSyncEntry {
                name: name.clone(),
                state,
                local_id: local_row.map(|p| p.id),
                remote_id: remote_row.map(|p| p.id),
                status: remote_row
                    .map(|p| p.status)
                    .or_else(|| local_row.and_then(|p| p.status)),
                transaction_count: remote_row.map(|p| p.transaction_count),
            }
        })
        .collect()
}

fn is_too_large_error(err: &Error) -> bool {
    matches!(err, Error::Other(msg) if msg.contains("remote browse supports backups up to"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stocker_portfolio::models::PortfolioStatus;

    fn remote_row(id: i64, name: &str) -> RemotePortfolioRow {
        RemotePortfolioRow {
            id,
            name: name.to_string(),
            status: PortfolioStatus::Active,
            transaction_count: id,
        }
    }

    fn local_portfolio(id: i64, name: &str) -> LocalPortfolioRef {
        LocalPortfolioRef {
            id,
            name: name.to_string(),
            status: Some(PortfolioStatus::Active),
        }
    }

    #[test]
    fn merge_diff_matched_drive_only_local_only() {
        let remote = vec![remote_row(1, "Alpha"), remote_row(2, "Beta")];
        let local = vec![local_portfolio(10, "Alpha"), local_portfolio(11, "Gamma")];
        let entries = merge_diff(remote, &local);
        assert_eq!(entries.len(), 3);

        let alpha = entries.iter().find(|e| e.name == "Alpha").unwrap();
        assert_eq!(alpha.state, PortfolioSyncState::Matched);
        assert_eq!(alpha.local_id, Some(10));
        assert_eq!(alpha.remote_id, Some(1));

        let beta = entries.iter().find(|e| e.name == "Beta").unwrap();
        assert_eq!(beta.state, PortfolioSyncState::DriveOnly);

        let gamma = entries.iter().find(|e| e.name == "Gamma").unwrap();
        assert_eq!(gamma.state, PortfolioSyncState::LocalOnly);

        let summary = summarize_entries(&entries);
        assert_eq!(summary.on_drive, 2);
        assert_eq!(summary.synced, 1);
        assert_eq!(summary.pending_pull, 1);
        assert_eq!(summary.pending_push, 1);
    }
}
