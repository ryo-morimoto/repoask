//! repoask CLI — search code and docs in any GitHub repository.

use clap::{Parser, Subcommand};
use repoask_core::types::SearchResult;
use repoask_repo::{cache, repo};

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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Search {
            repo_spec,
            query,
            limit,
            format,
        } => {
            if let Err(e) = run_search(&repo_spec, &query, limit, &format) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Cleanup { repo_spec } => {
            if let Err(e) = run_cleanup(repo_spec.as_deref()) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn run_search(
    repo_spec: &str,
    query: &str,
    limit: usize,
    format: &OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let results = repo::search(repo_spec, query, limit)?;

    match format {
        OutputFormat::Json => print_json(&results)?,
        OutputFormat::Text => print_text(&results),
    }

    Ok(())
}

fn print_json(results: &[SearchResult]) -> Result<(), serde_json::Error> {
    for result in results {
        let json = serde_json::to_string(result)?;
        // Use write! to stdout to avoid print_stdout lint
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{json}");
    }
    Ok(())
}

fn print_text(results: &[SearchResult]) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();

    for result in results {
        match result {
            SearchResult::Code(r) => {
                let _ = writeln!(
                    stdout,
                    "[code] {file}:{start}-{end}  {kind:?} {name}  (score: {score:.3})",
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
            SearchResult::Example(r) => {
                let _ = writeln!(
                    stdout,
                    "[example] {file}:{start}-{end}  {kind:?} {name}  (score: {score:.3})",
                    file = r.filepath,
                    start = r.start_line,
                    end = r.end_line,
                    kind = r.kind,
                    name = r.name,
                    score = r.score,
                );
            }
        }
    }
}

fn run_cleanup(repo_spec: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut stderr = std::io::stderr().lock();

    match repo_spec {
        Some(spec) => {
            let (owner, repo, _) = repo::parse_repo_spec(spec).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid repo spec: {spec}"),
                )
            })?;
            cache::cleanup_repo(owner, repo)?;
            let _ = writeln!(stderr, "cleaned up cache for {owner}/{repo}");
        }
        None => {
            cache::cleanup_all()?;
            let _ = writeln!(stderr, "cleaned up all cached data");
        }
    }
    Ok(())
}
