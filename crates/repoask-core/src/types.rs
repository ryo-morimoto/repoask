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
    /// A module or namespace declaration.
    Module,
    /// A type alias.
    Type,
    /// A trait declaration.
    Trait,
    /// A constant binding.
    Const,
}

// ---------------------------------------------------------------------------
// Parser output types (discriminated union: Code | Reexport | Doc)
// ---------------------------------------------------------------------------

/// A parser document retained for search and investigation surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexDocument {
    /// A code symbol extracted via AST parsing.
    Code(Symbol),
    /// A re-export surface extracted from module export syntax.
    Reexport(Reexport),
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
    /// Parameter names of the symbol (empty for non-callable symbols).
    pub params: Vec<String>,
    /// Compact signature preview used by investigation surfaces.
    pub signature_preview: Option<String>,
    /// Structured doc comment or docstring information, if present.
    pub comment: Option<CommentInfo>,
    /// Export or visibility metadata for this symbol.
    pub export: ExportInfo,
}

/// A re-export surface extracted from module export syntax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reexport {
    /// Path of the exporting file relative to the repository root.
    pub filepath: String,
    /// First line of the export declaration (1-based).
    pub start_line: u32,
    /// Last line of the export declaration (1-based).
    pub end_line: u32,
    /// Symbol name referenced locally or from the source module.
    pub local_name: String,
    /// Public name exposed by the re-export surface.
    pub exported_name: String,
    /// Source module specifier when re-exporting from another file.
    pub source_specifier: Option<String>,
    /// Whether the re-export is type-only syntax.
    pub is_type_only: bool,
}

/// Structured doc comment metadata extracted from parser output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentInfo {
    /// The highest-signal one-line summary for the symbol.
    pub summary_line: Option<String>,
    /// Additional preview text when available.
    pub body_preview: Option<String>,
    /// Important contract or risk flags detected in the comment.
    pub flags: Vec<CommentFlag>,
    /// The parser source from which this comment was derived.
    pub source: CommentSource,
    /// Whether the comment was normalized successfully.
    pub normalization_status: CommentNormalizationStatus,
}

impl CommentInfo {
    /// Build normalized comment metadata from cleaned comment text.
    #[must_use]
    pub fn from_normalized_text(text: &str, source: CommentSource) -> Option<Self> {
        let normalized = normalize_comment_text(text);
        if normalized.is_empty() {
            return None;
        }

        let summary_line = extract_summary_line(&normalized);
        let body_preview = (normalized != summary_line)
            .then(|| truncate_chars(&normalized, 200))
            .filter(|preview| preview != &summary_line);

        Some(Self {
            summary_line: Some(summary_line),
            body_preview,
            flags: detect_comment_flags(&normalized),
            source,
            normalization_status: CommentNormalizationStatus::SummaryOnly,
        })
    }

    /// Return flattened text for search indexing.
    #[must_use]
    pub fn searchable_text(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(summary_line) = &self.summary_line {
            parts.push(summary_line.as_str());
        }
        if let Some(body_preview) = &self.body_preview
            && self
                .summary_line
                .as_ref()
                .is_none_or(|summary| summary != body_preview)
        {
            parts.push(body_preview.as_str());
        }

        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

/// High-signal comment flags used by investigation surfaces.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommentFlag {
    /// Indicates deprecated behavior.
    Deprecated,
    /// Indicates unstable or experimental behavior.
    Experimental,
    /// Indicates an internal-only surface.
    Internal,
    /// Indicates unsafe behavior or contracts.
    Unsafe,
}

/// Source family for normalized comment data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommentSource {
    /// `JSDoc` or `TSDoc` style comments.
    JsDoc,
    /// Rust doc comments.
    RustDoc,
    /// Python docstring comments.
    PythonDocstring,
    /// Non-structured plain comments.
    PlainComment,
}

/// Status of structured comment normalization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommentNormalizationStatus {
    /// No comment was present.
    Missing,
    /// Only a summary line was derived.
    SummaryOnly,
    /// Structured comment fields were extracted successfully.
    Structured,
    /// Comment parsing attempted but failed.
    Failed,
}

/// Visibility or export metadata for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportInfo {
    /// Effective publicness for the investigation surface.
    pub publicness: Publicness,
    /// The syntax form by which the symbol is exported or exposed.
    pub export_kind: ExportKind,
    /// Optional enclosing container such as a module or namespace.
    pub container: Option<String>,
}

impl ExportInfo {
    /// Build export metadata with the provided publicness and kind.
    #[must_use]
    pub const fn new(publicness: Publicness, export_kind: ExportKind) -> Self {
        Self {
            publicness,
            export_kind,
            container: None,
        }
    }

    /// Build metadata for a symbol with unknown publicness.
    #[must_use]
    pub const fn unknown() -> Self {
        Self::new(Publicness::Unknown, ExportKind::Unknown)
    }

    /// Build metadata for a private symbol.
    #[must_use]
    pub const fn private() -> Self {
        Self::new(Publicness::Private, ExportKind::None)
    }

    /// Build metadata for a named public export.
    #[must_use]
    pub const fn public_named() -> Self {
        Self::new(Publicness::Public, ExportKind::Named)
    }

    /// Build metadata for a default public export.
    #[must_use]
    pub const fn public_default() -> Self {
        Self::new(Publicness::Public, ExportKind::Default)
    }

    /// Build metadata for a public re-export.
    #[must_use]
    pub const fn reexported() -> Self {
        Self::new(Publicness::Reexported, ExportKind::Reexport)
    }

    /// Return true when this symbol is part of the public investigation surface.
    #[must_use]
    pub const fn is_surface_public(&self) -> bool {
        matches!(self.publicness, Publicness::Public | Publicness::Reexported)
    }
}

impl Default for ExportInfo {
    fn default() -> Self {
        Self::unknown()
    }
}

/// Investigation-facing publicness categories.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Publicness {
    /// Publicly exposed from the current file or module.
    Public,
    /// Publicly exposed through a re-export surface.
    Reexported,
    /// Visible only within the package or crate boundary.
    Package,
    /// Private to the current implementation boundary.
    Private,
    /// Publicness could not be determined confidently.
    Unknown,
}

/// Syntax form describing how a symbol is exported or exposed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExportKind {
    /// Named export such as `export function foo`.
    Named,
    /// Default export such as `export default function`.
    Default,
    /// Re-export surface such as `pub use` or `export { foo } from`.
    Reexport,
    /// Exported from a containing module or namespace.
    ModuleMember,
    /// Not exported.
    None,
    /// Export kind could not be determined confidently.
    Unknown,
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
    #[must_use]
    pub const fn score(&self) -> f32 {
        match self {
            Self::Code(r) => r.score,
            Self::Doc(r) => r.score,
        }
    }

    /// Return the file path associated with this result.
    #[must_use]
    pub fn filepath(&self) -> &str {
        match self {
            Self::Code(r) => &r.filepath,
            Self::Doc(r) => &r.filepath,
        }
    }
}

/// Filter for the type of indexed document to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDocumentType {
    /// Search only code symbols.
    Code,
    /// Search only documentation sections.
    Doc,
}

/// Optional filters applied during search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchFilters {
    /// Restrict results to files under any of these directory prefixes.
    pub dirs: Vec<String>,
    /// Restrict results to files with any of these extensions.
    pub exts: Vec<String>,
    /// Restrict results to either code or documentation.
    pub result_type: Option<SearchDocumentType>,
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

impl ParseOutcome {
    /// Collapse the parse outcome into documents only.
    ///
    /// Returns `Some` when parsing succeeded and `None` for unsupported or failed parses.
    #[must_use]
    pub fn into_lenient(self) -> Option<Vec<IndexDocument>> {
        match self {
            Self::Ok(docs) => Some(docs),
            Self::Unsupported { .. } | Self::Failed { .. } => None,
        }
    }
}

fn normalize_comment_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_summary_line(text: &str) -> String {
    let summary_end = text
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '.' | '!' | '?').then_some(index + ch.len_utf8()))
        .unwrap_or_else(|| text.len().min(summary_limit(text)));

    text[..summary_end].trim().to_owned()
}

fn detect_comment_flags(text: &str) -> Vec<CommentFlag> {
    let lower = text.to_ascii_lowercase();
    let mut flags = Vec::new();

    maybe_push_flag(
        &mut flags,
        lower.contains("@deprecated") || lower.contains("deprecated"),
        CommentFlag::Deprecated,
    );
    maybe_push_flag(
        &mut flags,
        lower.contains("@experimental") || lower.contains("experimental"),
        CommentFlag::Experimental,
    );
    maybe_push_flag(
        &mut flags,
        lower.contains("@internal") || lower.contains("internal use only"),
        CommentFlag::Internal,
    );
    maybe_push_flag(
        &mut flags,
        lower.contains("unsafe") || lower.contains("@unsafe"),
        CommentFlag::Unsafe,
    );

    flags
}

fn maybe_push_flag(flags: &mut Vec<CommentFlag>, condition: bool, flag: CommentFlag) {
    if condition && !flags.contains(&flag) {
        flags.push(flag);
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn summary_limit(text: &str) -> usize {
    let truncated = truncate_chars(text, 120);
    truncated.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn comment_info_extracts_summary_and_flags() {
        let comment = CommentInfo::from_normalized_text(
            "@deprecated Validate token safely. Internal use only.",
            CommentSource::JsDoc,
        )
        .unwrap();

        assert_eq!(
            comment.summary_line.as_deref(),
            Some("@deprecated Validate token safely.")
        );
        assert!(comment.flags.contains(&CommentFlag::Deprecated));
        assert!(comment.flags.contains(&CommentFlag::Internal));
    }

    #[test]
    fn searchable_text_deduplicates_identical_body_preview() {
        let comment = CommentInfo {
            summary_line: Some("Validate token".to_owned()),
            body_preview: Some("Validate token".to_owned()),
            flags: vec![],
            source: CommentSource::PlainComment,
            normalization_status: CommentNormalizationStatus::SummaryOnly,
        };

        assert_eq!(comment.searchable_text().as_deref(), Some("Validate token"));
    }

    #[test]
    fn export_info_surface_public_matches_public_variants() {
        assert!(ExportInfo::public_named().is_surface_public());
        assert!(ExportInfo::reexported().is_surface_public());
        assert!(!ExportInfo::private().is_surface_public());
        assert!(!ExportInfo::unknown().is_surface_public());
    }
}
