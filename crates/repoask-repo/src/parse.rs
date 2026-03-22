//! Directory parsing: walk files and dispatch to the appropriate parser.

use std::path::Path;

use ignore::WalkBuilder;
use repoask_core::types::IndexDocument;

/// Parse all supported files in a directory and return index documents.
///
/// Uses `repoask-parser` for TS/JS and Markdown, and `repoask-treesitter`
/// for Rust, Python, Go, Java, C, C++, Ruby.
pub fn parse_directory(root: &Path) -> Vec<IndexDocument> {
    let mut documents = Vec::new();

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
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Try repoask-parser first (oxc + markdown)
        let docs = repoask_parser::parse_file(&relative_path, &source);
        if !docs.is_empty() {
            documents.extend(docs);
            continue;
        }

        // Then try repoask-treesitter
        if let Some(docs) = repoask_treesitter::parse_file(&relative_path, &source) {
            documents.extend(docs);
        }
    }

    documents
}
