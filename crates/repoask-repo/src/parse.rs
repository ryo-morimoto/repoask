//! Directory parsing: walk files and dispatch to the appropriate parser.

use std::path::Path;

use ignore::WalkBuilder;
use repoask_core::types::IndexDocument;

/// Maximum file size to parse (10 MB). Larger files are skipped.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Summary of skipped or failed files during directory parsing.
#[derive(Debug, Default)]
pub struct ParseReport {
    /// Files whose extension is not supported by any parser.
    pub unsupported: Vec<String>,
    /// Files that a parser attempted but failed to extract from.
    pub failed: Vec<(String, String)>,
    /// Files skipped because they exceeded the size limit.
    pub oversized: Vec<String>,
    /// Total files successfully parsed.
    pub parsed_count: usize,
}

/// Result of parsing a single file, sent from worker threads to the collector.
enum FileResult {
    Parsed(Vec<IndexDocument>),
    Failed(String, String),
    Unsupported(String),
    Oversized(String),
}

/// Parse all supported files in a directory and return index documents.
///
/// Uses `repoask-parser` for TS/JS and Markdown, and `repoask-treesitter`
/// for Rust, Python, Go, Java, C, C++, Ruby.
///
/// File walking and parsing are parallelized using `ignore`'s built-in
/// parallel walker and `crossbeam-channel`.
///
/// Returns both the documents and a report of what was skipped/failed.
pub fn parse_directory(root: &Path) -> (Vec<IndexDocument>, ParseReport) {
    let (tx, rx) = crossbeam_channel::unbounded::<FileResult>();
    let root_owned = root.to_path_buf();

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build_parallel();

    walker.run(|| {
        let tx = tx.clone();
        let root = root_owned.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();
            let relative_path = path
                .strip_prefix(&root)
                .unwrap_or_else(|_| path)
                .to_string_lossy()
                .to_string();

            // Skip oversized files
            if let Ok(meta) = path.metadata() {
                if meta.len() > MAX_FILE_SIZE {
                    let _ = tx.send(FileResult::Oversized(relative_path));
                    return ignore::WalkState::Continue;
                }
            }

            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(_) => return ignore::WalkState::Continue,
            };

            // Try repoask-parser first (oxc + markdown)
            match repoask_parser::parse_file(&relative_path, &source) {
                repoask_parser::ParseOutcome::Ok(docs) => {
                    let _ = tx.send(FileResult::Parsed(docs));
                    return ignore::WalkState::Continue;
                }
                repoask_parser::ParseOutcome::Failed { filepath, reason } => {
                    let _ = tx.send(FileResult::Failed(filepath, reason));
                    return ignore::WalkState::Continue;
                }
                repoask_parser::ParseOutcome::Unsupported { .. } => {
                    // Fall through to tree-sitter
                }
            }

            // Then try repoask-treesitter
            match repoask_treesitter::parse_file(&relative_path, &source) {
                repoask_treesitter::ParseOutcome::Ok(docs) => {
                    let _ = tx.send(FileResult::Parsed(docs));
                }
                repoask_treesitter::ParseOutcome::Failed { filepath, reason } => {
                    let _ = tx.send(FileResult::Failed(filepath, reason));
                }
                repoask_treesitter::ParseOutcome::Unsupported { filepath, .. } => {
                    let _ = tx.send(FileResult::Unsupported(filepath));
                }
            }

            ignore::WalkState::Continue
        })
    });

    // Drop the sender so rx.iter() terminates
    drop(tx);

    // Collect results
    let mut documents = Vec::new();
    let mut report = ParseReport::default();

    for result in rx {
        match result {
            FileResult::Parsed(docs) => {
                report.parsed_count += 1;
                documents.extend(docs);
            }
            FileResult::Failed(filepath, reason) => {
                report.failed.push((filepath, reason));
            }
            FileResult::Unsupported(filepath) => {
                report.unsupported.push(filepath);
            }
            FileResult::Oversized(filepath) => {
                report.oversized.push(filepath);
            }
        }
    }

    (documents, report)
}
