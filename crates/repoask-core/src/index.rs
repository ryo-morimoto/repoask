use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::bm25::Bm25Scorer;
use crate::tokenizer::{tokenize_identifier, tokenize_query, tokenize_text};
use crate::types::{
    CodeResult, DocResult, IndexDocument, SearchDocumentType, SearchFilters, SearchResult,
};

// ---------------------------------------------------------------------------
// Index internal types
// ---------------------------------------------------------------------------

/// Numeric identifier for an indexed document.
pub type DocId = u32;
/// Numeric identifier for a field within a document.
pub type FieldId = u8;

/// Field index for symbol name or section heading.
pub const FIELD_SYMBOL_NAME: FieldId = 0;
/// Field index for doc comment or section body content.
pub const FIELD_DOC_CONTENT: FieldId = 1;
/// Field index for parameter names or code symbols in docs.
pub const FIELD_PARAMS: FieldId = 2;
/// Field index for the file path.
pub const FIELD_FILEPATH: FieldId = 3;
/// Total number of indexed fields per document.
pub const NUM_FIELDS: usize = 4;

/// A single term occurrence record in the inverted index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Posting {
    /// The document containing this term.
    pub doc_id: DocId,
    /// The field in which the term appears.
    pub field_id: FieldId,
    /// How many times the term appears in this field.
    pub term_freq: u16,
}

/// Aggregate statistics for a single field across all documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStats {
    /// Sum of field lengths across all documents.
    pub total_length: u64,
    /// Number of documents that have a non-empty value for this field.
    pub doc_count: u32,
}

impl FieldStats {
    /// Return the average field length, or 0.0 if no documents exist.
    #[allow(
        clippy::cast_precision_loss,
        reason = "BM25 uses f32 scores and field lengths are already bounded per document"
    )]
    #[must_use]
    pub fn avg_length(&self) -> f32 {
        if self.doc_count == 0 {
            return 0.0;
        }
        self.total_length as f32 / self.doc_count as f32
    }
}

// ---------------------------------------------------------------------------
// Stored document metadata (discriminated union matching IndexDocument)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
enum StoredDoc {
    Code {
        filepath: String,
        name: String,
        kind: crate::types::SymbolKind,
        start_line: u32,
        end_line: u32,
        is_example: bool,
    },
    Doc {
        filepath: String,
        section_title: String,
        content_preview: String,
    },
}

// ---------------------------------------------------------------------------
// Inverted index
// ---------------------------------------------------------------------------

/// BM25-backed inverted index over code symbols and documentation sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(
    clippy::module_name_repetitions,
    reason = "public API uses the standard inverted-index term"
)]
pub struct InvertedIndex {
    postings: HashMap<String, Vec<Posting>>,
    /// Pre-computed document frequency per term (unique `doc_id` count).
    doc_freqs: HashMap<String, u32>,
    documents: Vec<StoredDoc>,
    field_lengths: Vec<[u16; NUM_FIELDS]>,
    field_stats: [FieldStats; NUM_FIELDS],
}

/// Helper for min-heap top-k selection. Ordered by score ascending (min first),
/// with ties broken by `doc_id` descending so the heap evicts the least desirable.
#[derive(PartialEq)]
struct ScoredDoc {
    doc_id: DocId,
    score: f32,
}

impl Eq for ScoredDoc {}

impl PartialOrd for ScoredDoc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredDoc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| other.doc_id.cmp(&self.doc_id))
    }
}

impl InvertedIndex {
    /// Build an index from a collection of parsed documents.
    #[must_use]
    pub fn build(docs: &[IndexDocument]) -> Self {
        let mut index = Self {
            postings: HashMap::new(),
            doc_freqs: HashMap::new(),
            documents: Vec::with_capacity(docs.len()),
            field_lengths: Vec::with_capacity(docs.len()),
            field_stats: std::array::from_fn(|_| FieldStats {
                total_length: 0,
                doc_count: 0,
            }),
        };

        let mut term_freq_buf = HashMap::new();
        for doc in docs {
            index.add_document(doc, &mut term_freq_buf);
        }

        // Pre-compute document frequencies (unique doc_id count per term)
        for (term, postings) in &index.postings {
            let mut seen = std::collections::HashSet::new();
            for p in postings {
                seen.insert(p.doc_id);
            }
            index
                .doc_freqs
                .insert(term.clone(), saturating_u32(seen.len()));
        }

        index
    }

    fn add_document(&mut self, doc: &IndexDocument, term_freq_buf: &mut HashMap<String, u16>) {
        let doc_id = saturating_doc_id(self.documents.len());
        let mut lengths = [0u16; NUM_FIELDS];

        match doc {
            IndexDocument::Code(symbol) => {
                let is_example = is_example_path(&symbol.filepath);

                // Field 0: symbol name tokens
                let name_tokens = tokenize_identifier(&symbol.name);
                lengths[usize::from(FIELD_SYMBOL_NAME)] = saturating_u16(name_tokens.len());
                self.add_field_tokens(doc_id, FIELD_SYMBOL_NAME, &name_tokens, term_freq_buf);

                // Field 1: doc comment tokens
                if let Some(ref comment) = symbol.doc_comment {
                    let comment_tokens = tokenize_text(comment);
                    lengths[usize::from(FIELD_DOC_CONTENT)] = saturating_u16(comment_tokens.len());
                    self.add_field_tokens(
                        doc_id,
                        FIELD_DOC_CONTENT,
                        &comment_tokens,
                        term_freq_buf,
                    );
                }

                // Field 2: parameter name tokens
                let param_tokens: Vec<String> = symbol
                    .params
                    .iter()
                    .flat_map(|p| tokenize_identifier(p))
                    .collect();
                lengths[usize::from(FIELD_PARAMS)] = saturating_u16(param_tokens.len());
                self.add_field_tokens(doc_id, FIELD_PARAMS, &param_tokens, term_freq_buf);

                // Field 3: filepath tokens
                let path_tokens = tokenize_identifier(&symbol.filepath);
                lengths[usize::from(FIELD_FILEPATH)] = saturating_u16(path_tokens.len());
                self.add_field_tokens(doc_id, FIELD_FILEPATH, &path_tokens, term_freq_buf);

                self.documents.push(StoredDoc::Code {
                    filepath: symbol.filepath.clone(),
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    start_line: symbol.start_line,
                    end_line: symbol.end_line,
                    is_example,
                });
            }
            IndexDocument::Doc(section) => {
                // Field 0: heading tokens (same weight slot as symbol name)
                let mut heading_tokens = tokenize_text(&section.section_title);
                for ancestor in &section.heading_hierarchy {
                    heading_tokens.extend(tokenize_text(ancestor));
                }
                lengths[usize::from(FIELD_SYMBOL_NAME)] = saturating_u16(heading_tokens.len());
                self.add_field_tokens(doc_id, FIELD_SYMBOL_NAME, &heading_tokens, term_freq_buf);

                // Field 1: body content tokens
                let body_tokens = tokenize_text(&section.content);
                lengths[usize::from(FIELD_DOC_CONTENT)] = saturating_u16(body_tokens.len());
                self.add_field_tokens(doc_id, FIELD_DOC_CONTENT, &body_tokens, term_freq_buf);

                // Field 2: code symbols extracted from fenced blocks
                let code_sym_tokens: Vec<String> = section
                    .code_symbols
                    .iter()
                    .flat_map(|s| tokenize_identifier(s))
                    .collect();
                lengths[usize::from(FIELD_PARAMS)] = saturating_u16(code_sym_tokens.len());
                self.add_field_tokens(doc_id, FIELD_PARAMS, &code_sym_tokens, term_freq_buf);

                // Field 3: filepath tokens
                let path_tokens = tokenize_identifier(&section.filepath);
                lengths[usize::from(FIELD_FILEPATH)] = saturating_u16(path_tokens.len());
                self.add_field_tokens(doc_id, FIELD_FILEPATH, &path_tokens, term_freq_buf);

                let preview = section.content.chars().take(200).collect::<String>();

                self.documents.push(StoredDoc::Doc {
                    filepath: section.filepath.clone(),
                    section_title: section.section_title.clone(),
                    content_preview: preview,
                });
            }
        }

        // Update field stats
        for (i, &len) in lengths.iter().enumerate() {
            self.field_stats[i].total_length += u64::from(len);
            if len > 0 {
                self.field_stats[i].doc_count += 1;
            }
        }

        self.field_lengths.push(lengths);
    }

    fn add_field_tokens(
        &mut self,
        doc_id: DocId,
        field_id: FieldId,
        tokens: &[String],
        term_freq_buf: &mut HashMap<String, u16>,
    ) {
        term_freq_buf.clear();
        for token in tokens {
            *term_freq_buf.entry(token.clone()).or_insert(0) += 1;
        }

        for (term, freq) in term_freq_buf.drain() {
            self.postings.entry(term).or_default().push(Posting {
                doc_id,
                field_id,
                term_freq: freq,
            });
        }
    }

    /// Search the index and return the top results ranked by BM25 score.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search_with_filters(query, limit, &SearchFilters::default())
    }

    /// Search the index and apply directory, extension, and type filters.
    #[must_use]
    pub fn search_with_filters(
        &self,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Vec<SearchResult> {
        let query_tokens = tokenize_query(query);
        if query_tokens.is_empty() || limit == 0 {
            return vec![];
        }

        let scorer = Bm25Scorer::new();
        let total_docs = saturating_u32(self.documents.len());
        let mut doc_scores: HashMap<DocId, f32> = HashMap::new();

        for token in &query_tokens {
            let Some(postings) = self.postings.get(token.as_str()) else {
                continue;
            };

            let doc_freq = self.doc_freqs.get(token.as_str()).copied().unwrap_or(0);

            for posting in postings {
                let doc_index = usize::try_from(posting.doc_id).unwrap_or_default();
                let field_index = usize::from(posting.field_id);
                let field_length = self.field_lengths[doc_index][field_index];
                let score = scorer.score(crate::bm25::ScoreInput {
                    term_freq: posting.term_freq,
                    field_length,
                    field_id: posting.field_id,
                    field_stats: &self.field_stats[field_index],
                    doc_freq,
                    total_docs,
                });
                *doc_scores.entry(posting.doc_id).or_insert(0.0) += score;
            }
        }

        // Top-k selection using a min-heap of size `limit` — O(n log k).
        let mut heap: BinaryHeap<Reverse<ScoredDoc>> = BinaryHeap::with_capacity(limit + 1);
        for (doc_id, score) in doc_scores {
            if !self.matches_filters(doc_id, filters) {
                continue;
            }
            heap.push(Reverse(ScoredDoc { doc_id, score }));
            if heap.len() > limit {
                heap.pop();
            }
        }

        let mut top_k: Vec<ScoredDoc> = heap.into_iter().map(|Reverse(sd)| sd).collect();
        top_k.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.doc_id.cmp(&b.doc_id))
        });

        top_k
            .into_iter()
            .map(|sd| self.to_search_result(sd.doc_id, sd.score))
            .collect()
    }

    fn matches_filters(&self, doc_id: DocId, filters: &SearchFilters) -> bool {
        let doc_index = usize::try_from(doc_id).unwrap_or_default();
        let Some(doc) = self.documents.get(doc_index) else {
            return false;
        };

        let (filepath, doc_type) = match doc {
            StoredDoc::Code { filepath, .. } => (filepath.as_str(), SearchDocumentType::Code),
            StoredDoc::Doc { filepath, .. } => (filepath.as_str(), SearchDocumentType::Doc),
        };

        if filters
            .result_type
            .is_some_and(|result_type| result_type != doc_type)
        {
            return false;
        }

        if !filters.dirs.is_empty()
            && !filters
                .dirs
                .iter()
                .any(|dir| path_matches_dir(filepath, dir.as_str()))
        {
            return false;
        }

        if !filters.exts.is_empty() {
            let ext = Path::new(filepath)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_ascii_lowercase);

            if !ext
                .as_ref()
                .is_some_and(|ext| filters.exts.iter().any(|candidate| candidate == ext))
            {
                return false;
            }
        }

        true
    }

    fn to_search_result(&self, doc_id: DocId, score: f32) -> SearchResult {
        let doc_index = usize::try_from(doc_id).unwrap_or_default();
        match &self.documents[doc_index] {
            StoredDoc::Code {
                filepath,
                name,
                kind,
                start_line,
                end_line,
                is_example,
            } => SearchResult::Code(CodeResult {
                filepath: filepath.clone(),
                name: name.clone(),
                kind: *kind,
                start_line: *start_line,
                end_line: *end_line,
                score,
                is_example: *is_example,
            }),
            StoredDoc::Doc {
                filepath,
                section_title,
                content_preview,
            } => SearchResult::Doc(DocResult {
                filepath: filepath.clone(),
                section: section_title.clone(),
                snippet: content_preview.clone(),
                score,
            }),
        }
    }

    /// Return the total number of indexed documents.
    #[must_use]
    pub fn doc_count(&self) -> usize {
        self.documents.len()
    }
}

fn is_example_path(filepath: &str) -> bool {
    let lower = filepath.to_lowercase();
    lower.contains("example") || lower.contains("sample") || lower.contains("demo")
}

fn path_matches_dir(filepath: &str, dir: &str) -> bool {
    filepath.replace('\\', "/").starts_with(&format!("{dir}/"))
}

fn saturating_doc_id(value: usize) -> DocId {
    DocId::try_from(value).unwrap_or(DocId::MAX)
}

fn saturating_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions"
)]
mod tests {
    use super::*;
    use crate::types::{DocSection, Symbol, SymbolKind};

    fn make_symbol(name: &str, filepath: &str) -> IndexDocument {
        IndexDocument::Code(Symbol {
            name: name.to_owned(),
            kind: SymbolKind::Function,
            filepath: filepath.to_owned(),
            start_line: 1,
            end_line: 10,
            doc_comment: None,
            params: vec![],
        })
    }

    fn make_doc(title: &str, content: &str, filepath: &str) -> IndexDocument {
        IndexDocument::Doc(DocSection {
            filepath: filepath.to_owned(),
            section_title: title.to_owned(),
            heading_hierarchy: vec![],
            content: content.to_owned(),
            code_symbols: vec![],
            start_line: 1,
            end_line: 20,
        })
    }

    #[test]
    fn test_empty_index() {
        let docs: Vec<IndexDocument> = vec![];
        let index = InvertedIndex::build(&docs);
        let results = index.search("anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let docs = vec![make_symbol("foo", "src/foo.rs")];
        let index = InvertedIndex::build(&docs);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_symbol_name_match() {
        let docs = vec![
            make_symbol("validateToken", "src/auth.rs"),
            make_symbol("parseJSON", "src/json.rs"),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("validate token", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].filepath(), "src/auth.rs");
    }

    #[test]
    fn test_doc_section_match() {
        let docs = vec![
            make_doc(
                "Authentication",
                "This section explains how to authenticate with JWT tokens",
                "docs/auth.md",
            ),
            make_doc(
                "Installation",
                "Run npm install to get started",
                "docs/install.md",
            ),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("authentication jwt", 10);
        assert!(!results.is_empty());
        match &results[0] {
            SearchResult::Doc(r) => assert_eq!(r.section, "Authentication"),
            SearchResult::Code(_) => panic!("Expected Doc result"),
        }
    }

    #[test]
    fn test_symbol_name_ranked_higher_than_filepath() {
        let docs = vec![
            make_symbol("createUser", "src/auth/validate.rs"),
            make_symbol("validateToken", "src/token.rs"),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("validate", 10);
        // validateToken (name match) should rank above createUser (path match)
        assert_eq!(results[0].filepath(), "src/token.rs");
    }

    #[test]
    fn test_example_detection() {
        let docs = vec![make_symbol("handler", "examples/auth/login.ts")];
        let index = InvertedIndex::build(&docs);
        let results = index.search("handler", 10);
        assert!(matches!(
            results[0],
            SearchResult::Code(CodeResult {
                is_example: true,
                ..
            })
        ));
    }

    #[test]
    fn test_mixed_code_and_doc() {
        let docs = vec![
            make_symbol("authenticate", "src/auth.rs"),
            make_doc(
                "Authentication Guide",
                "Learn how to authenticate users",
                "docs/auth.md",
            ),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("authenticate", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_limit() {
        let docs: Vec<IndexDocument> = (0..100)
            .map(|i| make_symbol(&format!("validateItem{i}"), &format!("src/f{i}.rs")))
            .collect();
        let index = InvertedIndex::build(&docs);
        let results = index.search("validate", 5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_search_with_type_filter() {
        let docs = vec![
            make_symbol("authenticate", "src/auth.rs"),
            make_doc(
                "Authentication Guide",
                "Authenticate users with tokens",
                "docs/auth.md",
            ),
        ];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            result_type: Some(SearchDocumentType::Doc),
            ..SearchFilters::default()
        };

        let results = index.search_with_filters("authenticate", 10, &filters);

        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], SearchResult::Doc(_)));
    }

    #[test]
    fn test_search_with_dir_filter() {
        let docs = vec![
            make_symbol("validateToken", "src/auth/token.rs"),
            make_symbol("validateToken", "examples/auth/token.rs"),
        ];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            dirs: vec!["src".to_owned()],
            ..SearchFilters::default()
        };

        let results = index.search_with_filters("validate token", 10, &filters);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filepath(), "src/auth/token.rs");
    }

    #[test]
    fn test_search_with_extension_filter() {
        let docs = vec![
            make_symbol("parseConfig", "src/config.rs"),
            make_symbol("parseConfig", "src/config.ts"),
        ];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            exts: vec!["ts".to_owned()],
            ..SearchFilters::default()
        };

        let results = index.search_with_filters("parse config", 10, &filters);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filepath(), "src/config.ts");
    }

    #[test]
    fn test_search_with_combined_filters() {
        let docs = vec![
            make_doc("Authentication", "Authenticate users", "docs/auth.md"),
            make_doc(
                "Authentication",
                "Authenticate users in guides",
                "guides/auth.md",
            ),
            make_symbol("authenticate", "src/auth.ts"),
        ];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            dirs: vec!["docs".to_owned()],
            exts: vec!["md".to_owned()],
            result_type: Some(SearchDocumentType::Doc),
        };

        let results = index.search_with_filters("authenticate", 10, &filters);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filepath(), "docs/auth.md");
    }

    #[test]
    fn test_search_with_dir_filter_matches_windows_style_paths() {
        let docs = vec![make_symbol("validateToken", "src\\auth\\token.rs")];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            dirs: vec!["src/auth".to_owned()],
            ..SearchFilters::default()
        };

        let results = index.search_with_filters("validate token", 10, &filters);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filepath(), "src\\auth\\token.rs");
    }

    #[test]
    fn test_search_with_dir_filter_does_not_match_file_named_like_directory() {
        let docs = vec![
            make_doc("Root docs file", "metadata", "docs"),
            make_doc("Auth docs", "Authenticate users", "docs/auth.md"),
        ];
        let index = InvertedIndex::build(&docs);
        let filters = SearchFilters {
            dirs: vec!["docs".to_owned()],
            ..SearchFilters::default()
        };

        let results = index.search_with_filters("docs authenticate", 10, &filters);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filepath(), "docs/auth.md");
    }

    // -----------------------------------------------------------------------
    // Snapshot tests (insta)
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_mixed_search_results() {
        let docs = vec![
            make_symbol("validateToken", "src/auth.rs"),
            make_symbol("handler", "examples/auth/login.ts"),
            make_doc(
                "Authentication",
                "JWT token validation guide",
                "docs/auth.md",
            ),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("validate token authentication", 10);
        insta::assert_json_snapshot!(results);
    }

    #[test]
    fn snapshot_code_only_results() {
        let docs = vec![
            make_symbol("parseJSON", "src/json.rs"),
            make_symbol("parseXML", "src/xml.rs"),
            make_symbol("parseCSV", "src/csv.rs"),
        ];
        let index = InvertedIndex::build(&docs);
        let results = index.search("parse", 10);
        insta::assert_json_snapshot!(results);
    }

    // -----------------------------------------------------------------------
    // Property-based tests (proptest)
    // -----------------------------------------------------------------------

    mod property {
        use super::*;
        use proptest::prelude::*;

        fn arb_symbol() -> impl Strategy<Value = IndexDocument> {
            (
                "[a-z]{2,8}[A-Z][a-z]{2,8}",
                "[a-z]{1,4}/[a-z]{1,8}\\.[a-z]{1,3}",
            )
                .prop_map(|(name, path)| make_symbol(&name, &path))
        }

        fn arb_doc() -> impl Strategy<Value = IndexDocument> {
            (
                "[A-Z][a-z]{3,12}",
                "[a-z ]{10,50}",
                "[a-z]{1,4}/[a-z]{1,8}\\.md",
            )
                .prop_map(|(title, content, path)| make_doc(&title, &content, &path))
        }

        fn arb_docs() -> impl Strategy<Value = Vec<IndexDocument>> {
            prop::collection::vec(prop_oneof![arb_symbol(), arb_doc()], 1..50)
        }

        proptest! {
            /// All search result scores must be non-negative.
            #[test]
            fn scores_are_non_negative(
                docs in arb_docs(),
                query in "[a-z]{2,8}( [a-z]{2,8}){0,3}",
            ) {
                let index = InvertedIndex::build(&docs);
                let results = index.search(&query, 20);
                for result in &results {
                    prop_assert!(result.score() >= 0.0, "negative score: {}", result.score());
                }
            }

            /// Search result count never exceeds the limit.
            #[test]
            fn result_count_within_limit(
                docs in arb_docs(),
                query in "[a-z]{2,8}",
                limit in 1_usize..20,
            ) {
                let index = InvertedIndex::build(&docs);
                let results = index.search(&query, limit);
                prop_assert!(results.len() <= limit);
            }

            /// Results are sorted by score descending.
            #[test]
            fn results_sorted_by_score(
                docs in arb_docs(),
                query in "[a-z]{2,8}( [a-z]{2,8}){0,2}",
            ) {
                let index = InvertedIndex::build(&docs);
                let results = index.search(&query, 50);
                for window in results.windows(2) {
                    prop_assert!(
                        window[0].score() >= window[1].score(),
                        "results not sorted: {} < {}",
                        window[0].score(),
                        window[1].score(),
                    );
                }
            }
        }
    }
}
