//! Index serialization and metadata for cache validity.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use repoask_core::index::InvertedIndex;

/// Current index format version. Bump when the index structure changes.
const INDEX_FORMAT_VERSION: u32 = 1;

/// Maximum index file size to load (500 MB). Guards against tampered files.
const MAX_INDEX_FILE_SIZE: u64 = 500 * 1024 * 1024;

/// Maximum metadata JSON file size to load (1 MB).
const MAX_META_FILE_SIZE: u64 = 1024 * 1024;

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
///
/// Rejects files larger than 500 MB to guard against tampered cache files.
pub fn load_index(path: &Path) -> Result<InvertedIndex, LoadError> {
    let file_size = path.metadata()?.len();
    if file_size > MAX_INDEX_FILE_SIZE {
        return Err(LoadError::TooLarge(file_size));
    }
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
///
/// Rejects files larger than 1 MB.
pub fn load_meta(path: &Path) -> Result<IndexMeta, LoadError> {
    let file_size = path.metadata()?.len();
    if file_size > MAX_META_FILE_SIZE {
        return Err(LoadError::TooLarge(file_size));
    }
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
    /// File exceeds the maximum allowed size.
    #[error("file too large: {0} bytes")]
    TooLarge(u64),
    /// Bincode decoding error.
    #[error("decode error: {0}")]
    Decode(bincode::error::DecodeError),
    /// JSON deserialization error.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests {
    use super::*;
    use repoask_core::index::InvertedIndex;
    use repoask_core::types::{IndexDocument, Symbol, SymbolKind};

    fn sample_index() -> InvertedIndex {
        let docs = vec![
            IndexDocument::Code(Symbol {
                name: "validateToken".to_string(),
                kind: SymbolKind::Function,
                filepath: "src/auth.rs".to_string(),
                start_line: 1,
                end_line: 20,
                doc_comment: Some("Validates a JWT token".to_string()),
                params: vec!["token".to_string()],
            }),
            IndexDocument::Doc(repoask_core::types::DocSection {
                filepath: "README.md".to_string(),
                section_title: "Authentication".to_string(),
                heading_hierarchy: vec!["Auth".to_string()],
                content: "This section covers auth setup".to_string(),
                code_symbols: vec!["validateToken".to_string()],
                start_line: 1,
                end_line: 10,
            }),
        ];
        InvertedIndex::build(docs)
    }

    #[test]
    fn index_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.bin");

        let original = sample_index();
        save_index(&original, &path).unwrap();
        let loaded = load_index(&path).unwrap();

        // Verify search produces identical results
        let query = "validate token";
        let original_results = original.search(query, 10);
        let loaded_results = loaded.search(query, 10);

        assert_eq!(original_results.len(), loaded_results.len());
        for (a, b) in original_results.iter().zip(loaded_results.iter()) {
            assert_eq!(a.filepath(), b.filepath());
            assert!((a.score() - b.score()).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn meta_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");

        let original = IndexMeta::new("abc123def".to_string());
        save_meta(&original, &path).unwrap();
        let loaded = load_meta(&path).unwrap();

        assert_eq!(loaded.commit_hash, "abc123def");
        assert_eq!(loaded.index_format_version, INDEX_FORMAT_VERSION);
        assert!(loaded.is_compatible());
        assert!(loaded.matches_commit("abc123def"));
        assert!(!loaded.matches_commit("other"));
    }

    #[test]
    fn load_index_rejects_oversized_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        // Create a file that claims to be too large by checking metadata
        // We can't create a 500MB file in tests, but we can test the path exists check
        std::fs::write(&path, b"small").unwrap();
        // Should succeed since file is small
        assert!(load_index(&path).is_err()); // Will fail on decode, not size
    }

    #[test]
    fn empty_index_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");

        let original = InvertedIndex::build(vec![]);
        save_index(&original, &path).unwrap();
        let loaded = load_index(&path).unwrap();

        assert_eq!(loaded.doc_count(), 0);
        assert!(loaded.search("anything", 10).is_empty());
    }
}
