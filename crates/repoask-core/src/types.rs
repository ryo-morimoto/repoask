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
// Search result types (discriminated union: Code | Doc | Example)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResult {
    Code(CodeResult),
    Doc(DocResult),
    Example(ExampleResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocResult {
    pub filepath: String,
    pub section: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleResult {
    pub filepath: String,
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
}

impl SearchResult {
    pub fn score(&self) -> f32 {
        match self {
            Self::Code(r) => r.score,
            Self::Doc(r) => r.score,
            Self::Example(r) => r.score,
        }
    }

    pub fn filepath(&self) -> &str {
        match self {
            Self::Code(r) => &r.filepath,
            Self::Doc(r) => &r.filepath,
            Self::Example(r) => &r.filepath,
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

// ---------------------------------------------------------------------------
// Internal index types
// ---------------------------------------------------------------------------

pub type DocId = u32;
pub type FieldId = u8;

pub const FIELD_SYMBOL_NAME: FieldId = 0;
pub const FIELD_DOC_CONTENT: FieldId = 1;
pub const FIELD_PARAMS: FieldId = 2;
pub const FIELD_FILEPATH: FieldId = 3;
pub const NUM_FIELDS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    pub doc_id: DocId,
    pub field_id: FieldId,
    pub term_freq: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStats {
    pub total_length: u64,
    pub doc_count: u32,
}

impl FieldStats {
    pub fn avg_length(&self) -> f32 {
        if self.doc_count == 0 {
            return 0.0;
        }
        self.total_length as f32 / self.doc_count as f32
    }
}
