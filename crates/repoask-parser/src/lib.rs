//! Source code and documentation parsing for repoask.
//!
//! Handles TS/JS (via oxc) and Markdown. Pure Rust, WASM-compatible.
//! For tree-sitter based languages (Rust, Python, Go, etc.), see `repoask-treesitter`.

/// Markdown document parsing and section extraction.
pub mod markdown;
/// TypeScript/JavaScript symbol extraction via oxc-parser.
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
#[must_use]
pub fn parse_file(filepath: &str, source: &str) -> ParseOutcome {
    let Some(ext) = filepath.rsplit('.').next() else {
        return ParseOutcome::Unsupported {
            filepath: filepath.to_owned(),
            extension: None,
        };
    };

    match ext {
        "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" => {
            let documents = oxc::extract_ts_documents(source, filepath);
            if documents.is_empty() && !source.trim().is_empty() {
                ParseOutcome::Failed {
                    filepath: filepath.to_owned(),
                    reason: "oxc parser returned no symbols (possible parse error)".to_owned(),
                }
            } else {
                ParseOutcome::Ok(documents)
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
#[must_use]
pub fn parse_file_lenient(filepath: &str, source: &str) -> Option<Vec<IndexDocument>> {
    parse_file(filepath, source).into_lenient()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_lenient_returns_some_for_supported_markdown() {
        let docs = parse_file_lenient("README.md", "# Hello\n\nrepoask");
        assert!(docs.is_some());
    }

    #[test]
    fn parse_file_lenient_returns_none_for_unsupported_extension() {
        let docs = parse_file_lenient("README.txt", "repoask");
        assert!(docs.is_none());
    }
}
