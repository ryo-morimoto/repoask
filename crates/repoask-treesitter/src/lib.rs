//! Tree-sitter based symbol extraction for multiple languages.
//!
//! This crate provides symbol extraction for languages that use tree-sitter
//! grammars (Rust, Python, Go, Java, C, C++, Ruby). It depends on C-compiled
//! grammar crates and is NOT WASM-compatible without wasi-sdk.
//!
//! For TS/JS and Markdown parsing (pure Rust, WASM-safe), use `repoask-parser`.

mod parser;
mod queries;

use repoask_core::types::{IndexDocument, Symbol};

/// Outcome of parsing a single file with tree-sitter.
#[derive(Debug)]
pub enum ParseOutcome {
    /// Successfully extracted symbols.
    Ok(Vec<IndexDocument>),
    /// File extension not supported by tree-sitter.
    Unsupported {
        /// The file path that was skipped.
        filepath: String,
        /// The file extension (or `None` if no extension).
        extension: Option<String>,
    },
    /// tree-sitter parser or query failed.
    Failed {
        /// The file path that failed.
        filepath: String,
        /// Description of the failure.
        reason: String,
    },
}

/// Error type for tree-sitter parse operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The file extension is not a tree-sitter language.
    #[error("unsupported tree-sitter extension: {filepath}")]
    UnsupportedExtension {
        /// The file path.
        filepath: String,
    },
    /// tree-sitter failed to set the language grammar.
    #[error("failed to set language for {filepath}")]
    LanguageError {
        /// The file path.
        filepath: String,
    },
    /// tree-sitter failed to parse the source.
    #[error("tree-sitter parse failed for {filepath}")]
    ParseFailed {
        /// The file path.
        filepath: String,
    },
    /// tree-sitter query compilation failed.
    #[error("tree-sitter query error for {filepath}: {reason}")]
    QueryError {
        /// The file path.
        filepath: String,
        /// The query error message.
        reason: String,
    },
}

/// Parse a file using tree-sitter if the extension is supported.
///
/// Returns a [`ParseOutcome`] distinguishing success, unsupported, and failure.
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

    let (language, query) = match language_for_extension(ext) {
        Some(lq) => lq,
        None => {
            return ParseOutcome::Unsupported {
                filepath: filepath.to_owned(),
                extension: Some(ext.to_owned()),
            };
        }
    };

    let symbols = parser::extract_symbols(source, filepath, &language, query);
    ParseOutcome::Ok(symbols.into_iter().map(IndexDocument::Code).collect())
}

/// Parse a file, returning only the documents (ignoring skips/failures).
///
/// Convenience wrapper for callers that don't need skip/failure info.
pub fn parse_file_lenient(filepath: &str, source: &str) -> Option<Vec<IndexDocument>> {
    let ext = filepath.rsplit('.').next()?;
    let (language, query) = language_for_extension(ext)?;
    let symbols = parser::extract_symbols(source, filepath, &language, query);
    Some(symbols.into_iter().map(IndexDocument::Code).collect())
}

/// Returns true if the given extension is handled by this crate.
pub fn supports_extension(ext: &str) -> bool {
    language_for_extension(ext).is_some()
}

/// Returns the tree-sitter language and query for a given file extension.
fn language_for_extension(ext: &str) -> Option<(tree_sitter::Language, &'static str)> {
    match ext {
        "rs" => Some((tree_sitter_rust::LANGUAGE.into(), queries::RUST)),
        "py" | "pyi" => Some((tree_sitter_python::LANGUAGE.into(), queries::PYTHON)),
        "go" => Some((tree_sitter_go::LANGUAGE.into(), queries::GO)),
        "java" => Some((tree_sitter_java::LANGUAGE.into(), queries::JAVA)),
        "c" | "h" => Some((tree_sitter_c::LANGUAGE.into(), queries::C)),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => {
            Some((tree_sitter_cpp::LANGUAGE.into(), queries::CPP))
        }
        "rb" => Some((tree_sitter_ruby::LANGUAGE.into(), queries::RUBY)),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;
    use repoask_core::types::SymbolKind;

    fn get_symbols(filepath: &str, source: &str) -> Vec<Symbol> {
        match parse_file(filepath, source) {
            ParseOutcome::Ok(docs) => docs
                .into_iter()
                .filter_map(|doc| match doc {
                    IndexDocument::Code(s) => Some(s),
                    IndexDocument::Doc(_) => None,
                })
                .collect(),
            ParseOutcome::Unsupported { .. } | ParseOutcome::Failed { .. } => vec![],
        }
    }

    #[test]
    fn test_rust() {
        let source = "/// Add two numbers\nfn add(a: i32, b: i32) -> i32 { a + b }\n\nstruct Point { x: f64 }\n\nenum Color { Red }\n";
        let symbols = get_symbols("lib.rs", source);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "add" && s.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Point" && s.kind == SymbolKind::Struct)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Color" && s.kind == SymbolKind::Enum)
        );
        let add = symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(
            add.doc_comment
                .as_ref()
                .unwrap()
                .contains("Add two numbers")
        );
    }

    #[test]
    fn test_python() {
        let source = "def greet(name):\n    return name\n\nclass UserService:\n    def find(self, user_id):\n        pass\n";
        let symbols = get_symbols("app.py", source);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "greet" && s.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
        );
        assert!(symbols.iter().any(|s| s.name == "find"));
    }

    #[test]
    fn test_go() {
        let source =
            "func main() {\n\tfmt.Println(\"hello\")\n}\n\ntype Config struct {\n\tPort int\n}\n";
        let symbols = get_symbols("main.go", source);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "main" && s.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Config" && s.kind == SymbolKind::Type)
        );
    }

    #[test]
    fn test_unsupported_extension() {
        assert!(matches!(
            parse_file("test.zig", "const x = 1;"),
            ParseOutcome::Unsupported { .. }
        ));
    }
}
