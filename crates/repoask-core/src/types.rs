use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Symbol kinds (shared by parser output and search results)
// ---------------------------------------------------------------------------

/// The kind of code symbol extracted from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    /// A standalone function.
    Function,
    /// A method on a class, struct, or impl block.
    Method,
    /// A class declaration.
    Class,
    /// A struct declaration.
    Struct,
    /// An enum declaration.
    Enum,
    /// An interface declaration (e.g. TypeScript `interface`).
    Interface,
    /// A type alias.
    Type,
    /// A trait declaration.
    Trait,
    /// A constant binding.
    Const,
}

// ---------------------------------------------------------------------------
// Parser output types (discriminated union: Code | Doc)
// ---------------------------------------------------------------------------

/// A document to be indexed, either a code symbol or a documentation section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexDocument {
    /// A code symbol extracted via AST parsing.
    Code(Symbol),
    /// A documentation section extracted from Markdown.
    Doc(DocSection),
}

/// A code symbol extracted from source via AST parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// The symbol's identifier name.
    pub name: String,
    /// The kind of symbol (function, class, etc.).
    pub kind: SymbolKind,
    /// Path of the source file relative to the repository root.
    pub filepath: String,
    /// First line of the symbol definition (1-based).
    pub start_line: u32,
    /// Last line of the symbol definition (1-based).
    pub end_line: u32,
    /// Doc comment or docstring, if present.
    pub doc_comment: Option<String>,
    /// Parameter names of the symbol (empty for non-callable symbols).
    pub params: Vec<String>,
}

/// A section of documentation extracted from a Markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSection {
    /// Path of the Markdown file relative to the repository root.
    pub filepath: String,
    /// The heading text of this section.
    pub section_title: String,
    /// Ancestor headings from root to parent (for nested sections).
    pub heading_hierarchy: Vec<String>,
    /// The body text content of this section.
    pub content: String,
    /// Symbol names found inside fenced code blocks.
    pub code_symbols: Vec<String>,
    /// First line of this section (1-based).
    pub start_line: u32,
    /// Last line of this section (1-based).
    pub end_line: u32,
}

// ---------------------------------------------------------------------------
// Search result types (discriminated union: Code | Doc)
// ---------------------------------------------------------------------------

/// A ranked search result, either from code or documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResult {
    /// A matching code symbol.
    Code(CodeResult),
    /// A matching documentation section.
    Doc(DocResult),
}

/// Search result for a code symbol match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResult {
    /// Path of the source file.
    pub filepath: String,
    /// The matched symbol name.
    pub name: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// First line of the symbol definition (1-based).
    pub start_line: u32,
    /// Last line of the symbol definition (1-based).
    pub end_line: u32,
    /// BM25 relevance score.
    pub score: f32,
    /// Whether the symbol is from an example/demo file.
    pub is_example: bool,
}

/// Search result for a documentation section match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocResult {
    /// Path of the Markdown file.
    pub filepath: String,
    /// The section heading that matched.
    pub section: String,
    /// A preview of the section content.
    pub snippet: String,
    /// BM25 relevance score.
    pub score: f32,
}

impl SearchResult {
    /// Return the BM25 relevance score of this result.
    pub fn score(&self) -> f32 {
        match self {
            Self::Code(r) => r.score,
            Self::Doc(r) => r.score,
        }
    }

    /// Return the file path associated with this result.
    pub fn filepath(&self) -> &str {
        match self {
            Self::Code(r) => &r.filepath,
            Self::Doc(r) => &r.filepath,
        }
    }
}

// ---------------------------------------------------------------------------
// Parser outcome (shared by all parser crates)
// ---------------------------------------------------------------------------

/// Outcome of parsing a single file.
#[derive(Debug)]
pub enum ParseOutcome {
    /// Successfully extracted documents.
    Ok(Vec<IndexDocument>),
    /// File extension not supported by this parser.
    Unsupported {
        /// The file path that was skipped.
        filepath: String,
        /// The file extension (or `None` if no extension).
        extension: Option<String>,
    },
    /// Parser encountered an error.
    Failed {
        /// The file path that failed.
        filepath: String,
        /// Description of the failure.
        reason: String,
    },
}
