//! Search benchmarks for `repoask-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "benchmark harness")]

use divan::Bencher;
use repoask_core::index::InvertedIndex;
use repoask_core::types::{IndexDocument, SearchResult, Symbol, SymbolKind};

fn main() {
    divan::main();
}

fn build_index(n: usize) -> InvertedIndex {
    let docs: Vec<IndexDocument> = (0..n)
        .map(|i| {
            IndexDocument::Code(Symbol {
                name: format!("function_{i}"),
                kind: SymbolKind::Function,
                filepath: format!("src/mod_{}/lib.rs", i % 100),
                start_line: 1,
                end_line: 20,
                doc_comment: Some(format!("Process request for handler {i}")),
                params: vec![format!("req_{i}"), "ctx".to_owned()],
            })
        })
        .collect();
    InvertedIndex::build(&docs)
}

const QUERIES: &[&str] = &[
    "function process request",
    "handler context",
    "validate token",
    "middleware jwt",
];

#[divan::bench(args = [1_000, 5_000, 10_000])]
fn search_latency(bencher: Bencher, n: usize) {
    let index = build_index(n);
    bencher.bench(|| {
        for query in QUERIES {
            divan::black_box(index.search(divan::black_box(query), 10));
        }
    });
}

#[divan::bench(args = QUERIES)]
fn search_single_query(query: &str) -> Vec<SearchResult> {
    static INDEX: std::sync::LazyLock<InvertedIndex> =
        std::sync::LazyLock::new(|| build_index(10_000));
    INDEX.search(query, 10)
}
