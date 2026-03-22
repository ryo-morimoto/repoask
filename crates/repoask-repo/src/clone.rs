//! Git clone operations with shallow clone and atomic swap.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cache;

/// Error type for clone operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CloneError {
    /// Git command failed.
    #[error("git clone failed: {0}")]
    GitFailed(String),
    /// IO error during directory operations.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Ensure a shallow clone of the repository exists in the cache.
///
/// Returns the path to the cloned repository directory.
/// If the clone already exists, returns immediately.
pub fn ensure_clone(owner: &str, repo: &str, ref_spec: Option<&str>) -> Result<PathBuf, CloneError> {
    let repo_dir = cache::repo_clone_dir(owner, repo);

    if repo_dir.exists() {
        return Ok(repo_dir);
    }

    clone_fresh(owner, repo, ref_spec, &repo_dir)
}

/// Perform a fresh shallow clone with atomic swap.
fn clone_fresh(
    owner: &str,
    repo: &str,
    ref_spec: Option<&str>,
    target: &Path,
) -> Result<PathBuf, CloneError> {
    let tmp_dir = cache::cache_dir().join(format!(
        "tmp/{}-{}-{}",
        owner,
        repo,
        std::process::id()
    ));

    // Clean up any stale tmp dir
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir)?;
    }
    std::fs::create_dir_all(&tmp_dir)?;

    let url = format!("https://github.com/{owner}/{repo}.git");

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--depth", "1", "--single-branch"]);

    if let Some(r) = ref_spec {
        cmd.args(["--branch", r]);
    }

    cmd.arg(&url).arg(&tmp_dir);

    let output = cmd.output()?;

    if !output.status.success() {
        // Clean up tmp on failure
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloneError::GitFailed(stderr.to_string()));
    }

    // Atomic move into place
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&tmp_dir, target)?;

    Ok(target.to_path_buf())
}

/// Get the current HEAD commit hash of a cloned repo.
pub fn head_commit(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
