//! Source code and documentation parsing for repoask.
//!
//! Handles TS/JS (via oxc) and Markdown. Pure Rust, WASM-compatible.
//! For tree-sitter based languages (Rust, Python, Go, etc.), see `repoask-treesitter`.

pub mod markdown;
pub mod oxc;

use repoask_core::types::IndexDocument;
pub use repoask_core::types::ParseOutcome;

/// Error type for parse operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The file extension is not supported.
    #[error("unsupported file extension: {filepath}")]
    UnsupportedExtension {
        /// The file path.
        filepath: String,
    },
    /// The parser failed to parse the file.
    #[error("parse failed for {filepath}: {reason}")]
    ParseFailed {
        /// The file path.
        filepath: String,
        /// The reason for the failure.
        reason: String,
    },
}

/// Parse a single file given its path and source content.
///
/// Returns a [`ParseOutcome`] that distinguishes between success,
/// unsupported file types, and parse failures.
pub fn parse_file(filepath: &str, source: &str) -> ParseOutcome {
    let ext = match filepath.rsplit('.').next() {
        Some(e) => e,
        None => {
            return ParseOutcome::Unsupported {
                filepath: filepath.to_owned(),
                extension: None,
            };
        }
    };

    match ext {
        "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" => {
            let symbols = oxc::extract_ts_symbols(source, filepath);
            if symbols.is_empty() && !source.trim().is_empty() {
                ParseOutcome::Failed {
                    filepath: filepath.to_owned(),
                    reason: "oxc parser returned no symbols (possible parse error)".to_owned(),
                }
            } else {
                ParseOutcome::Ok(symbols.into_iter().map(IndexDocument::Code).collect())
            }
        }
        "md" | "mdx" => {
            let sections = markdown::parse_markdown(source, filepath);
            ParseOutcome::Ok(sections.into_iter().map(IndexDocument::Doc).collect())
        }
        _ => ParseOutcome::Unsupported {
            filepath: filepath.to_owned(),
            extension: Some(ext.to_owned()),
        },
    }
}

/// Parse a single file, returning only the documents (ignoring skips/failures).
///
/// Convenience wrapper for callers that don't need skip/failure info.
pub fn parse_file_lenient(filepath: &str, source: &str) -> Vec<IndexDocument> {
    match parse_file(filepath, source) {
        ParseOutcome::Ok(docs) => docs,
        ParseOutcome::Unsupported { .. } | ParseOutcome::Failed { .. } => vec![],
    }
}
