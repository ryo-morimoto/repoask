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
    dirs::cache_dir().map_or_else(|| PathBuf::from("/tmp/repoask"), |d| d.join("repoask"))
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

/// Maximum total cache size in bytes (2 GB).
const MAX_CACHE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Evict oldest repository caches until total size is under the limit.
///
/// Walks `<cache_dir>/repos/github.com/<owner>/<repo>/` directories,
/// sorts by modification time (oldest first), and removes repos until
/// the total size drops below `MAX_CACHE_SIZE`.
pub fn evict_if_needed() -> std::io::Result<()> {
    let repos_dir = cache_dir().join("repos/github.com");
    if !repos_dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut total_size: u64 = 0;

    // Walk owner/repo directories
    for owner_entry in std::fs::read_dir(&repos_dir)? {
        let owner_entry = owner_entry?;
        if !owner_entry.file_type()?.is_dir() {
            continue;
        }
        for repo_entry in std::fs::read_dir(owner_entry.path())? {
            let repo_entry = repo_entry?;
            if !repo_entry.file_type()?.is_dir() {
                continue;
            }
            let repo_path = repo_entry.path();
            let size = dir_size(&repo_path)?;
            let mtime = repo_entry
                .metadata()?
                .modified()
                .unwrap_or(std::time::UNIX_EPOCH);
            total_size += size;
            entries.push((repo_path, size, mtime));
        }
    }

    if total_size <= MAX_CACHE_SIZE {
        return Ok(());
    }

    // Sort by mtime ascending (oldest first)
    entries.sort_by_key(|(_, _, mtime)| *mtime);

    for (path, size, _) in &entries {
        if total_size <= MAX_CACHE_SIZE {
            break;
        }
        let _ = std::fs::remove_dir_all(path);
        total_size = total_size.saturating_sub(*size);
    }

    Ok(())
}

/// Calculate total size of a directory recursively.
fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_file() {
            total += entry.metadata()?.len();
        } else if ft.is_dir() {
            total += dir_size(&entry.path())?;
        }
    }
    Ok(total)
}
