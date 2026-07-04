//! Read-only portfolio APIs backed by the Google Drive backup.

use crate::portfolio_api::{
    AllocationRow, Dashboard, FifoLot, Holding, Transaction, TransactionFilter,
};

#[cfg(feature = "desktop")]
pub use desktop::*;

#[cfg(feature = "desktop")]
mod desktop {
    use stocker_portfolio::TransactionFilter as BackendFilter;
    use stocker_sync::{
        remote_browse_index, remote_exported_at, remote_portfolio_allocation_label,
        remote_portfolio_allocation_stock, remote_portfolio_dashboard, remote_portfolio_holdings,
        remote_portfolio_stock_lots, remote_portfolio_transactions,
    };

    use super::{
        AllocationRow, Dashboard, FifoLot, Holding, Transaction, TransactionFilter as UiFilter,
    };

    pub use stocker_sync::{PortfolioSyncEntry, PortfolioSyncState, RemoteBrowseIndex, RemoteBrowseSummary};

    fn convert<T: serde::Serialize, U: serde::de::DeserializeOwned>(value: T) -> Result<U, String> {
        serde_json::from_value(serde_json::to_value(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }

    fn map_filter(filter: &UiFilter) -> Result<BackendFilter, String> {
        convert(filter.clone())
    }

    pub async fn sync_remote_browse_index(force_refresh: bool) -> Result<RemoteBrowseIndex, String> {
        let local = crate::portfolio_api::local_portfolio_refs_for_sync().await?;
        remote_browse_index(force_refresh, local)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn sync_remote_exported_at() -> Result<Option<String>, String> {
        remote_exported_at()
            .await
            .map(|ts| ts.map(|t| t.to_rfc3339()))
            .map_err(|e| e.to_string())
    }

    pub async fn remote_dashboard(portfolio_id: i64) -> Result<Dashboard, String> {
        convert(
            remote_portfolio_dashboard(portfolio_id)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    pub async fn remote_holdings(portfolio_id: i64) -> Result<Vec<Holding>, String> {
        convert(
            remote_portfolio_holdings(portfolio_id)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    pub async fn remote_transactions(
        portfolio_id: i64,
        filter: &UiFilter,
    ) -> Result<Vec<Transaction>, String> {
        let filter = map_filter(filter)?;
        convert(
            remote_portfolio_transactions(portfolio_id, &filter)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    pub async fn remote_allocation_stock(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
        convert(
            remote_portfolio_allocation_stock(portfolio_id)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    pub async fn remote_allocation_label(portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
        convert(
            remote_portfolio_allocation_label(portfolio_id)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    pub async fn remote_fifo_lots(portfolio_id: i64, symbol: &str) -> Result<Vec<FifoLot>, String> {
        convert(
            remote_portfolio_stock_lots(portfolio_id, symbol)
                .await
                .map_err(|e| e.to_string())?,
        )
    }
}

#[cfg(not(feature = "desktop"))]
pub use stub::*;

#[cfg(not(feature = "desktop"))]
mod stub {
    use serde::{Deserialize, Serialize};

    use super::{AllocationRow, Dashboard, FifoLot, Holding, Transaction, TransactionFilter};

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
        pub transaction_count: Option<i64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
    pub struct RemoteBrowseSummary {
        pub on_drive: u32,
        pub synced: u32,
        pub pending_pull: u32,
        pub pending_push: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RemoteBrowseIndex {
        pub remote_exported_at: Option<String>,
        pub entries: Vec<PortfolioSyncEntry>,
        pub has_portfolio_db: bool,
        pub too_large: bool,
        pub summary: RemoteBrowseSummary,
        pub error: Option<String>,
    }

    pub async fn sync_remote_browse_index(_force_refresh: bool) -> Result<RemoteBrowseIndex, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn sync_remote_exported_at() -> Result<Option<String>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_dashboard(_portfolio_id: i64) -> Result<Dashboard, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_holdings(_portfolio_id: i64) -> Result<Vec<Holding>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_transactions(
        _portfolio_id: i64,
        _filter: &TransactionFilter,
    ) -> Result<Vec<Transaction>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_allocation_stock(_portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_allocation_label(_portfolio_id: i64) -> Result<Vec<AllocationRow>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }

    pub async fn remote_fifo_lots(_portfolio_id: i64, _symbol: &str) -> Result<Vec<FifoLot>, String> {
        Err("Google Drive sync is available in the desktop app only".into())
    }
}
