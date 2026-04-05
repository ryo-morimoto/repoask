//! Investigation corpus serialization for overview and future inspect surfaces.

use std::path::Path;

use repoask_core::investigation::InvestigationCorpus;
use serde::{Deserialize, Serialize};

/// Current corpus format version. Bump when the persisted corpus structure changes.
const CORPUS_FORMAT_VERSION: u32 = 1;

/// Maximum corpus file size to load (500 MB). Guards against tampered cache files.
const MAX_CORPUS_FILE_SIZE: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedCorpus {
    format_version: u32,
    corpus: InvestigationCorpus,
}

/// Save a corpus to disk using postcard.
///
/// # Errors
///
/// Returns an error if serialization fails or the file cannot be written.
pub fn save_corpus(corpus: &InvestigationCorpus, path: &Path) -> Result<(), SaveError> {
    let payload = PersistedCorpus {
        format_version: CORPUS_FORMAT_VERSION,
        corpus: corpus.clone(),
    };
    let bytes = postcard::to_stdvec(&payload).map_err(SaveError::Encode)?;
    std::fs::write(path, &bytes)?;
    Ok(())
}

/// Load a corpus from disk.
///
/// # Errors
///
/// Returns an error if the file is too large, cannot be read, or fails compatibility checks.
pub fn load_corpus(path: &Path) -> Result<InvestigationCorpus, LoadError> {
    let file_size = path.metadata()?.len();
    if file_size > MAX_CORPUS_FILE_SIZE {
        return Err(LoadError::TooLarge(file_size));
    }

    let bytes = std::fs::read(path)?;
    let payload = postcard::from_bytes::<PersistedCorpus>(&bytes).map_err(LoadError::Decode)?;
    if payload.format_version != CORPUS_FORMAT_VERSION {
        return Err(LoadError::IncompatibleFormat {
            found: payload.format_version,
            expected: CORPUS_FORMAT_VERSION,
        });
    }

    Ok(payload.corpus)
}

/// Error saving a corpus artifact.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SaveError {
    /// IO error writing file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Postcard encoding error.
    #[error("encode error: {0}")]
    Encode(postcard::Error),
}

/// Error loading a corpus artifact.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// IO error reading file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// File exceeds the maximum allowed size.
    #[error("file too large: {0} bytes")]
    TooLarge(u64),
    /// Postcard decoding error.
    #[error("decode error: {0}")]
    Decode(postcard::Error),
    /// Corpus format version mismatch.
    #[error("incompatible corpus format: found {found}, expected {expected}")]
    IncompatibleFormat {
        /// Format version found on disk.
        found: u32,
        /// Expected format version.
        expected: u32,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;
    use repoask_core::investigation::SurfaceKind;
    use repoask_core::investigation::build_surface_ref;
    use repoask_core::types::{ExportInfo, IndexDocument, Symbol, SymbolKind};

    fn code_symbol(document: &IndexDocument) -> &Symbol {
        match document {
            IndexDocument::Code(symbol) => symbol,
            IndexDocument::Reexport(_) | IndexDocument::Doc(_) => {
                unreachable!("expected code symbol")
            }
        }
    }

    fn sample_corpus() -> InvestigationCorpus {
        InvestigationCorpus::new(vec![IndexDocument::Code(Symbol {
            name: "validateToken".to_owned(),
            kind: SymbolKind::Function,
            filepath: "src/auth.ts".to_owned(),
            start_line: 3,
            end_line: 8,
            params: vec!["token".to_owned()],
            signature_preview: Some("validateToken(token)".to_owned()),
            comment: None,
            export: ExportInfo::public_named(),
        })])
    }

    #[test]
    fn corpus_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corpus.bin");

        let original = sample_corpus();
        save_corpus(&original, &path).unwrap();
        let loaded = load_corpus(&path).unwrap();

        let original_symbol = code_symbol(&original.documents[0]);
        let loaded_symbol = code_symbol(&loaded.documents[0]);

        assert_eq!(
            build_surface_ref(
                SurfaceKind::Api,
                &original_symbol.filepath,
                &original_symbol.name,
                original_symbol.start_line,
            ),
            build_surface_ref(
                SurfaceKind::Api,
                &loaded_symbol.filepath,
                &loaded_symbol.name,
                loaded_symbol.start_line,
            )
        );
    }
}
