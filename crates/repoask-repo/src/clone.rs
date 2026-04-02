//! Git clone operations with shallow clone and atomic swap.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cache;

/// Error type for clone operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CloneError {
    /// Git command failed.
    #[error("git clone failed: {0}")]
    GitFailed(String),
    /// Invalid repository specification (owner, repo, or `ref_spec`).
    #[error("invalid repository spec: {0}")]
    InvalidSpec(String),
    /// IO error during directory operations.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Validate that an owner or repo name contains only safe characters.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// Ensure a shallow clone of the repository exists in the cache.
///
/// Returns the path to the cloned repository directory.
/// If the clone already exists, returns immediately.
pub fn ensure_clone(
    owner: &str,
    repo: &str,
    ref_spec: Option<&str>,
) -> Result<PathBuf, CloneError> {
    if !is_valid_name(owner) {
        return Err(CloneError::InvalidSpec(format!("invalid owner: {owner:?}")));
    }
    if !is_valid_name(repo) {
        return Err(CloneError::InvalidSpec(format!("invalid repo: {repo:?}")));
    }
    if let Some(r) = ref_spec {
        if r.starts_with('-') {
            return Err(CloneError::InvalidSpec(format!("invalid ref_spec: {r:?}")));
        }
    }

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
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_dir = cache::cache_dir().join(format!("tmp/{owner}-{repo}-{unique}"));

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

    cmd.arg(&url)
        .arg(&tmp_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let output = cmd.output()?;

    if !output.status.success() {
        // Clean up tmp on failure
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloneError::GitFailed(stderr.to_string()));
    }

    // Move into place — try atomic rename first, fall back to recursive copy
    // when source and destination are on different filesystems (EXDEV).
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(&tmp_dir, target) {
        Ok(()) => {}
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            // Cross-filesystem: fall back to recursive copy
            copy_dir_recursive(&tmp_dir, target).inspect_err(|_copy_err| {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                let _ = std::fs::remove_dir_all(target);
            })?;
            let _ = std::fs::remove_dir_all(&tmp_dir);
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err(e.into());
        }
    }

    Ok(target.to_path_buf())
}

/// Recursively copy a directory tree. Used as fallback when rename fails
/// due to cross-filesystem boundaries (EXDEV).
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Get the current HEAD commit hash of a cloned repo.
pub fn head_commit(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}
