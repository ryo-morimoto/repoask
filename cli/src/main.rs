//! repoask CLI — search code and docs in any GitHub repository.

#![allow(
    clippy::print_stderr,
    reason = "CLI binary uses stderr for user-facing messages"
)]

use clap::{Parser, Subcommand};
use repoask_core::investigation::{HintAction, InvestigationOverview, OverviewBudget};
use repoask_core::types::{SearchDocumentType, SearchFilters, SearchResult};
use repoask_repo::{cache, repo, repo::ParseDiagnostics};

/// Search code and documentation in any GitHub repository.
#[derive(Parser)]
#[command(name = "repoask", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Commands {
    /// Search a repository's code and documentation.
    Search {
        /// Repository in `owner/repo` format (optionally `owner/repo@ref`).
        repo_spec: String,
        /// Search query (natural language).
        query: String,
        /// Maximum number of results.
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        /// Output format.
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,
        /// Restrict results to files under this directory. Repeat or use commas for multiple.
        #[arg(long = "dir", value_delimiter = ',')]
        dirs: Vec<String>,
        /// Restrict results to these file extensions. Repeat or use commas for multiple.
        #[arg(long = "ext", value_delimiter = ',')]
        exts: Vec<String>,
        /// Restrict results to code or documentation.
        #[arg(long = "type")]
        result_type: Option<SearchTypeArg>,
        /// Print parse diagnostics to stderr when rebuilding the index.
        #[arg(long)]
        verbose: bool,
    },
    /// Show public-surface-first entrypoints for investigation.
    Overview {
        /// Repository in `owner/repo` format (optionally `owner/repo@ref`).
        repo_spec: String,
        /// Output format.
        #[arg(short, long, default_value = "json")]
        format: OutputFormat,
        /// Print parse diagnostics to stderr when rebuilding the corpus.
        #[arg(long)]
        verbose: bool,
    },
    /// Trace impact of changes (call graphs, type dependencies, affected files).
    Trace {
        /// Repository in `owner/repo` format (optionally `owner/repo@ref`).
        repo_spec: String,
        /// File path or symbol to trace (e.g. `src/auth/session.ts#UserSession`).
        target: String,
    },
    /// Remove cached data.
    Cleanup {
        /// Specific repository to clean (`owner/repo`). Omit to clean all.
        repo_spec: Option<String>,
    },
}

/// Output format for search results.
#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    /// JSON lines (one JSON object per result).
    Json,
    /// Human-readable text.
    Text,
}

/// CLI value for `--type` search filtering.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum SearchTypeArg {
    /// Search only code symbols.
    Code,
    /// Search only documentation sections.
    Doc,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Search {
            repo_spec,
            query,
            limit,
            format,
            dirs,
            exts,
            result_type,
            verbose,
        } => run_search(
            &repo_spec,
            &query,
            limit,
            &format,
            &build_search_filters(&dirs, &exts, result_type),
            verbose,
        ),
        Commands::Overview {
            repo_spec,
            format,
            verbose,
        } => run_overview(&repo_spec, &format, verbose),
        Commands::Trace { .. } => {
            eprintln!("repoask trace is not yet implemented. Coming soon.");
            std::process::exit(1);
        }
        Commands::Cleanup { repo_spec } => run_cleanup(repo_spec.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run_search(
    repo_spec: &str,
    query: &str,
    limit: usize,
    format: &OutputFormat,
    filters: &SearchFilters,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    repoask_core::tokenizer::validate_query(query)?;
    let (results, parse_report) = if verbose {
        let output = repo::search_with_report_and_filters(repo_spec, query, limit, filters)?;
        (output.results, output.parse_diagnostics)
    } else {
        (
            repo::search_with_filters(repo_spec, query, limit, filters)?.results,
            None,
        )
    };

    match format {
        OutputFormat::Json => print_json(&results)?,
        OutputFormat::Text => print_text(&results),
    }

    if verbose {
        print_parse_report(parse_report.as_ref())?;
    }

    Ok(())
}

fn run_overview(
    repo_spec: &str,
    format: &OutputFormat,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let budget = OverviewBudget::default();
    let (overview, parse_report) = if verbose {
        let output = repo::overview_with_report(repo_spec, budget)?;
        (output.overview, output.parse_diagnostics)
    } else {
        (repo::overview(repo_spec, budget)?, None)
    };

    match format {
        OutputFormat::Json => print_overview_json(&overview)?,
        OutputFormat::Text => print_overview_text(&overview),
    }

    if verbose {
        print_parse_report(parse_report.as_ref())?;
    }

    Ok(())
}

fn print_parse_report(report: Option<&ParseDiagnostics>) -> std::io::Result<()> {
    use std::io::Write;

    let mut stderr = std::io::stderr().lock();
    for line in format_parse_report(report) {
        writeln!(stderr, "{line}")?;
    }
    Ok(())
}

fn format_parse_report(report: Option<&ParseDiagnostics>) -> Vec<String> {
    let Some(report) = report else {
        return vec!["verbose: reused cached index; parse report unavailable".to_owned()];
    };

    let mut lines = vec![format!(
        "verbose: parsed {} files (unsupported: {}, failed: {}, oversized: {})",
        report.parsed_count,
        report.unsupported.len(),
        report.failed.len(),
        report.oversized.len(),
    )];

    lines.extend(
        report
            .unsupported
            .iter()
            .map(|filepath| format!("unsupported: {filepath}")),
    );
    lines.extend(
        report
            .failed
            .iter()
            .map(|(filepath, reason)| format!("failed: {filepath} ({reason})")),
    );
    lines.extend(
        report
            .oversized
            .iter()
            .map(|filepath| format!("oversized: {filepath}")),
    );

    lines
}

fn build_search_filters(
    dirs: &[String],
    exts: &[String],
    result_type: Option<SearchTypeArg>,
) -> SearchFilters {
    SearchFilters {
        dirs: dirs
            .iter()
            .filter_map(|dir| normalize_dir_filter(dir))
            .collect(),
        exts: exts
            .iter()
            .filter_map(|ext| normalize_ext_filter(ext))
            .collect(),
        result_type: result_type.map(|value| match value {
            SearchTypeArg::Code => SearchDocumentType::Code,
            SearchTypeArg::Doc => SearchDocumentType::Doc,
        }),
    }
}

fn normalize_dir_filter(dir: &str) -> Option<String> {
    let normalized = dir.trim().replace('\\', "/");
    let normalized = normalized.trim_start_matches("./").trim_matches('/');
    (!normalized.is_empty()).then(|| normalized.to_owned())
}

fn normalize_ext_filter(ext: &str) -> Option<String> {
    let normalized = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn print_json(results: &[SearchResult]) -> Result<(), serde_json::Error> {
    use std::io::Write;

    for result in results {
        let json = serde_json::to_string(result)?;
        // Use write! to stdout to avoid print_stdout lint
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{json}");
    }
    Ok(())
}

fn print_overview_json(overview: &InvestigationOverview) -> Result<(), serde_json::Error> {
    use std::io::Write;

    let json = serde_json::to_string(overview)?;
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{json}");
    Ok(())
}

fn print_text(results: &[SearchResult]) {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();

    for result in results {
        match result {
            SearchResult::Code(r) => {
                let label = if r.is_example { "example" } else { "code" };
                let _ = writeln!(
                    stdout,
                    "[{label}] {file}:{start}-{end}  {kind:?} {name}  (score: {score:.3})",
                    file = r.filepath,
                    start = r.start_line,
                    end = r.end_line,
                    kind = r.kind,
                    name = r.name,
                    score = r.score,
                );
            }
            SearchResult::Doc(r) => {
                let snippet = r.snippet.chars().take(80).collect::<String>();
                let _ = writeln!(
                    stdout,
                    "[doc]  {file}#{section}  \"{snippet}...\"  (score: {score:.3})",
                    file = r.filepath,
                    section = r.section,
                    score = r.score,
                );
            }
        }
    }
}

fn print_overview_text(overview: &InvestigationOverview) {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "repo: {}", overview.meta.repo);
    let _ = writeln!(
        stdout,
        "coverage: publicness={:?}, comments={:?}, tests={:?}",
        overview.meta.coverage.publicness,
        overview.meta.coverage.comment_normalization,
        overview.meta.coverage.test_linkage,
    );
    if overview.meta.truncated {
        let _ = writeln!(stdout, "truncated: true");
    }
    for note in &overview.meta.coverage.notes {
        let _ = writeln!(stdout, "note: {note}");
    }

    let _ = writeln!(stdout, "public_api:");
    for api in &overview.public_apis {
        let _ = writeln!(stdout, "  {}  {}", api.signature, api.filepath);
        if let Some(summary) = &api.comment_summary {
            let _ = writeln!(stdout, "    doc: {summary}");
        }
        if !api.flags.is_empty() {
            let flags = api
                .flags
                .iter()
                .map(|flag| format!("{flag:?}").to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(stdout, "    flags: {flags}");
        }
        if !api.tests.is_empty() {
            let tests = api
                .tests
                .iter()
                .map(|test| test.display_name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(stdout, "    tests: {tests}");
        }
    }

    let _ = writeln!(stdout, "public_types:");
    for public_type in &overview.public_types {
        let _ = writeln!(
            stdout,
            "  {} ({:?})  {}",
            public_type.signature, public_type.kind, public_type.filepath,
        );
        if let Some(summary) = &public_type.comment_summary {
            let _ = writeln!(stdout, "    doc: {summary}");
        }
    }

    let _ = writeln!(stdout, "public_api_tests:");
    for test in &overview.public_api_tests {
        let _ = writeln!(stdout, "  {}  {}", test.display_name, test.filepath);
    }

    let _ = writeln!(stdout, "entry_hints:");
    for hint in &overview.entry_hints {
        let _ = writeln!(
            stdout,
            "  {} {} ({})",
            render_hint_action(hint.action),
            hint.label,
            hint.reason,
        );
    }
}

const fn render_hint_action(action: HintAction) -> &'static str {
    match action {
        HintAction::InspectSymbol => "inspect",
        HintAction::SearchQuery => "search",
        HintAction::ReadPath => "read",
    }
}

fn run_cleanup(repo_spec: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let mut stderr = std::io::stderr().lock();

    if let Some(spec) = repo_spec {
        let (owner, repo, _) = repo::parse_repo_spec(spec).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid repo spec: {spec}"),
            )
        })?;
        cache::cleanup_repo(owner, repo)?;
        let _ = writeln!(stderr, "cleaned up cache for {owner}/{repo}");
    } else {
        cache::cleanup_all()?;
        let _ = writeln!(stderr, "cleaned up all cached data");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn format_parse_report_for_cache_hit() {
        assert_eq!(
            format_parse_report(None),
            vec!["verbose: reused cached index; parse report unavailable"]
        );
    }

    #[test]
    fn format_parse_report_lists_sorted_entries() {
        let report = ParseDiagnostics {
            unsupported: vec!["docs/guide.txt".to_owned()],
            failed: vec![("src/app.ts".to_owned(), "parse error".to_owned())],
            oversized: vec!["fixtures/big.log".to_owned()],
            parsed_count: 3,
        };

        assert_eq!(
            format_parse_report(Some(&report)),
            vec![
                "verbose: parsed 3 files (unsupported: 1, failed: 1, oversized: 1)",
                "unsupported: docs/guide.txt",
                "failed: src/app.ts (parse error)",
                "oversized: fixtures/big.log",
            ]
        );
    }

    #[test]
    fn build_search_filters_normalizes_dir_and_ext_values() {
        let filters = build_search_filters(
            &[
                "./src/".to_owned(),
                "docs\\api".to_owned(),
                "   ".to_owned(),
            ],
            &[".RS".to_owned(), " md ".to_owned(), String::new()],
            Some(SearchTypeArg::Code),
        );

        assert_eq!(filters.dirs, vec!["src", "docs/api"]);
        assert_eq!(filters.exts, vec!["rs", "md"]);
        assert_eq!(filters.result_type, Some(SearchDocumentType::Code));
    }

    #[test]
    fn cli_parses_search_filters() {
        let cli = Cli::try_parse_from([
            "repoask",
            "search",
            "owner/repo",
            "query",
            "--dir",
            "src",
            "--ext",
            "ts,js",
            "--type",
            "code",
        ])
        .expect("search command should parse");

        let Commands::Search {
            dirs,
            exts,
            result_type,
            ..
        } = cli.command
        else {
            panic!("expected search command");
        };

        assert_eq!(dirs, vec!["src"]);
        assert_eq!(exts, vec!["ts", "js"]);
        assert!(matches!(result_type, Some(SearchTypeArg::Code)));
    }

    #[test]
    fn cli_parses_overview_command() {
        let cli = Cli::try_parse_from(["repoask", "overview", "owner/repo", "--format", "text"])
            .expect("overview command should parse");

        let Commands::Overview {
            repo_spec, format, ..
        } = cli.command
        else {
            panic!("expected overview command");
        };

        assert_eq!(repo_spec, "owner/repo");
        assert!(matches!(format, OutputFormat::Text));
    }
}
