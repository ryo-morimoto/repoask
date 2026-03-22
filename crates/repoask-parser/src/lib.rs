pub mod language;
pub mod markdown;
pub mod oxc;
pub mod tree_sitter_parser;

use std::path::Path;

use ignore::WalkBuilder;
use repoask_core::types::IndexDocument;

use crate::language::{parser_for_extension, ParserKind};

/// Parse all supported files in a directory and return index documents.
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
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        let parser_kind = match parser_for_extension(ext) {
            Some(k) => k,
            None => continue,
        };

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue, // skip binary/unreadable files
        };

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        match parser_kind {
            ParserKind::Oxc => {
                let symbols = crate::oxc::extract_ts_symbols(&source, &relative_path);
                documents.extend(symbols.into_iter().map(IndexDocument::Code));
            }
            ParserKind::TreeSitter { language, query } => {
                let symbols = crate::tree_sitter_parser::extract_symbols(
                    &source,
                    &relative_path,
                    &language,
                    query,
                );
                documents.extend(symbols.into_iter().map(IndexDocument::Code));
            }
            ParserKind::Markdown => {
                let sections = crate::markdown::parse_markdown(&source, &relative_path);
                documents.extend(sections.into_iter().map(IndexDocument::Doc));
            }
        }
    }

    documents
}
