//! Index serialization and metadata for cache validity.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use repoask_core::index::InvertedIndex;

/// Current index format version. Bump when the index structure changes.
const INDEX_FORMAT_VERSION: u32 = 1;

/// Metadata about a cached index for validity checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    /// Git commit hash at the time of indexing.
    pub commit_hash: String,
    /// Unix timestamp when the index was built.
    pub indexed_at: u64,
    /// Version of repoask that built the index.
    pub repoask_version: String,
    /// Format version for compatibility checking.
    pub index_format_version: u32,
}

impl IndexMeta {
    /// Create metadata for a freshly built index.
    pub fn new(commit_hash: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            commit_hash,
            indexed_at: now,
            repoask_version: env!("CARGO_PKG_VERSION").to_string(),
            index_format_version: INDEX_FORMAT_VERSION,
        }
    }

    /// Check if the index format is compatible with the current version.
    pub fn is_compatible(&self) -> bool {
        self.index_format_version == INDEX_FORMAT_VERSION
    }

    /// Check if the index was built from the same commit.
    pub fn matches_commit(&self, commit_hash: &str) -> bool {
        self.commit_hash == commit_hash
    }
}

/// Save an index to disk using bincode.
pub fn save_index(index: &InvertedIndex, path: &Path) -> Result<(), SaveError> {
    let config = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(index, config).map_err(SaveError::Encode)?;
    std::fs::write(path, &bytes)?;
    Ok(())
}

/// Load an index from disk.
pub fn load_index(path: &Path) -> Result<InvertedIndex, LoadError> {
    let bytes = std::fs::read(path)?;
    let config = bincode::config::standard();
    let (index, _) = bincode::serde::decode_from_slice::<InvertedIndex, _>(&bytes, config)
        .map_err(LoadError::Decode)?;
    Ok(index)
}

/// Save index metadata as JSON.
pub fn save_meta(meta: &IndexMeta, path: &Path) -> Result<(), SaveError> {
    let json = serde_json::to_string_pretty(meta).map_err(SaveError::Json)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load index metadata from JSON.
pub fn load_meta(path: &Path) -> Result<IndexMeta, LoadError> {
    let json = std::fs::read_to_string(path)?;
    let meta: IndexMeta = serde_json::from_str(&json).map_err(LoadError::Json)?;
    Ok(meta)
}

/// Error saving index or metadata.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SaveError {
    /// IO error writing file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Bincode encoding error.
    #[error("encode error: {0}")]
    Encode(bincode::error::EncodeError),
    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}

/// Error loading index or metadata.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// IO error reading file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Bincode decoding error.
    #[error("decode error: {0}")]
    Decode(bincode::error::DecodeError),
    /// JSON deserialization error.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}
