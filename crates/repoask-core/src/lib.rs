//! Core search engine: BM25 scoring, inverted index, tokenization, and shared types.

/// BM25 scoring algorithm implementation.
pub mod bm25;
/// Inverted index construction and search.
pub mod index;
/// Text tokenization for identifiers, queries, and natural language.
pub mod tokenizer;
/// Shared types for index documents, search results, and symbols.
pub mod types;
