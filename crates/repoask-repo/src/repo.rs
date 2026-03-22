//! High-level repository operations: clone → index → search.

use std::path::Path;

use fs2::FileExt;
use repoask_core::index::InvertedIndex;
use repoask_core::types::SearchResult;

use crate::cache;
use crate::clone;
use crate::index_store;

/// Error type for repository operations.
#[derive(Debug)]
pub enum RepoError {
    /// Clone failed.
    Clone(clone::CloneError),
    /// Index save/load failed.
    IndexSave(index_store::SaveError),
    /// Index load failed.
    IndexLoad(index_store::LoadError),
    /// IO error.
    Io(std::io::Error),
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clone(e) => write!(f, "clone: {e}"),
            Self::IndexSave(e) => write!(f, "index save: {e}"),
            Self::IndexLoad(e) => write!(f, "index load: {e}"),
            Self::Io(e) => write!(f, "IO: {e}"),
        }
    }
}

impl std::error::Error for RepoError {}

impl From<clone::CloneError> for RepoError {
    fn from(e: clone::CloneError) -> Self {
        Self::Clone(e)
    }
}

impl From<index_store::SaveError> for RepoError {
    fn from(e: index_store::SaveError) -> Self {
        Self::IndexSave(e)
    }
}

impl From<index_store::LoadError> for RepoError {
    fn from(e: index_store::LoadError) -> Self {
        Self::IndexLoad(e)
    }
}

impl From<std::io::Error> for RepoError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
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
    let (owner, repo, ref_spec) = parse_repo_spec(spec)
        .ok_or_else(|| RepoError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid repo spec: {spec} (expected owner/repo)"),
        )))?;

    // Acquire advisory lock
    let lock_path = cache::repo_lock_path(owner, repo);
    std::fs::create_dir_all(lock_path.parent().unwrap_or(Path::new("")))?;
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    lock_file.lock_exclusive()?;

    // Try loading cached index
    let index = load_or_build_index(owner, repo, ref_spec)?;

    // Unlock (dropped automatically, but explicit for clarity)
    let _ = lock_file.unlock();

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
                if clone_dir.exists() {
                    if let Some(current_hash) = clone::head_commit(&clone_dir) {
                        if meta.matches_commit(&current_hash) {
                            return Ok(index_store::load_index(&index_path)?);
                        }
                    }
                }
            }
        }
    }

    // Need to build index: ensure clone exists, parse, build, save
    let clone_dir = clone::ensure_clone(owner, repo, ref_spec)?;

    let documents = repoask_parser::parse_directory(&clone_dir);
    let index = InvertedIndex::build(documents);

    // Save index and metadata
    std::fs::create_dir_all(index_path.parent().unwrap_or(Path::new("")))?;
    index_store::save_index(&index, &index_path)?;

    let commit_hash = clone::head_commit(&clone_dir).unwrap_or_default();
    let meta = index_store::IndexMeta::new(commit_hash);
    index_store::save_meta(&meta, &meta_path)?;

    Ok(index)
}

#[cfg(test)]
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
