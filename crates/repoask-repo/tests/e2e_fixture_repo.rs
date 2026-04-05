//! Fixture-backed end-to-end tests for parse -> index -> search.

#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use std::path::PathBuf;

    use repoask_core::index::InvertedIndex;
    use repoask_core::investigation::{InvestigationCorpus, OverviewBudget, build_overview};
    use repoask_core::types::{IndexDocument, SearchDocumentType, SearchFilters, SearchResult};
    use repoask_repo::parse::parse_directory;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-repo")
    }

    fn build_fixture_index() -> (
        Vec<IndexDocument>,
        repoask_repo::parse::ParseReport,
        InvertedIndex,
    ) {
        let (documents, report) = parse_directory(&fixture_root());
        let index = InvertedIndex::build(&documents);
        (documents, report, index)
    }

    #[test]
    fn parses_fixture_repo_end_to_end() {
        let (documents, report, index) = build_fixture_index();

        assert_eq!(report.parsed_count, 6);
        assert!(report.failed.is_empty());
        assert!(report.oversized.is_empty());
        assert_eq!(report.unsupported, vec!["notes.txt"]);

        let code_names = documents
            .iter()
            .filter_map(|document| match document {
                IndexDocument::Code(symbol) => Some(symbol.name.as_str()),
                IndexDocument::Reexport(_) | IndexDocument::Doc(_) => None,
            })
            .collect::<Vec<_>>();
        assert!(code_names.contains(&"validateToken"));
        assert!(code_names.contains(&"SessionStore"));

        let doc_titles = documents
            .iter()
            .filter_map(|document| match document {
                IndexDocument::Doc(section) => Some(section.section_title.as_str()),
                IndexDocument::Code(_) | IndexDocument::Reexport(_) => None,
            })
            .collect::<Vec<_>>();
        assert!(doc_titles.contains(&"validateToken"));
        assert!(doc_titles.contains(&"Usage"));
        assert!(!doc_titles.contains(&"Example Auth Kit"));
        assert!(!doc_titles.contains(&"API"));

        let results = index.search("validate token jwt session", 10);
        assert!(matches!(
            results.first(),
            Some(SearchResult::Code(result))
                if result.filepath == "src/auth.ts" && result.name == "validateToken"
        ));
        assert!(results.iter().any(|result| matches!(
            result,
            SearchResult::Doc(doc) if doc.filepath == "docs/guide.md" && doc.section == "validateToken"
        )));
    }

    #[test]
    fn fixture_repo_overview_surfaces_public_api_and_linked_tests() {
        let (documents, _, _) = build_fixture_index();
        let overview = build_overview(
            &InvestigationCorpus::new(documents),
            "owner/sample-repo",
            OverviewBudget::default(),
        );

        assert!(
            overview
                .public_apis
                .iter()
                .any(|api| api.signature == "validateToken(token, secret)")
        );
        assert!(
            overview
                .public_apis
                .iter()
                .any(|api| api.signature == "createSession(token, secret)")
        );
        assert!(
            overview
                .public_types
                .iter()
                .any(|public_type| public_type.signature == "Session")
        );
        assert!(
            overview
                .public_types
                .iter()
                .any(|public_type| public_type.signature == "UserSession")
        );
        assert!(
            overview
                .public_api_tests
                .iter()
                .any(|test| test.filepath == "tests/auth_test.rs")
        );
        assert!(
            overview
                .public_apis
                .iter()
                .flat_map(|api| api.tests.iter())
                .any(|test| test.filepath == "tests/auth_test.rs")
        );
        assert!(overview.entry_hints.iter().any(|hint| {
            hint.target.contains("validateToken") || hint.target.contains("createSession")
        }));
    }

    #[test]
    fn fixture_repo_filters_and_example_detection_work() {
        let (_, _, index) = build_fixture_index();

        let example_filters = SearchFilters {
            dirs: vec!["examples".to_owned()],
            exts: vec!["ts".to_owned()],
            result_type: Some(SearchDocumentType::Code),
        };
        let example_results = index.search_with_filters("demo login token", 10, &example_filters);

        assert_eq!(example_results.len(), 1);
        assert!(matches!(
            example_results.first(),
            Some(SearchResult::Code(result))
                if result.filepath == "examples/demo.ts" && result.is_example
        ));

        let doc_filters = SearchFilters {
            dirs: vec!["docs".to_owned()],
            exts: vec!["md".to_owned()],
            result_type: Some(SearchDocumentType::Doc),
        };
        let doc_results = index.search_with_filters("refresh token usage", 10, &doc_filters);

        assert!(
            doc_results
                .iter()
                .all(|result| matches!(result, SearchResult::Doc(_)))
        );
        assert!(doc_results.iter().any(|result| matches!(
            result,
            SearchResult::Doc(doc) if doc.filepath == "docs/guide.md" && doc.section == "Usage"
        )));
    }
}
