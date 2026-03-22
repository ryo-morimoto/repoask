use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use crate::bm25::Bm25Scorer;
use crate::tokenizer::{tokenize_identifier, tokenize_query, tokenize_text};
use crate::types::{
    CodeResult, DocId, DocResult, ExampleResult, FIELD_DOC_CONTENT, FIELD_FILEPATH, FIELD_PARAMS,
    FIELD_SYMBOL_NAME, FieldId, FieldStats, IndexDocument, NUM_FIELDS, Posting, SearchResult,
};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvertedIndex {
    postings: HashMap<String, Vec<Posting>>,
    documents: Vec<StoredDoc>,
    field_lengths: Vec<[u16; NUM_FIELDS]>,
    field_stats: [FieldStats; NUM_FIELDS],
}

/// Helper for min-heap top-k selection. Ordered by score ascending (min first),
/// with ties broken by doc_id descending so the heap evicts the least desirable.
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
    pub fn build(docs: Vec<IndexDocument>) -> Self {
        let mut index = Self {
            postings: HashMap::new(),
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

        index
    }

    fn add_document(&mut self, doc: IndexDocument, term_freq_buf: &mut HashMap<String, u16>) {
        let doc_id = self.documents.len() as DocId;
        let mut lengths = [0u16; NUM_FIELDS];

        match &doc {
            IndexDocument::Code(symbol) => {
                let is_example = is_example_path(&symbol.filepath);

                // Field 0: symbol name tokens
                let name_tokens = tokenize_identifier(&symbol.name);
                lengths[FIELD_SYMBOL_NAME as usize] = name_tokens.len() as u16;
                self.add_field_tokens(doc_id, FIELD_SYMBOL_NAME, &name_tokens, term_freq_buf);

                // Field 1: doc comment tokens
                if let Some(ref comment) = symbol.doc_comment {
                    let comment_tokens = tokenize_text(comment);
                    lengths[FIELD_DOC_CONTENT as usize] = comment_tokens.len() as u16;
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
                lengths[FIELD_PARAMS as usize] = param_tokens.len() as u16;
                self.add_field_tokens(doc_id, FIELD_PARAMS, &param_tokens, term_freq_buf);

                // Field 3: filepath tokens
                let path_tokens = tokenize_identifier(&symbol.filepath);
                lengths[FIELD_FILEPATH as usize] = path_tokens.len() as u16;
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
                lengths[FIELD_SYMBOL_NAME as usize] = heading_tokens.len() as u16;
                self.add_field_tokens(doc_id, FIELD_SYMBOL_NAME, &heading_tokens, term_freq_buf);

                // Field 1: body content tokens
                let body_tokens = tokenize_text(&section.content);
                lengths[FIELD_DOC_CONTENT as usize] = body_tokens.len() as u16;
                self.add_field_tokens(doc_id, FIELD_DOC_CONTENT, &body_tokens, term_freq_buf);

                // Field 2: code symbols extracted from fenced blocks
                let code_sym_tokens: Vec<String> = section
                    .code_symbols
                    .iter()
                    .flat_map(|s| tokenize_identifier(s))
                    .collect();
                lengths[FIELD_PARAMS as usize] = code_sym_tokens.len() as u16;
                self.add_field_tokens(doc_id, FIELD_PARAMS, &code_sym_tokens, term_freq_buf);

                // Field 3: filepath tokens
                let path_tokens = tokenize_identifier(&section.filepath);
                lengths[FIELD_FILEPATH as usize] = path_tokens.len() as u16;
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
            self.field_stats[i].total_length += len as u64;
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

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = tokenize_query(query);
        if query_tokens.is_empty() {
            return vec![];
        }

        let scorer = Bm25Scorer::new();
        let total_docs = self.documents.len() as u32;
        let mut doc_scores: HashMap<DocId, f32> = HashMap::new();

        for token in &query_tokens {
            let Some(postings) = self.postings.get(token.as_str()) else {
                continue;
            };

            let doc_freq = postings
                .iter()
                .map(|p| p.doc_id)
                .collect::<std::collections::HashSet<_>>()
                .len() as u32;

            for posting in postings {
                let field_length =
                    self.field_lengths[posting.doc_id as usize][posting.field_id as usize];
                let score = scorer.score(
                    posting.term_freq,
                    field_length,
                    posting.field_id,
                    &self.field_stats[posting.field_id as usize],
                    doc_freq,
                    total_docs,
                );
                *doc_scores.entry(posting.doc_id).or_insert(0.0) += score;
            }
        }

        // Top-k selection using a min-heap of size `limit` — O(n log k).
        let mut heap: BinaryHeap<Reverse<ScoredDoc>> = BinaryHeap::with_capacity(limit + 1);
        for (doc_id, score) in doc_scores {
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

    fn to_search_result(&self, doc_id: DocId, score: f32) -> SearchResult {
        match &self.documents[doc_id as usize] {
            StoredDoc::Code {
                filepath,
                name,
                kind,
                start_line,
                end_line,
                is_example: true,
            } => SearchResult::Example(ExampleResult {
                filepath: filepath.clone(),
                name: name.clone(),
                kind: *kind,
                start_line: *start_line,
                end_line: *end_line,
                score,
            }),
            StoredDoc::Code {
                filepath,
                name,
                kind,
                start_line,
                end_line,
                is_example: false,
            } => SearchResult::Code(CodeResult {
                filepath: filepath.clone(),
                name: name.clone(),
                kind: *kind,
                start_line: *start_line,
                end_line: *end_line,
                score,
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

    pub fn doc_count(&self) -> usize {
        self.documents.len()
    }
}

fn is_example_path(filepath: &str) -> bool {
    let lower = filepath.to_lowercase();
    lower.contains("example") || lower.contains("sample") || lower.contains("demo")
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
            name: name.to_string(),
            kind: SymbolKind::Function,
            filepath: filepath.to_string(),
            start_line: 1,
            end_line: 10,
            doc_comment: None,
            params: vec![],
        })
    }

    fn make_doc(title: &str, content: &str, filepath: &str) -> IndexDocument {
        IndexDocument::Doc(DocSection {
            filepath: filepath.to_string(),
            section_title: title.to_string(),
            heading_hierarchy: vec![],
            content: content.to_string(),
            code_symbols: vec![],
            start_line: 1,
            end_line: 20,
        })
    }

    #[test]
    fn test_empty_index() {
        let index = InvertedIndex::build(vec![]);
        let results = index.search("anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let index = InvertedIndex::build(vec![make_symbol("foo", "src/foo.rs")]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_symbol_name_match() {
        let index = InvertedIndex::build(vec![
            make_symbol("validateToken", "src/auth.rs"),
            make_symbol("parseJSON", "src/json.rs"),
        ]);
        let results = index.search("validate token", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].filepath(), "src/auth.rs");
    }

    #[test]
    fn test_doc_section_match() {
        let index = InvertedIndex::build(vec![
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
        ]);
        let results = index.search("authentication jwt", 10);
        assert!(!results.is_empty());
        match &results[0] {
            SearchResult::Doc(r) => assert_eq!(r.section, "Authentication"),
            _ => panic!("Expected Doc result"),
        }
    }

    #[test]
    fn test_symbol_name_ranked_higher_than_filepath() {
        let index = InvertedIndex::build(vec![
            make_symbol("createUser", "src/auth/validate.rs"),
            make_symbol("validateToken", "src/token.rs"),
        ]);
        let results = index.search("validate", 10);
        // validateToken (name match) should rank above createUser (path match)
        assert_eq!(results[0].filepath(), "src/token.rs");
    }

    #[test]
    fn test_example_detection() {
        let index = InvertedIndex::build(vec![make_symbol("handler", "examples/auth/login.ts")]);
        let results = index.search("handler", 10);
        assert!(matches!(results[0], SearchResult::Example(_)));
    }

    #[test]
    fn test_mixed_code_and_doc() {
        let index = InvertedIndex::build(vec![
            make_symbol("authenticate", "src/auth.rs"),
            make_doc(
                "Authentication Guide",
                "Learn how to authenticate users",
                "docs/auth.md",
            ),
        ]);
        let results = index.search("authenticate", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_limit() {
        let docs: Vec<IndexDocument> = (0..100)
            .map(|i| make_symbol(&format!("validateItem{i}"), &format!("src/f{i}.rs")))
            .collect();
        let index = InvertedIndex::build(docs);
        let results = index.search("validate", 5);
        assert_eq!(results.len(), 5);
    }

    // -----------------------------------------------------------------------
    // Snapshot tests (insta)
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_mixed_search_results() {
        let index = InvertedIndex::build(vec![
            make_symbol("validateToken", "src/auth.rs"),
            make_symbol("handler", "examples/auth/login.ts"),
            make_doc(
                "Authentication",
                "JWT token validation guide",
                "docs/auth.md",
            ),
        ]);
        let results = index.search("validate token authentication", 10);
        insta::assert_json_snapshot!(results);
    }

    #[test]
    fn snapshot_code_only_results() {
        let index = InvertedIndex::build(vec![
            make_symbol("parseJSON", "src/json.rs"),
            make_symbol("parseXML", "src/xml.rs"),
            make_symbol("parseCSV", "src/csv.rs"),
        ]);
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
                let index = InvertedIndex::build(docs);
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
                let index = InvertedIndex::build(docs);
                let results = index.search(&query, limit);
                prop_assert!(results.len() <= limit);
            }

            /// Results are sorted by score descending.
            #[test]
            fn results_sorted_by_score(
                docs in arb_docs(),
                query in "[a-z]{2,8}( [a-z]{2,8}){0,2}",
            ) {
                let index = InvertedIndex::build(docs);
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
