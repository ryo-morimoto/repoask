use tree_sitter::Language;

use crate::tree_sitter_parser;

/// How a file should be parsed.
pub enum ParserKind {
    /// Use oxc_parser for TS/JS/TSX/JSX
    Oxc,
    /// Use tree-sitter with a specific language and query
    TreeSitter {
        language: Language,
        query: &'static str,
    },
    /// Parse as markdown documentation
    Markdown,
}

/// Determine the parser to use for a given file extension.
pub fn parser_for_extension(ext: &str) -> Option<ParserKind> {
    match ext {
        // TS/JS → oxc (faster, richer type info)
        "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" => Some(ParserKind::Oxc),

        // Rust
        "rs" => Some(ParserKind::TreeSitter {
            language: tree_sitter_rust::LANGUAGE.into(),
            query: tree_sitter_parser::rust_query(),
        }),

        // Python
        "py" | "pyi" => Some(ParserKind::TreeSitter {
            language: tree_sitter_python::LANGUAGE.into(),
            query: tree_sitter_parser::python_query(),
        }),

        // Go
        "go" => Some(ParserKind::TreeSitter {
            language: tree_sitter_go::LANGUAGE.into(),
            query: tree_sitter_parser::go_query(),
        }),

        // Java
        "java" => Some(ParserKind::TreeSitter {
            language: tree_sitter_java::LANGUAGE.into(),
            query: tree_sitter_parser::java_query(),
        }),

        // C
        "c" | "h" => Some(ParserKind::TreeSitter {
            language: tree_sitter_c::LANGUAGE.into(),
            query: tree_sitter_parser::c_query(),
        }),

        // C++
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(ParserKind::TreeSitter {
            language: tree_sitter_cpp::LANGUAGE.into(),
            query: tree_sitter_parser::cpp_query(),
        }),

        // Ruby
        "rb" => Some(ParserKind::TreeSitter {
            language: tree_sitter_ruby::LANGUAGE.into(),
            query: tree_sitter_parser::ruby_query(),
        }),

        // Markdown
        "md" | "mdx" => Some(ParserKind::Markdown),

        _ => None,
    }
}

/// Check if a file is likely a documentation file based on path.
pub fn is_doc_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".mdx")
        || lower.contains("readme")
        || lower.contains("changelog")
        || lower.contains("contributing")
}
