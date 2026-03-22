use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Symbol kinds (shared by parser output and search results)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Type,
    Trait,
    Const,
}

// ---------------------------------------------------------------------------
// Parser output types (discriminated union: Code | Doc)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexDocument {
    Code(Symbol),
    Doc(DocSection),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub filepath: String,
    pub start_line: u32,
    pub end_line: u32,
    pub doc_comment: Option<String>,
    pub params: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSection {
    pub filepath: String,
    pub section_title: String,
    pub heading_hierarchy: Vec<String>,
    pub content: String,
    pub code_symbols: Vec<String>,
    pub start_line: u32,
    pub end_line: u32,
}

// ---------------------------------------------------------------------------
// Search result types (discriminated union: Code | Doc)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResult {
    Code(CodeResult),
    Doc(DocResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
    pub is_example: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocResult {
    pub filepath: String,
    pub section: String,
    pub snippet: String,
    pub score: f32,
}

impl SearchResult {
    pub fn score(&self) -> f32 {
        match self {
            Self::Code(r) => r.score,
            Self::Doc(r) => r.score,
        }
    }

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
