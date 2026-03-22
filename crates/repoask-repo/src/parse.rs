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

/// Parse all supported files in a directory and return index documents.
///
/// Uses `repoask-parser` for TS/JS and Markdown, and `repoask-treesitter`
/// for Rust, Python, Go, Java, C, C++, Ruby.
///
/// Returns both the documents and a report of what was skipped/failed.
pub fn parse_directory(root: &Path) -> (Vec<IndexDocument>, ParseReport) {
    let mut documents = Vec::new();
    let mut report = ParseReport::default();

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();

        // Skip files exceeding the size limit to avoid OOM on generated/bundled files.
        if let Ok(meta) = path.metadata() {
            if meta.len() > MAX_FILE_SIZE {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or_else(|_| path)
                    .to_string_lossy()
                    .to_string();
                report.oversized.push(rel);
                continue;
            }
        }

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or_else(|_| path)
            .to_string_lossy()
            .to_string();

        // Try repoask-parser first (oxc + markdown)
        match repoask_parser::parse_file(&relative_path, &source) {
            repoask_parser::ParseOutcome::Ok(docs) => {
                report.parsed_count += 1;
                documents.extend(docs);
                continue;
            }
            repoask_parser::ParseOutcome::Failed { filepath, reason } => {
                report.failed.push((filepath, reason));
                continue;
            }
            repoask_parser::ParseOutcome::Unsupported { .. } => {
                // Fall through to tree-sitter
            }
        }

        // Then try repoask-treesitter
        match repoask_treesitter::parse_file(&relative_path, &source) {
            repoask_treesitter::ParseOutcome::Ok(docs) => {
                report.parsed_count += 1;
                documents.extend(docs);
            }
            repoask_treesitter::ParseOutcome::Failed { filepath, reason } => {
                report.failed.push((filepath, reason));
            }
            repoask_treesitter::ParseOutcome::Unsupported { filepath, .. } => {
                report.unsupported.push(filepath);
            }
        }
    }

    (documents, report)
}
