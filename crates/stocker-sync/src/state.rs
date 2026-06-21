use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{ensure_config_dir, sync_state_path};
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    pub device_id: Uuid,
    pub drive_file_id: Option<String>,
    pub last_pushed_at: Option<DateTime<Utc>>,
    pub last_pulled_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_backup_files: Vec<String>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            device_id: Uuid::new_v4(),
            drive_file_id: None,
            last_pushed_at: None,
            last_pulled_at: None,
            last_backup_files: Vec::new(),
        }
    }
}

impl SyncState {
    pub fn load() -> Result<Self> {
        if crate::vault::is_configured() {
            return crate::vault::load_sync_state();
        }

        let path = sync_state_path();
        if !path.is_file() {
            let state = Self::default();
            state.save()?;
            return Ok(state);
        }
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        if crate::vault::is_configured() {
            return crate::vault::save_sync_state(self);
        }

        ensure_config_dir()?;
        let path = sync_state_path();
        let text = serde_json::to_string_pretty(self)?;
        write_private(&path, text.as_bytes())?;
        Ok(())
    }

    pub fn baseline_at(&self) -> Option<DateTime<Utc>> {
        match (self.last_pulled_at, self.last_pushed_at) {
            (Some(a), Some(b)) => Some(if a > b { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
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
        fs::write(path, bytes)?;
        Ok(())
    }
}
