use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncManifest {
    pub version: u32,
    pub exported_at: DateTime<Utc>,
    pub device_id: Uuid,
    pub files: BTreeMap<String, FileEntry>,
}

impl SyncManifest {
    pub fn new(device_id: Uuid, files: BTreeMap<String, FileEntry>) -> Self {
        Self {
            version: MANIFEST_VERSION,
            exported_at: Utc::now(),
            device_id,
            files,
        }
    }

    pub fn to_json(&self) -> crate::error::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(text: &str) -> crate::error::Result<Self> {
        Ok(serde_json::from_str(text)?)
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn file_entry(path: &std::path::Path) -> crate::error::Result<FileEntry> {
    let bytes = std::fs::read(path)?;
    Ok(FileEntry {
        sha256: sha256_hex(&bytes),
        size: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let mut files = BTreeMap::new();
        files.insert(
            "portfolio.db".to_string(),
            FileEntry {
                sha256: "abc".to_string(),
                size: 42,
            },
        );
        let manifest = SyncManifest::new(Uuid::new_v4(), files);
        let json = manifest.to_json().unwrap();
        let parsed = SyncManifest::from_json(&json).unwrap();
        assert_eq!(manifest, parsed);
    }
}
