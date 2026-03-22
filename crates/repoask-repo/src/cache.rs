//! Cache directory management following XDG Base Directory Specification.

use std::path::PathBuf;

/// Returns the root cache directory for repoask.
///
/// Priority:
/// 1. `$REPOASK_CACHE_DIR` (explicit override)
/// 2. `$XDG_CACHE_HOME/repoask`
/// 3. `~/.cache/repoask`
/// 4. `/tmp/repoask` (fallback)
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("REPOASK_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("repoask");
    }
    dirs::cache_dir()
        .map(|d| d.join("repoask"))
        .unwrap_or_else(|| PathBuf::from("/tmp/repoask"))
}

/// Returns the directory for a specific repository's data.
///
/// Layout: `<cache_dir>/repos/github.com/<owner>/<repo>/`
///
/// # Panics
///
/// Panics if `owner` or `repo` contain path traversal components (e.g. `..`, `/`).
pub fn repo_cache_dir(owner: &str, repo: &str) -> PathBuf {
    assert!(
        is_safe_path_component(owner) && is_safe_path_component(repo),
        "owner/repo must not contain path separators or `..`: owner={owner:?}, repo={repo:?}"
    );
    cache_dir().join("repos/github.com").join(owner).join(repo)
}

/// Check that a string is safe to use as a single path component.
fn is_safe_path_component(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains('/') && !s.contains('\\')
}

/// Returns the path for the cloned repository.
pub fn repo_clone_dir(owner: &str, repo: &str) -> PathBuf {
    repo_cache_dir(owner, repo).join("repo")
}

/// Returns the path for the serialized index.
pub fn repo_index_path(owner: &str, repo: &str) -> PathBuf {
    repo_cache_dir(owner, repo).join("index.bin")
}

/// Returns the path for the index metadata.
pub fn repo_meta_path(owner: &str, repo: &str) -> PathBuf {
    repo_cache_dir(owner, repo).join("index.meta.json")
}

/// Returns the path for the lock file.
pub fn repo_lock_path(owner: &str, repo: &str) -> PathBuf {
    repo_cache_dir(owner, repo).join(".lock")
}

/// Remove all cached data.
///
/// Refuses to delete if the cache directory path does not contain "repoask"
/// as a safety guard against misconfigured `REPOASK_CACHE_DIR`.
pub fn cleanup_all() -> std::io::Result<()> {
    let dir = cache_dir();
    if !dir.to_string_lossy().contains("repoask") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "refusing to delete cache directory that does not contain \"repoask\": {}",
                dir.display()
            ),
        ));
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Remove cached data for a specific repository.
pub fn cleanup_repo(owner: &str, repo: &str) -> std::io::Result<()> {
    let dir = repo_cache_dir(owner, repo);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
