//! Source code and documentation parsing for repoask.
//!
//! Handles TS/JS (via oxc) and Markdown. Pure Rust, WASM-compatible.
//! For tree-sitter based languages (Rust, Python, Go, etc.), see `repoask-treesitter`.

pub mod markdown;
pub mod oxc;

use repoask_core::types::IndexDocument;

/// Parse a single file given its path and source content.
///
/// Returns extracted index documents (code symbols or doc sections).
/// Returns an empty vec if the file extension is not supported.
pub fn parse_file(filepath: &str, source: &str) -> Vec<IndexDocument> {
    let ext = match filepath.rsplit('.').next() {
        Some(e) => e,
        None => return vec![],
    };

    match ext {
        "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" => {
            let symbols = oxc::extract_ts_symbols(source, filepath);
            symbols.into_iter().map(IndexDocument::Code).collect()
        }
        "md" | "mdx" => {
            let sections = markdown::parse_markdown(source, filepath);
            sections.into_iter().map(IndexDocument::Doc).collect()
        }
        _ => vec![],
    }
}
