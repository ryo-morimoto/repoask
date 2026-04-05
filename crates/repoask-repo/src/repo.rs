//! High-level repository operations: clone → index → search.

use fs2::FileExt;
use repoask_core::index::InvertedIndex;
use repoask_core::investigation::{
    InvestigationCorpus, InvestigationOverview, OverviewBudget, build_overview,
};
use repoask_core::types::{SearchFilters, SearchResult};

use crate::cache;
use crate::clone;
use crate::index_store;
use crate::investigation_store;
use crate::module_resolution;
pub use crate::parse::ParseReport as ParseDiagnostics;

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
    /// Corpus save failed.
    #[error("corpus save: {0}")]
    CorpusSave(#[from] investigation_store::SaveError),
    /// Corpus load failed.
    #[error("corpus load: {0}")]
    CorpusLoad(#[from] investigation_store::LoadError),
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

/// Search output including optional parse diagnostics from a rebuilt index.
pub struct SearchOutput {
    /// Ranked search results.
    pub results: Vec<SearchResult>,
    /// Parse diagnostics when the index was rebuilt during this search.
    ///
    /// `None` indicates a cache hit, so no parse step ran.
    pub parse_diagnostics: Option<ParseDiagnostics>,
}

/// Overview output including optional parse diagnostics from a rebuilt corpus.
pub struct OverviewOutput {
    /// Structured overview response.
    pub overview: InvestigationOverview,
    /// Parse diagnostics when the corpus was rebuilt during this overview request.
    pub parse_diagnostics: Option<ParseDiagnostics>,
}

struct LoadedIndex {
    index: InvertedIndex,
    parse_report: Option<ParseDiagnostics>,
}

struct LoadedCorpus {
    corpus: InvestigationCorpus,
    parse_report: Option<ParseDiagnostics>,
}

/// Parse an `owner/repo` spec, optionally with `@ref`.
///
/// Examples:
/// - `"vercel/next.js"` → `("vercel", "next.js", None)`
/// - `"vercel/next.js@v14"` → `("vercel", "next.js", Some("v14"))`
#[must_use]
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
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or index files cannot be written.
pub fn search(spec: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>, RepoError> {
    Ok(search_with_filters(spec, query, limit, &SearchFilters::default())?.results)
}

/// Search a repository and apply optional result filters.
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or index files cannot be written.
pub fn search_with_filters(
    spec: &str,
    query: &str,
    limit: usize,
    filters: &SearchFilters,
) -> Result<SearchOutput, RepoError> {
    search_with_report_and_filters(spec, query, limit, filters)
}

/// Search a repository and include parse diagnostics when rebuilding the index.
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or index files cannot be written.
pub fn search_with_report(
    spec: &str,
    query: &str,
    limit: usize,
) -> Result<SearchOutput, RepoError> {
    search_with_report_and_filters(spec, query, limit, &SearchFilters::default())
}

/// Build an investigation overview for a repository.
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or corpus files cannot be written.
pub fn overview(spec: &str, budget: OverviewBudget) -> Result<InvestigationOverview, RepoError> {
    Ok(overview_with_report(spec, budget)?.overview)
}

/// Build an investigation overview and include parse diagnostics when rebuilding the corpus.
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or corpus files cannot be written.
pub fn overview_with_report(
    spec: &str,
    budget: OverviewBudget,
) -> Result<OverviewOutput, RepoError> {
    let (owner, repo, ref_spec, lock_file) = open_repo_lock(spec)?;

    let output = {
        let loaded = load_or_build_corpus(&owner, &repo, ref_spec.as_deref())?;
        OverviewOutput {
            overview: build_overview(&loaded.corpus, spec, budget),
            parse_diagnostics: loaded.parse_report,
        }
    };
    drop(lock_file);

    let _ = cache::evict_if_needed();

    Ok(output)
}

/// Search a repository, apply optional result filters, and include parse diagnostics when
/// rebuilding the index.
///
/// # Errors
///
/// Returns an error if the repository spec is invalid, cloning fails, cached data cannot be
/// loaded, or index files cannot be written.
pub fn search_with_report_and_filters(
    spec: &str,
    query: &str,
    limit: usize,
    filters: &SearchFilters,
) -> Result<SearchOutput, RepoError> {
    let (owner, repo, ref_spec, lock_file) = open_repo_lock(spec)?;

    let loaded = load_or_build_index(&owner, &repo, ref_spec.as_deref())?;
    let results = loaded.index.search_with_filters(query, limit, filters);
    let parse_diagnostics = loaded.parse_report;
    drop(lock_file);

    let _ = cache::evict_if_needed();

    Ok(SearchOutput {
        results,
        parse_diagnostics,
    })
}

fn open_repo_lock(
    spec: &str,
) -> Result<(String, String, Option<String>, std::fs::File), RepoError> {
    let (owner, repo, ref_spec) = parse_repo_spec(spec).ok_or_else(|| RepoError::InvalidSpec {
        spec: spec.to_owned(),
    })?;

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

    Ok((
        owner.to_owned(),
        repo.to_owned(),
        ref_spec.map(str::to_owned),
        lock_file,
    ))
}

/// Load a cached index if valid, otherwise build a new one.
fn load_or_build_index(
    owner: &str,
    repo: &str,
    ref_spec: Option<&str>,
) -> Result<LoadedIndex, RepoError> {
    let index_path = cache::repo_index_path(owner, repo);
    let meta_path = cache::repo_index_meta_path(owner, repo);

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
                    if let Ok(index) = index_store::load_index(&index_path) {
                        return Ok(LoadedIndex {
                            index,
                            parse_report: None,
                        });
                    }
                }
            }
        }
    }

    // Need to build index: reuse the investigation corpus as the canonical parsed artifact.
    let loaded_corpus = load_or_build_corpus(owner, repo, ref_spec)?;
    let clone_dir = cache::repo_clone_dir(owner, repo);
    let index = InvertedIndex::build(&loaded_corpus.corpus.documents);

    // Save index and metadata
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    index_store::save_index(&index, &index_path)?;

    let commit_hash = clone::head_commit(&clone_dir).unwrap_or_default();
    let meta = index_store::IndexMeta::new(commit_hash);
    index_store::save_meta(&meta, &meta_path)?;

    Ok(LoadedIndex {
        index,
        parse_report: loaded_corpus.parse_report,
    })
}

/// Load a cached investigation corpus if valid, otherwise build a new one.
fn load_or_build_corpus(
    owner: &str,
    repo: &str,
    ref_spec: Option<&str>,
) -> Result<LoadedCorpus, RepoError> {
    let corpus_path = cache::repo_corpus_path(owner, repo);
    let meta_path = cache::repo_corpus_meta_path(owner, repo);

    if corpus_path.exists() && meta_path.exists() {
        if let Ok(meta) = index_store::load_meta(&meta_path) {
            if meta.is_compatible() {
                let clone_dir = cache::repo_clone_dir(owner, repo);
                if clone_dir.exists()
                    && let Some(current_hash) = clone::head_commit(&clone_dir)
                    && meta.matches_commit(&current_hash)
                {
                    if let Ok(corpus) = investigation_store::load_corpus(&corpus_path) {
                        return Ok(LoadedCorpus {
                            corpus,
                            parse_report: None,
                        });
                    }
                }
            }
        }
    }

    let clone_dir = clone::ensure_clone(owner, repo, ref_spec)?;
    let (documents, report) = crate::parse::parse_directory(&clone_dir);
    let module_resolution = module_resolution::read_module_resolution(&clone_dir);
    let corpus = InvestigationCorpus::with_module_resolution(documents, module_resolution);

    if let Some(parent) = corpus_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    investigation_store::save_corpus(&corpus, &corpus_path)?;

    let commit_hash = clone::head_commit(&clone_dir).unwrap_or_default();
    let meta = index_store::IndexMeta::new(commit_hash);
    index_store::save_meta(&meta, &meta_path)?;

    Ok(LoadedCorpus {
        corpus,
        parse_report: Some(report),
    })
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
