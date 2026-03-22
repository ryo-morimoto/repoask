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

/// Parse a file using tree-sitter if the extension is supported.
///
/// Returns `None` if the extension is not a tree-sitter language.
/// Returns `Some(vec![])` if parsing fails.
pub fn parse_file(filepath: &str, source: &str) -> Option<Vec<IndexDocument>> {
    let ext = filepath.rsplit('.').next()?;
    let (language, query) = language_for_extension(ext)?;

    let symbols = parser::extract_symbols(source, filepath, &language, query);
    Some(symbols.into_iter().map(IndexDocument::Code).collect())
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

/// Returns true if the given extension is handled by this crate.
pub fn supports_extension(ext: &str) -> bool {
    language_for_extension(ext).is_some()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;
    use repoask_core::types::SymbolKind;

    fn get_symbols(filepath: &str, source: &str) -> Vec<Symbol> {
        parse_file(filepath, source)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|doc| match doc {
                IndexDocument::Code(s) => Some(s),
                IndexDocument::Doc(_) => None,
            })
            .collect()
    }

    #[test]
    fn test_rust() {
        let source = "/// Add two numbers\nfn add(a: i32, b: i32) -> i32 { a + b }\n\nstruct Point { x: f64 }\n\nenum Color { Red }\n";
        let symbols = get_symbols("lib.rs", source);
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "Color" && s.kind == SymbolKind::Enum));
        let add = symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add.doc_comment.as_ref().unwrap().contains("Add two numbers"));
    }

    #[test]
    fn test_python() {
        let source = "def greet(name):\n    return name\n\nclass UserService:\n    def find(self, user_id):\n        pass\n";
        let symbols = get_symbols("app.py", source);
        assert!(symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "find"));
    }

    #[test]
    fn test_go() {
        let source = "func main() {\n\tfmt.Println(\"hello\")\n}\n\ntype Config struct {\n\tPort int\n}\n";
        let symbols = get_symbols("main.go", source);
        assert!(symbols.iter().any(|s| s.name == "main" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Config" && s.kind == SymbolKind::Type));
    }

    #[test]
    fn test_unsupported_extension() {
        assert!(parse_file("test.zig", "const x = 1;").is_none());
    }
}
