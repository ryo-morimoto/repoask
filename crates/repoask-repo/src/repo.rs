//! High-level repository operations: clone → index → search.

use fs2::FileExt;
use repoask_core::index::InvertedIndex;
use repoask_core::types::SearchResult;

use crate::cache;
use crate::clone;
use crate::index_store;

/// Error type for repository operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RepoError {
    /// Clone failed.
    #[error("clone: {0}")]
    Clone(#[from] clone::CloneError),
    /// Index save/load failed.
    #[error("index save: {0}")]
    IndexSave(#[from] index_store::SaveError),
    /// Index load failed.
    #[error("index load: {0}")]
    IndexLoad(#[from] index_store::LoadError),
    /// IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Invalid repository specification.
    #[error("invalid repo spec: {spec} (expected owner/repo)")]
    InvalidSpec {
        /// The invalid spec string.
        spec: String,
    },
}

/// Parse an `owner/repo` spec, optionally with `@ref`.
///
/// Examples:
/// - `"vercel/next.js"` → `("vercel", "next.js", None)`
/// - `"vercel/next.js@v14"` → `("vercel", "next.js", Some("v14"))`
pub fn parse_repo_spec(spec: &str) -> Option<(&str, &str, Option<&str>)> {
    let (main, ref_spec) = match spec.split_once('@') {
        Some((main, r)) => (main, Some(r)),
        None => (spec, None),
    };

    let (owner, repo) = main.split_once('/')?;

    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some((owner, repo, ref_spec))
}

/// Search a repository. Handles clone, indexing, caching, and search.
pub fn search(spec: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>, RepoError> {
    let (owner, repo, ref_spec) = parse_repo_spec(spec).ok_or_else(|| RepoError::InvalidSpec {
        spec: spec.to_owned(),
    })?;

    // Acquire advisory lock
    let lock_path = cache::repo_lock_path(owner, repo);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    lock_file.lock_exclusive()?;

    // Try loading cached index (lock released on drop)
    let index = load_or_build_index(owner, repo, ref_spec)?;
    drop(lock_file);

    // Best-effort cache eviction (non-fatal)
    let _ = cache::evict_if_needed();

    Ok(index.search(query, limit))
}

/// Load a cached index if valid, otherwise build a new one.
fn load_or_build_index(
    owner: &str,
    repo: &str,
    ref_spec: Option<&str>,
) -> Result<InvertedIndex, RepoError> {
    let index_path = cache::repo_index_path(owner, repo);
    let meta_path = cache::repo_meta_path(owner, repo);

    // Check if we have a valid cached index
    if index_path.exists() && meta_path.exists() {
        if let Ok(meta) = index_store::load_meta(&meta_path) {
            if meta.is_compatible() {
                // Check if the clone still matches
                let clone_dir = cache::repo_clone_dir(owner, repo);
                if clone_dir.exists()
                    && let Some(current_hash) = clone::head_commit(&clone_dir)
                    && meta.matches_commit(&current_hash)
                {
                    return Ok(index_store::load_index(&index_path)?);
                }
            }
        }
    }

    // Need to build index: ensure clone exists, parse, build, save
    let clone_dir = clone::ensure_clone(owner, repo, ref_spec)?;

    let (documents, _report) = crate::parse::parse_directory(&clone_dir);
    let index = InvertedIndex::build(&documents);

    // Save index and metadata
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    index_store::save_index(&index, &index_path)?;

    let commit_hash = clone::head_commit(&clone_dir).unwrap_or_default();
    let meta = index_store::IndexMeta::new(commit_hash);
    index_store::save_meta(&meta, &meta_path)?;

    Ok(index)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_spec_basic() {
        let (owner, repo, ref_spec) = parse_repo_spec("vercel/next.js").unwrap();
        assert_eq!(owner, "vercel");
        assert_eq!(repo, "next.js");
        assert!(ref_spec.is_none());
    }

    #[test]
    fn test_parse_repo_spec_with_ref() {
        let (owner, repo, ref_spec) = parse_repo_spec("vercel/next.js@v14.0.0").unwrap();
        assert_eq!(owner, "vercel");
        assert_eq!(repo, "next.js");
        assert_eq!(ref_spec, Some("v14.0.0"));
    }

    #[test]
    fn test_parse_repo_spec_with_branch() {
        let (_, _, ref_spec) = parse_repo_spec("owner/repo@main").unwrap();
        assert_eq!(ref_spec, Some("main"));
    }

    #[test]
    fn test_parse_repo_spec_invalid() {
        assert!(parse_repo_spec("no-slash").is_none());
        assert!(parse_repo_spec("/repo").is_none());
        assert!(parse_repo_spec("owner/").is_none());
    }
}
