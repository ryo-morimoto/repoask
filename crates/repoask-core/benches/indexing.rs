//! Index-build benchmarks for `repoask-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "benchmark harness")]

use divan::Bencher;
use repoask_core::index::InvertedIndex;
use repoask_core::types::{
    CommentInfo, CommentSource, DocSection, ExportInfo, IndexDocument, Symbol, SymbolKind,
};

fn main() {
    divan::main();
}

fn generate_docs(n: usize) -> Vec<IndexDocument> {
    (0..n)
        .map(|i| {
            if i % 3 == 0 {
                IndexDocument::Doc(DocSection {
                    filepath: format!("docs/guide_{}.md", i % 20),
                    section_title: format!("Section {i}: Configuration"),
                    heading_hierarchy: vec!["Guide".to_owned(), format!("Part {}", i % 5)],
                    content: format!(
                        "This section covers configuration option {i}. \
                         Use the validate function to check parameters."
                    ),
                    code_symbols: vec![format!("validate_config_{i}")],
                    start_line: 1,
                    end_line: 20,
                })
            } else {
                IndexDocument::Code(Symbol {
                    name: format!("validateItem{i}"),
                    kind: SymbolKind::Function,
                    filepath: format!("src/module_{}/handler.rs", i % 50),
                    start_line: 1,
                    end_line: 30,
                    params: vec![format!("item_id_{i}"), "context".to_owned()],
                    signature_preview: Some(format!("validateItem{i}(item_id_{i}, context)")),
                    comment: CommentInfo::from_normalized_text(
                        &format!("Validates item number {i}"),
                        CommentSource::PlainComment,
                    ),
                    export: ExportInfo::default(),
                })
            }
        })
        .collect()
}

#[divan::bench(args = [100, 1_000, 5_000, 10_000])]
fn index_build(bencher: Bencher, n: usize) {
    let docs = generate_docs(n);
    bencher
        .with_inputs(|| docs.clone())
        .bench_values(|docs| InvertedIndex::build(&docs));
}
