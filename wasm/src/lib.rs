//! repoask WASM bindings for browser usage.
//!
//! Provides the same search functionality as the CLI, but operates on
//! in-memory file contents instead of filesystem paths.

use wasm_bindgen::prelude::*;

use repoask_core::index::InvertedIndex;
use repoask_core::types::IndexDocument;

/// A search index that can be built from file contents and queried.
///
/// Usage from JavaScript:
/// ```js
/// const index = new RepoIndex();
/// index.addFile("src/auth.ts", sourceCode);
/// index.addFile("README.md", readmeContent);
/// index.build();
/// const results = index.search("authentication", 10);
/// ```
#[wasm_bindgen]
pub struct RepoIndex {
    documents: Vec<IndexDocument>,
    index: Option<InvertedIndex>,
}

#[wasm_bindgen]
impl RepoIndex {
    /// Create a new empty index.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
            index: None,
        }
    }

    /// Add a file to be indexed.
    ///
    /// Call this for each file before calling `build()`.
    /// The filepath determines the parser used (e.g. `.ts` → oxc, `.md` → markdown).
    #[wasm_bindgen(js_name = "addFile")]
    pub fn add_file(&mut self, filepath: &str, content: &str) {
        let docs = repoask_parser::parse_file(filepath, content);
        self.documents.extend(docs);
    }

    /// Build the search index from all added files.
    ///
    /// Must be called after all `addFile()` calls and before `search()`.
    pub fn build(&mut self) {
        let docs = std::mem::take(&mut self.documents);
        self.index = Some(InvertedIndex::build(docs));
    }

    /// Search the index and return results as a JSON string.
    ///
    /// Returns a JSON array of search results.
    /// Each result is `{"Code": {...}}`, `{"Doc": {...}}`, or `{"Example": {...}}`.
    pub fn search(&self, query: &str, limit: usize) -> Result<String, JsError> {
        let index = self
            .index
            .as_ref()
            .ok_or_else(|| JsError::new("index not built: call build() first"))?;

        let results = index.search(query, limit);
        serde_json::to_string(&results).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return the number of documents in the index.
    #[wasm_bindgen(js_name = "docCount")]
    pub fn doc_count(&self) -> usize {
        match &self.index {
            Some(idx) => idx.doc_count(),
            None => self.documents.len(),
        }
    }
}

impl Default for RepoIndex {
    fn default() -> Self {
        Self::new()
    }
}
