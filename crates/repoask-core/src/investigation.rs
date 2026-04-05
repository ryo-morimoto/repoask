//! Investigation-specific shared types for overview and future inspect surfaces.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tokenizer::tokenize_identifier;
use crate::types::{CommentFlag, ExportKind, IndexDocument, Publicness, Reexport, SymbolKind};

/// A persisted corpus used by investigation surfaces such as `overview`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationCorpus {
    /// Rich parser output retained for non-search investigation features.
    pub documents: Vec<IndexDocument>,
    /// Module-resolution hints used for re-export resolution.
    pub module_resolution: ModuleResolutionConfig,
}

impl InvestigationCorpus {
    /// Build a persisted investigation corpus from parser documents.
    #[must_use]
    pub fn new(documents: Vec<IndexDocument>) -> Self {
        Self {
            documents,
            module_resolution: ModuleResolutionConfig::default(),
        }
    }

    /// Build a persisted investigation corpus with module-resolution hints.
    #[must_use]
    pub const fn with_module_resolution(
        documents: Vec<IndexDocument>,
        module_resolution: ModuleResolutionConfig,
    ) -> Self {
        Self {
            documents,
            module_resolution,
        }
    }
}

/// Module-resolution hints used when resolving re-export source specifiers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleResolutionConfig {
    /// Optional TypeScript baseUrl resolved to a repo-relative directory.
    pub tsconfig_base_url: Option<String>,
    /// Optional TypeScript path alias rules resolved to repo-relative targets.
    pub tsconfig_paths: Vec<PathAliasRule>,
    /// Nested or scoped config overrides ordered independently from the root config.
    pub scoped_configs: Vec<ScopedModuleResolution>,
}

/// A single TypeScript path alias rule from `compilerOptions.paths`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathAliasRule {
    /// Alias pattern, such as `@/*` or `@lib`.
    pub pattern: String,
    /// Repo-relative target patterns, such as `src/*`.
    pub targets: Vec<String>,
}

/// A module-resolution config scoped to a repo-relative directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedModuleResolution {
    /// Repo-relative directory to which this config applies.
    pub scope_dir: String,
    /// Optional baseUrl for this scoped config.
    pub tsconfig_base_url: Option<String>,
    /// Optional path alias rules for this scoped config.
    pub tsconfig_paths: Vec<PathAliasRule>,
}

/// Repo-level overview response for public-surface-first investigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationOverview {
    /// Top-level repo and coverage metadata.
    pub meta: OverviewMeta,
    /// Public API entrypoint cards.
    pub public_apis: Vec<PublicApiCard>,
    /// Public type entrypoint cards.
    pub public_types: Vec<PublicTypeCard>,
    /// Tests linked to the surfaced public APIs.
    pub public_api_tests: Vec<TestCard>,
    /// Next-step navigation hints.
    pub entry_hints: Vec<HintCard>,
}

/// High-level metadata about an overview payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewMeta {
    /// Repo spec or repo label rendered with the overview.
    pub repo: String,
    /// Coverage and confidence notes for Step 1 metadata.
    pub coverage: CoverageSummary,
    /// Whether the result was truncated by a render budget.
    pub truncated: bool,
}

/// Summary of metadata coverage for an overview response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Coverage of publicness detection.
    pub publicness: CoverageStatus,
    /// Coverage of comment normalization.
    pub comment_normalization: CoverageStatus,
    /// Coverage of test linkage.
    pub test_linkage: CoverageStatus,
    /// Human-readable notes describing partial coverage or truncation.
    pub notes: Vec<String>,
}

/// Coverage state for a metadata dimension.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CoverageStatus {
    /// Coverage is complete for the relevant surfaced items.
    Complete,
    /// Coverage exists but is partial.
    Partial,
    /// Coverage is unsupported or unavailable.
    Unsupported,
}

/// Overview card representing a surfaced public API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicApiCard {
    /// Stable symbol reference for follow-up inspection.
    pub symbol_ref: String,
    /// Repo-relative path of the source symbol.
    pub filepath: String,
    /// Compact signature preview.
    pub signature: String,
    /// Optional one-line comment summary.
    pub comment_summary: Option<String>,
    /// Important flags derived from the comment.
    pub flags: Vec<CommentFlag>,
    /// Publicness classification for the symbol.
    pub publicness: Publicness,
    /// Top linked tests for this API.
    pub tests: Vec<TestLink>,
    /// Deterministic ranking score.
    pub score: f32,
}

/// Overview card representing a surfaced public type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTypeCard {
    /// Stable symbol reference for follow-up inspection.
    pub symbol_ref: String,
    /// Repo-relative path of the source symbol.
    pub filepath: String,
    /// The kind of surfaced type.
    pub kind: SymbolKind,
    /// Compact signature preview.
    pub signature: String,
    /// Optional one-line comment summary.
    pub comment_summary: Option<String>,
    /// Important flags derived from the comment.
    pub flags: Vec<CommentFlag>,
    /// Publicness classification for the symbol.
    pub publicness: Publicness,
    /// Deterministic ranking score.
    pub score: f32,
}

/// Overview card representing a linked test entrypoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCard {
    /// Stable test reference.
    pub test_ref: String,
    /// Repo-relative path of the test.
    pub filepath: String,
    /// Human-readable test display name.
    pub display_name: String,
    /// Linked public symbols for this test.
    pub linked_symbols: Vec<String>,
    /// Evidence explaining why this test was linked.
    pub reasons: Vec<TestLinkReason>,
    /// Deterministic ranking score.
    pub score: f32,
}

/// Test linkage metadata attached to a public API card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestLink {
    /// Stable test reference.
    pub test_ref: String,
    /// Repo-relative path of the test.
    pub filepath: String,
    /// Human-readable test display name.
    pub display_name: String,
    /// Evidence explaining why this test was linked.
    pub reasons: Vec<TestLinkReason>,
    /// Deterministic linkage score.
    pub score: f32,
}

/// Explainable reasons for test linkage.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TestLinkReason {
    /// The test path includes symbol-name tokens.
    TestPathMatch,
    /// The extracted test name includes symbol-name tokens.
    TestNameMatch,
    /// The test file stem matches the target file stem.
    FileStemMatch,
    /// The test path sits near the target source path.
    DirectoryProximity,
}

/// Navigation hint for follow-up investigation actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HintCard {
    /// The structured action this hint represents.
    pub action: HintAction,
    /// The target argument or symbol reference for the action.
    pub target: String,
    /// Short human-readable label.
    pub label: String,
    /// Explanation for why the hint was surfaced.
    pub reason: String,
}

/// Structured follow-up actions for overview hints.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HintAction {
    /// Follow the symbol through a future inspect surface.
    InspectSymbol,
    /// Run a targeted search query.
    SearchQuery,
    /// Read a specific file path directly.
    ReadPath,
}

/// Render budget for the Step 1 overview surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewBudget {
    /// Maximum number of surfaced public APIs.
    pub max_apis: usize,
    /// Maximum number of surfaced public types.
    pub max_types: usize,
    /// Maximum number of surfaced linked tests.
    pub max_tests: usize,
    /// Maximum number of surfaced hints.
    pub max_hints: usize,
    /// Maximum characters retained from comment previews.
    pub max_comment_chars: usize,
    /// Maximum total rendered characters for the overview.
    pub max_total_chars: usize,
}

impl Default for OverviewBudget {
    fn default() -> Self {
        Self {
            max_apis: 6,
            max_types: 6,
            max_tests: 6,
            max_hints: 4,
            max_comment_chars: 140,
            max_total_chars: 6_000,
        }
    }
}

/// Stable surface kind used in investigation references.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    /// A public API card.
    Api,
    /// A public type card.
    Type,
    /// A test card.
    Test,
    /// A generic symbol reference.
    Symbol,
}

impl SurfaceKind {
    /// Return the stable string prefix for this surface kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Type => "type",
            Self::Test => "test",
            Self::Symbol => "symbol",
        }
    }
}

/// Build a stable repo-snapshot reference for an investigation surface.
#[must_use]
pub fn build_surface_ref(kind: SurfaceKind, filepath: &str, name: &str, start_line: u32) -> String {
    format!(
        "{}:{}#{}@L{}",
        kind.as_str(),
        normalize_surface_path(filepath),
        name,
        start_line,
    )
}

fn normalize_surface_path(filepath: &str) -> String {
    filepath.replace('\\', "/")
}

/// Build a deterministic Step 1 overview from a persisted corpus.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "aggregates all Step 1 overview sections in one place"
)]
pub fn build_overview(
    corpus: &InvestigationCorpus,
    repo: &str,
    budget: OverviewBudget,
) -> InvestigationOverview {
    let symbols = corpus
        .documents
        .iter()
        .filter_map(|document| match document {
            IndexDocument::Code(symbol) => Some(symbol),
            IndexDocument::Reexport(_) | IndexDocument::Doc(_) => None,
        })
        .collect::<Vec<_>>();
    let reexports = corpus
        .documents
        .iter()
        .filter_map(|document| match document {
            IndexDocument::Reexport(reexport) => Some(reexport),
            IndexDocument::Code(_) | IndexDocument::Doc(_) => None,
        })
        .collect::<Vec<_>>();
    let test_candidates = symbols
        .iter()
        .copied()
        .filter(|symbol| is_test_candidate(symbol.filepath.as_str(), symbol.name.as_str()))
        .collect::<Vec<_>>();

    let mut public_apis = symbols
        .iter()
        .copied()
        .filter(|symbol| is_public_api_candidate(symbol.kind, symbol.export.publicness))
        .map(|symbol| build_public_api_card(symbol, &test_candidates, budget.max_comment_chars))
        .collect::<Vec<_>>();

    let mut public_types = symbols
        .iter()
        .copied()
        .filter(|symbol| is_public_type_candidate(symbol.kind, symbol.export.publicness))
        .map(|symbol| build_public_type_card(symbol, budget.max_comment_chars))
        .collect::<Vec<_>>();

    public_apis.extend(build_reexport_api_cards(
        &reexports,
        &symbols,
        &test_candidates,
        &corpus.module_resolution,
        budget.max_comment_chars,
    ));
    public_types.extend(build_reexport_type_cards(
        &reexports,
        &symbols,
        &corpus.module_resolution,
        budget.max_comment_chars,
    ));

    let used_unknown_fallback = public_apis.is_empty() && public_types.is_empty();
    if used_unknown_fallback {
        public_apis = symbols
            .iter()
            .copied()
            .filter(|symbol| is_unknown_api_candidate(symbol.kind, symbol.export.publicness))
            .map(|symbol| build_public_api_card(symbol, &test_candidates, budget.max_comment_chars))
            .collect();
        public_types = symbols
            .iter()
            .copied()
            .filter(|symbol| is_unknown_type_candidate(symbol.kind, symbol.export.publicness))
            .map(|symbol| build_public_type_card(symbol, budget.max_comment_chars))
            .collect();
    }

    sort_by_score_key(&mut public_apis, |card| {
        (&card.filepath, &card.symbol_ref, card.score)
    });
    sort_by_score_key(&mut public_types, |card| {
        (&card.filepath, &card.symbol_ref, card.score)
    });

    let mut public_api_tests = collect_public_api_tests(&public_apis);
    sort_by_score_key(&mut public_api_tests, |card| {
        (&card.filepath, &card.test_ref, card.score)
    });

    let mut truncated = public_apis.len() > budget.max_apis;
    if truncated {
        public_apis.truncate(budget.max_apis);
    }
    if public_types.len() > budget.max_types {
        public_types.truncate(budget.max_types);
        truncated = true;
    }
    if public_api_tests.len() > budget.max_tests {
        public_api_tests.truncate(budget.max_tests);
        truncated = true;
    }

    let mut entry_hints = build_entry_hints(&public_apis, &public_types, &public_api_tests);
    if entry_hints.len() > budget.max_hints {
        entry_hints.truncate(budget.max_hints);
        truncated = true;
    }

    let mut coverage = build_coverage_summary(&symbols, &public_apis);
    if used_unknown_fallback && (!public_apis.is_empty() || !public_types.is_empty()) {
        coverage
            .notes
            .push("overview fell back to symbols with unknown publicness".to_owned());
    }
    if truncated {
        coverage
            .notes
            .push("overview output truncated by render budget".to_owned());
    }

    InvestigationOverview {
        meta: OverviewMeta {
            repo: repo.to_owned(),
            coverage,
            truncated,
        },
        public_apis,
        public_types,
        public_api_tests,
        entry_hints,
    }
}

fn build_public_api_card(
    symbol: &crate::types::Symbol,
    test_candidates: &[&crate::types::Symbol],
    max_comment_chars: usize,
) -> PublicApiCard {
    let tests = link_tests(symbol, test_candidates);
    PublicApiCard {
        symbol_ref: build_surface_ref(
            SurfaceKind::Api,
            &symbol.filepath,
            &symbol.name,
            symbol.start_line,
        ),
        filepath: normalize_surface_path(&symbol.filepath),
        signature: symbol
            .signature_preview
            .clone()
            .unwrap_or_else(|| symbol.name.clone()),
        comment_summary: truncated_comment_summary(symbol, max_comment_chars),
        flags: symbol
            .comment
            .as_ref()
            .map_or_else(Vec::new, |comment| comment.flags.clone()),
        publicness: symbol.export.publicness,
        score: api_score(symbol, tests.len()),
        tests,
    }
}

fn build_reexport_api_cards(
    reexports: &[&Reexport],
    symbols: &[&crate::types::Symbol],
    test_candidates: &[&crate::types::Symbol],
    module_resolution: &ModuleResolutionConfig,
    max_comment_chars: usize,
) -> Vec<PublicApiCard> {
    let build_api_card = |reexport: &Reexport, resolved: &crate::types::Symbol| PublicApiCard {
        symbol_ref: build_surface_ref(
            SurfaceKind::Api,
            &reexport.filepath,
            reexport_card_name(reexport, resolved),
            reexport.start_line,
        ),
        filepath: normalize_surface_path(&reexport.filepath),
        signature: build_reexport_signature(resolved, reexport_card_name(reexport, resolved)),
        comment_summary: truncated_comment_summary(resolved, max_comment_chars),
        flags: resolved
            .comment
            .as_ref()
            .map_or_else(Vec::new, |comment| comment.flags.clone()),
        publicness: Publicness::Reexported,
        tests: link_tests(resolved, test_candidates),
        score: reexport_api_score(reexport, resolved, test_candidates),
    };

    build_reexport_cards(
        reexports,
        symbols,
        module_resolution,
        |reexport, resolved| {
            (resolved.kind == SymbolKind::Function).then(|| build_api_card(reexport, resolved))
        },
    )
}

fn build_public_type_card(
    symbol: &crate::types::Symbol,
    max_comment_chars: usize,
) -> PublicTypeCard {
    let symbol_ref = build_surface_ref(
        SurfaceKind::Type,
        &symbol.filepath,
        &symbol.name,
        symbol.start_line,
    );
    let filepath = normalize_surface_path(&symbol.filepath);
    let signature = symbol
        .signature_preview
        .clone()
        .unwrap_or_else(|| symbol.name.clone());
    let flags = symbol
        .comment
        .as_ref()
        .map_or_else(Vec::new, |comment| comment.flags.clone());

    PublicTypeCard {
        symbol_ref,
        filepath,
        kind: symbol.kind,
        signature,
        comment_summary: truncated_comment_summary(symbol, max_comment_chars),
        flags,
        publicness: symbol.export.publicness,
        score: type_score(symbol),
    }
}

fn build_reexport_type_cards(
    reexports: &[&Reexport],
    symbols: &[&crate::types::Symbol],
    module_resolution: &ModuleResolutionConfig,
    max_comment_chars: usize,
) -> Vec<PublicTypeCard> {
    let mut cards = Vec::new();
    let build_reexport_type =
        |reexport: &Reexport, resolved: &crate::types::Symbol| PublicTypeCard {
            symbol_ref: build_surface_ref(
                SurfaceKind::Type,
                &reexport.filepath,
                reexport_card_name(reexport, resolved),
                reexport.start_line,
            ),
            filepath: normalize_surface_path(&reexport.filepath),
            kind: resolved.kind,
            signature: reexport_card_name(reexport, resolved).to_owned(),
            comment_summary: truncated_comment_summary(resolved, max_comment_chars),
            flags: resolved
                .comment
                .as_ref()
                .map_or_else(Vec::new, |comment| comment.flags.clone()),
            publicness: Publicness::Reexported,
            score: reexport_type_score(reexport, resolved),
        };

    for reexport in reexports {
        if is_namespace_reexport(reexport) {
            cards.push(PublicTypeCard {
                symbol_ref: build_surface_ref(
                    SurfaceKind::Type,
                    &reexport.filepath,
                    &reexport.exported_name,
                    reexport.start_line,
                ),
                filepath: normalize_surface_path(&reexport.filepath),
                kind: SymbolKind::Module,
                signature: reexport.exported_name.clone(),
                comment_summary: reexport
                    .source_specifier
                    .as_ref()
                    .map(|source| format!("namespace re-export from {source}")),
                flags: vec![],
                publicness: Publicness::Reexported,
                score: 2.1,
            });
            continue;
        }

        cards.extend(build_reexport_cards(
            std::slice::from_ref(reexport),
            symbols,
            module_resolution,
            |reexport, resolved| {
                is_public_type_kind(resolved.kind).then(|| build_reexport_type(reexport, resolved))
            },
        ));
    }

    cards
}

fn build_reexport_cards<T, F>(
    reexports: &[&Reexport],
    symbols: &[&crate::types::Symbol],
    module_resolution: &ModuleResolutionConfig,
    mut build: F,
) -> Vec<T>
where
    F: FnMut(&Reexport, &crate::types::Symbol) -> Option<T>,
{
    let mut cards = Vec::new();

    for reexport in reexports {
        if is_wildcard_reexport(reexport) {
            for resolved in resolve_wildcard_reexport_targets(reexport, symbols, module_resolution)
            {
                if let Some(card) = build(reexport, resolved) {
                    cards.push(card);
                }
            }
            continue;
        }

        let Some(resolved) = resolve_reexport_target(reexport, symbols, module_resolution) else {
            continue;
        };
        if let Some(card) = build(reexport, resolved) {
            cards.push(card);
        }
    }

    cards
}

fn reexport_card_name<'a>(reexport: &'a Reexport, resolved: &'a crate::types::Symbol) -> &'a str {
    if is_wildcard_reexport(reexport) {
        &resolved.name
    } else {
        &reexport.exported_name
    }
}

fn link_tests(
    symbol: &crate::types::Symbol,
    test_candidates: &[&crate::types::Symbol],
) -> Vec<TestLink> {
    let symbol_tokens = tokenize_identifier(&symbol.name);
    let target_stem = normalized_file_stem(&symbol.filepath);
    let target_parent = normalized_parent_dir(&symbol.filepath);
    let mut links = test_candidates
        .iter()
        .filter_map(|candidate| {
            let mut reasons = Vec::new();
            let candidate_path = normalize_surface_path(&candidate.filepath);
            let candidate_path_lower = candidate_path.to_ascii_lowercase();
            let candidate_name_tokens = tokenize_identifier(&candidate.name);

            if symbol_tokens
                .iter()
                .all(|token| candidate_path_lower.contains(token))
            {
                reasons.push(TestLinkReason::TestPathMatch);
            }
            if symbol_tokens.iter().all(|token| {
                candidate_name_tokens
                    .iter()
                    .any(|candidate_token| candidate_token == token)
            }) {
                reasons.push(TestLinkReason::TestNameMatch);
            }
            if target_stem == normalized_file_stem(&candidate.filepath) {
                reasons.push(TestLinkReason::FileStemMatch);
            }
            if !target_parent.is_empty()
                && target_parent == normalized_parent_dir(&candidate.filepath)
            {
                reasons.push(TestLinkReason::DirectoryProximity);
            }

            if reasons.is_empty() {
                return None;
            }

            Some(TestLink {
                test_ref: build_surface_ref(
                    SurfaceKind::Test,
                    &candidate.filepath,
                    &candidate.name,
                    candidate.start_line,
                ),
                filepath: candidate_path,
                display_name: candidate.name.clone(),
                score: saturating_f32(reasons.len()) + 0.5,
                reasons,
            })
        })
        .collect::<Vec<_>>();

    sort_by_score_key(&mut links, |card| {
        (&card.filepath, &card.test_ref, card.score)
    });
    links.truncate(3);
    links
}

fn collect_public_api_tests(public_apis: &[PublicApiCard]) -> Vec<TestCard> {
    let mut deduped = std::collections::HashMap::<String, TestCard>::new();

    for api in public_apis {
        for test in &api.tests {
            deduped
                .entry(test.test_ref.clone())
                .and_modify(|existing| {
                    if !existing.linked_symbols.contains(&api.symbol_ref) {
                        existing.linked_symbols.push(api.symbol_ref.clone());
                    }
                    for reason in &test.reasons {
                        if !existing.reasons.contains(reason) {
                            existing.reasons.push(*reason);
                        }
                    }
                    existing.score = existing.score.max(test.score);
                })
                .or_insert_with(|| TestCard {
                    test_ref: test.test_ref.clone(),
                    filepath: test.filepath.clone(),
                    display_name: test.display_name.clone(),
                    linked_symbols: vec![api.symbol_ref.clone()],
                    reasons: test.reasons.clone(),
                    score: test.score,
                });
        }
    }

    deduped.into_values().collect()
}

fn build_entry_hints(
    public_apis: &[PublicApiCard],
    public_types: &[PublicTypeCard],
    public_api_tests: &[TestCard],
) -> Vec<HintCard> {
    let mut hints = Vec::new();

    for api in public_apis.iter().take(2) {
        hints.push(HintCard {
            action: HintAction::InspectSymbol,
            target: api.symbol_ref.clone(),
            label: api.signature.clone(),
            reason: "top public API".to_owned(),
        });
    }
    for public_type in public_types.iter().take(1) {
        hints.push(HintCard {
            action: HintAction::InspectSymbol,
            target: public_type.symbol_ref.clone(),
            label: public_type.signature.clone(),
            reason: "top public type".to_owned(),
        });
    }
    for test in public_api_tests.iter().take(1) {
        hints.push(HintCard {
            action: HintAction::ReadPath,
            target: test.filepath.clone(),
            label: test.display_name.clone(),
            reason: "top linked test".to_owned(),
        });
    }

    hints
}

fn resolve_reexport_target<'a>(
    reexport: &Reexport,
    symbols: &'a [&crate::types::Symbol],
    module_resolution: &ModuleResolutionConfig,
) -> Option<&'a crate::types::Symbol> {
    let candidate_paths = reexport_candidate_paths(reexport, module_resolution);
    for candidate in candidate_paths {
        if let Some(symbol) = symbols.iter().copied().find(|symbol| {
            reexport_local_matches_symbol(reexport, symbol)
                && candidate == normalize_surface_path(&symbol.filepath)
        }) {
            return Some(symbol);
        }
    }
    None
}

fn reexport_local_matches_symbol(reexport: &Reexport, symbol: &crate::types::Symbol) -> bool {
    symbol.name == reexport.local_name
        || (reexport.local_name == "default" && symbol.export.export_kind == ExportKind::Default)
}

fn reexport_candidate_paths(
    reexport: &Reexport,
    module_resolution: &ModuleResolutionConfig,
) -> Vec<String> {
    reexport.source_specifier.as_ref().map_or_else(
        || vec![normalize_surface_path(&reexport.filepath)],
        |source_specifier| {
            resolve_module_specifier(&reexport.filepath, source_specifier, module_resolution)
        },
    )
}

fn resolve_module_specifier(
    filepath: &str,
    source_specifier: &str,
    module_resolution: &ModuleResolutionConfig,
) -> Vec<String> {
    let mut candidates = if source_specifier.starts_with('.') {
        let base_dir = normalized_parent_dir(filepath);
        vec![normalize_relative_path(&base_dir, source_specifier)]
    } else {
        resolve_path_alias_candidates(filepath, source_specifier, module_resolution)
    };

    if !source_specifier.starts_with('.') {
        for scoped in scoped_configs_for_filepath(filepath, module_resolution) {
            if let Some(base_url) = &scoped.tsconfig_base_url {
                candidates.push(normalize_relative_path(base_url, source_specifier));
            }
        }
        if let Some(base_url) = &module_resolution.tsconfig_base_url {
            candidates.push(normalize_relative_path(base_url, source_specifier));
        }
    }

    let mut expanded = Vec::new();
    for candidate in candidates {
        let raw_path = candidate.to_string_lossy().replace('\\', "/");
        expanded.extend(expand_module_candidate(&raw_path));
    }
    expanded.sort_unstable();
    expanded.dedup();
    expanded
}

fn resolve_path_alias_candidates(
    filepath: &str,
    source_specifier: &str,
    module_resolution: &ModuleResolutionConfig,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for scoped in scoped_configs_for_filepath(filepath, module_resolution) {
        for rule in &scoped.tsconfig_paths {
            let Some(wildcard) = match_alias_pattern(&rule.pattern, source_specifier) else {
                continue;
            };
            for target in &rule.targets {
                let replaced = substitute_alias_target(target, wildcard);
                candidates.push(PathBuf::from(replaced));
            }
        }
    }

    for rule in &module_resolution.tsconfig_paths {
        let Some(wildcard) = match_alias_pattern(&rule.pattern, source_specifier) else {
            continue;
        };
        for target in &rule.targets {
            let replaced = substitute_alias_target(target, wildcard);
            candidates.push(PathBuf::from(replaced));
        }
    }

    candidates
}

fn scoped_configs_for_filepath<'a>(
    filepath: &str,
    module_resolution: &'a ModuleResolutionConfig,
) -> Vec<&'a ScopedModuleResolution> {
    let normalized = normalize_surface_path(filepath);
    let mut scoped = module_resolution
        .scoped_configs
        .iter()
        .filter(|config| path_is_within_scope(&normalized, &config.scope_dir))
        .collect::<Vec<_>>();
    scoped.sort_unstable_by(|left, right| right.scope_dir.len().cmp(&left.scope_dir.len()));
    scoped
}

fn path_is_within_scope(filepath: &str, scope_dir: &str) -> bool {
    scope_dir.is_empty() || filepath == scope_dir || filepath.starts_with(&format!("{scope_dir}/"))
}

fn match_alias_pattern<'a>(pattern: &'a str, source_specifier: &'a str) -> Option<&'a str> {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => source_specifier
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix)),
        None => (pattern == source_specifier).then_some(""),
    }
}

fn substitute_alias_target(target: &str, wildcard: &str) -> String {
    target.replacen('*', wildcard, 1)
}

fn expand_module_candidate(raw_path: &str) -> Vec<String> {
    if Path::new(raw_path).extension().is_some() {
        return vec![raw_path.to_owned()];
    }

    let mut candidates = vec![];
    for ext in ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"] {
        candidates.push(format!("{raw_path}.{ext}"));
    }
    for ext in ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"] {
        candidates.push(format!("{raw_path}/index.{ext}"));
    }
    candidates
}

fn normalize_relative_path(base_dir: &str, relative: &str) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in Path::new(base_dir).join(relative).components() {
        match component {
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

fn build_reexport_signature(symbol: &crate::types::Symbol, exported_name: &str) -> String {
    match symbol.kind {
        SymbolKind::Function | SymbolKind::Method => {
            format!("{exported_name}({})", symbol.params.join(", "))
        }
        _ => exported_name.to_owned(),
    }
}

fn resolve_wildcard_reexport_targets<'a>(
    reexport: &Reexport,
    symbols: &'a [&crate::types::Symbol],
    module_resolution: &ModuleResolutionConfig,
) -> Vec<&'a crate::types::Symbol> {
    let candidate_paths = reexport_candidate_paths(reexport, module_resolution);
    let mut resolved = Vec::new();
    for candidate in candidate_paths {
        resolved.extend(symbols.iter().copied().filter(|symbol| {
            symbol.export.is_surface_public()
                && symbol.export.export_kind != ExportKind::Default
                && candidate == normalize_surface_path(&symbol.filepath)
        }));
    }
    resolved
}

fn is_wildcard_reexport(reexport: &Reexport) -> bool {
    reexport.local_name == "*" && reexport.exported_name == "*"
}

fn is_namespace_reexport(reexport: &Reexport) -> bool {
    reexport.local_name == "*" && reexport.exported_name != "*"
}

fn build_coverage_summary(
    symbols: &[&crate::types::Symbol],
    public_apis: &[PublicApiCard],
) -> CoverageSummary {
    let surface_symbols = symbols
        .iter()
        .copied()
        .filter(|symbol| is_surface_symbol_kind(symbol.kind))
        .collect::<Vec<_>>();
    let publicness = if surface_symbols.is_empty() {
        CoverageStatus::Unsupported
    } else if surface_symbols
        .iter()
        .any(|symbol| symbol.export.publicness == Publicness::Unknown)
    {
        CoverageStatus::Partial
    } else {
        CoverageStatus::Complete
    };
    let comment_normalization = if surface_symbols.is_empty() {
        CoverageStatus::Unsupported
    } else if surface_symbols
        .iter()
        .all(|symbol| symbol.comment.is_some())
    {
        CoverageStatus::Complete
    } else {
        CoverageStatus::Partial
    };
    let test_linkage = if public_apis.is_empty() {
        CoverageStatus::Unsupported
    } else if public_apis.iter().all(|api| !api.tests.is_empty()) {
        CoverageStatus::Complete
    } else if public_apis.iter().any(|api| !api.tests.is_empty()) {
        CoverageStatus::Partial
    } else {
        CoverageStatus::Unsupported
    };

    let mut notes = Vec::new();
    if publicness == CoverageStatus::Partial {
        notes.push("some symbol publicness values remain unknown".to_owned());
    }
    if comment_normalization == CoverageStatus::Partial {
        notes.push("some surfaced symbols have no normalized comment summary".to_owned());
    }
    if test_linkage == CoverageStatus::Partial {
        notes.push("linked tests were found for only part of the public API surface".to_owned());
    }

    CoverageSummary {
        publicness,
        comment_normalization,
        test_linkage,
        notes,
    }
}

fn truncated_comment_summary(symbol: &crate::types::Symbol, max_chars: usize) -> Option<String> {
    symbol.comment.as_ref().and_then(|comment| {
        comment
            .summary_line
            .as_ref()
            .map(|summary| summary.chars().take(max_chars).collect())
    })
}

fn is_public_api_candidate(kind: SymbolKind, publicness: Publicness) -> bool {
    kind == SymbolKind::Function
        && matches!(publicness, Publicness::Public | Publicness::Reexported)
}

const fn is_public_type_candidate(kind: SymbolKind, publicness: Publicness) -> bool {
    is_public_type_kind(kind) && matches!(publicness, Publicness::Public | Publicness::Reexported)
}

const fn is_public_type_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Class
            | SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Interface
            | SymbolKind::Module
            | SymbolKind::Type
            | SymbolKind::Trait
    )
}

fn is_surface_symbol_kind(kind: SymbolKind) -> bool {
    kind == SymbolKind::Function
        || matches!(
            kind,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Interface
                | SymbolKind::Module
                | SymbolKind::Type
                | SymbolKind::Trait
        )
}

fn is_unknown_api_candidate(kind: SymbolKind, publicness: Publicness) -> bool {
    kind == SymbolKind::Function && publicness == Publicness::Unknown
}

fn is_unknown_type_candidate(kind: SymbolKind, publicness: Publicness) -> bool {
    is_surface_symbol_kind(kind)
        && kind != SymbolKind::Function
        && publicness == Publicness::Unknown
}

fn is_test_candidate(filepath: &str, name: &str) -> bool {
    let normalized_path = normalize_surface_path(filepath).to_ascii_lowercase();
    normalized_path.contains("/tests/")
        || normalized_path.contains("/test/")
        || normalized_path.contains("/__tests__/")
        || normalized_path.contains("/spec/")
        || normalized_path.contains(".test.")
        || normalized_path.contains("_test.")
        || normalized_path.contains(".spec.")
        || normalized_path.contains("_spec.")
        || name.starts_with("test")
}

fn normalized_file_stem(filepath: &str) -> String {
    let normalized = normalize_surface_path(filepath);
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);
    stem.replace("_test", "")
        .replace("_spec", "")
        .replace(".test", "")
        .replace(".spec", "")
}

fn normalized_parent_dir(filepath: &str) -> String {
    let normalized = normalize_surface_path(filepath);
    normalized
        .rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_owned())
}

fn api_score(symbol: &crate::types::Symbol, linked_tests: usize) -> f32 {
    surface_score(symbol, linked_tests, 3.0, &symbol.filepath)
}

fn type_score(symbol: &crate::types::Symbol) -> f32 {
    surface_score(symbol, 0, 2.0, &symbol.filepath)
}

fn reexport_api_score(
    reexport: &Reexport,
    symbol: &crate::types::Symbol,
    test_candidates: &[&crate::types::Symbol],
) -> f32 {
    surface_score(
        symbol,
        link_tests(symbol, test_candidates).len(),
        3.0,
        &reexport.filepath,
    ) + 0.1
}

fn reexport_type_score(reexport: &Reexport, symbol: &crate::types::Symbol) -> f32 {
    surface_score(symbol, 0, 2.0, &reexport.filepath) + 0.1
}

fn surface_score(
    symbol: &crate::types::Symbol,
    linked_tests: usize,
    base: f32,
    surface_filepath: &str,
) -> f32 {
    let mut score = base;
    if symbol
        .comment
        .as_ref()
        .and_then(|comment| comment.summary_line.as_ref())
        .is_some()
    {
        score += 0.3;
    }
    if !symbol
        .comment
        .as_ref()
        .map_or_else(Vec::new, |comment| comment.flags.clone())
        .is_empty()
    {
        score += 0.2;
    }
    if linked_tests > 0 {
        score += 1.5;
    }
    if normalize_surface_path(surface_filepath).contains("/examples/") {
        score -= 0.7;
    }
    score
}

fn sort_by_score_key<T, F>(cards: &mut [T], key: F)
where
    F: Fn(&T) -> (&str, &str, f32),
{
    cards.sort_unstable_by(|left, right| {
        let (left_path, left_ref, left_score) = key(left);
        let (right_path, right_ref, right_score) = key(right);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_path.cmp(right_path))
            .then_with(|| left_ref.cmp(right_ref))
    });
}

#[allow(
    clippy::cast_precision_loss,
    reason = "test-link score stays in a tiny bounded range"
)]
const fn saturating_f32(value: usize) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CommentInfo, CommentSource, ExportInfo, ExportKind, Publicness, Reexport, Symbol,
    };

    #[test]
    fn build_surface_ref_normalizes_path_and_keeps_line_anchor() {
        assert_eq!(
            build_surface_ref(SurfaceKind::Api, "src\\auth.ts", "validateToken", 3),
            "api:src/auth.ts#validateToken@L3"
        );
    }

    #[test]
    fn overview_budget_defaults_match_step_one_spec() {
        let budget = OverviewBudget::default();
        assert_eq!(budget.max_apis, 6);
        assert_eq!(budget.max_hints, 4);
        assert_eq!(budget.max_total_chars, 6_000);
    }

    #[test]
    fn build_overview_surfaces_public_api_type_test_and_hints() {
        let corpus = InvestigationCorpus::new(vec![
            IndexDocument::Code(Symbol {
                name: "validateToken".to_owned(),
                kind: SymbolKind::Function,
                filepath: "src/auth.ts".to_owned(),
                start_line: 3,
                end_line: 8,
                params: vec!["token".to_owned()],
                signature_preview: Some("validateToken(token)".to_owned()),
                comment: CommentInfo::from_normalized_text(
                    "Validates a token.",
                    CommentSource::JsDoc,
                ),
                export: ExportInfo::public_named(),
            }),
            IndexDocument::Code(Symbol {
                name: "Session".to_owned(),
                kind: SymbolKind::Interface,
                filepath: "src/auth.ts".to_owned(),
                start_line: 10,
                end_line: 13,
                params: vec![],
                signature_preview: Some("Session".to_owned()),
                comment: None,
                export: ExportInfo::public_named(),
            }),
            IndexDocument::Code(Symbol {
                name: "validate_token_returns_session".to_owned(),
                kind: SymbolKind::Function,
                filepath: "tests/auth_test.rs".to_owned(),
                start_line: 5,
                end_line: 9,
                params: vec![],
                signature_preview: Some("validate_token_returns_session()".to_owned()),
                comment: None,
                export: ExportInfo {
                    publicness: Publicness::Private,
                    export_kind: ExportKind::None,
                    container: None,
                },
            }),
        ]);

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert_eq!(overview.public_apis.len(), 1);
        assert_eq!(overview.public_types.len(), 1);
        assert_eq!(overview.public_api_tests.len(), 1);
        assert!(
            overview
                .entry_hints
                .iter()
                .any(|hint| hint.action == HintAction::InspectSymbol)
        );
        assert!(
            overview.public_apis[0].tests[0]
                .reasons
                .contains(&TestLinkReason::FileStemMatch)
        );
        assert!(
            overview.public_apis[0].tests[0]
                .reasons
                .contains(&TestLinkReason::TestNameMatch)
        );
    }

    #[test]
    fn build_overview_falls_back_to_unknown_publicness_symbols() {
        let corpus = InvestigationCorpus::new(vec![IndexDocument::Code(Symbol {
            name: "findUser".to_owned(),
            kind: SymbolKind::Function,
            filepath: "app.py".to_owned(),
            start_line: 1,
            end_line: 3,
            params: vec!["user_id".to_owned()],
            signature_preview: Some("findUser(user_id)".to_owned()),
            comment: None,
            export: ExportInfo::unknown(),
        })]);

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert_eq!(overview.public_apis.len(), 1);
        assert_eq!(overview.public_apis[0].publicness, Publicness::Unknown);
        assert!(
            overview
                .meta
                .coverage
                .notes
                .iter()
                .any(|note| note.contains("unknown publicness"))
        );
    }

    #[test]
    fn build_overview_resolves_barrel_reexports() {
        let corpus = InvestigationCorpus::new(vec![
            IndexDocument::Code(Symbol {
                name: "validateToken".to_owned(),
                kind: SymbolKind::Function,
                filepath: "src/auth.ts".to_owned(),
                start_line: 3,
                end_line: 8,
                params: vec!["token".to_owned()],
                signature_preview: Some("validateToken(token)".to_owned()),
                comment: None,
                export: ExportInfo::public_named(),
            }),
            IndexDocument::Reexport(Reexport {
                filepath: "src/index.ts".to_owned(),
                start_line: 1,
                end_line: 1,
                local_name: "validateToken".to_owned(),
                exported_name: "createSession".to_owned(),
                source_specifier: Some("./auth".to_owned()),
                is_type_only: false,
            }),
        ]);

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert!(overview.public_apis.iter().any(|api| {
            api.signature == "createSession(token)" && api.publicness == Publicness::Reexported
        }));
    }

    #[test]
    fn build_overview_resolves_default_reexports() {
        let corpus = InvestigationCorpus::new(vec![
            IndexDocument::Code(Symbol {
                name: "createClient".to_owned(),
                kind: SymbolKind::Function,
                filepath: "src/client.ts".to_owned(),
                start_line: 1,
                end_line: 3,
                params: vec!["config".to_owned()],
                signature_preview: Some("createClient(config)".to_owned()),
                comment: None,
                export: ExportInfo::public_default(),
            }),
            IndexDocument::Reexport(Reexport {
                filepath: "src/index.ts".to_owned(),
                start_line: 1,
                end_line: 1,
                local_name: "default".to_owned(),
                exported_name: "createRepoClient".to_owned(),
                source_specifier: Some("./client".to_owned()),
                is_type_only: false,
            }),
        ]);

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert!(
            overview
                .public_apis
                .iter()
                .any(|api| api.signature == "createRepoClient(config)")
        );
    }

    #[test]
    fn build_overview_resolves_path_alias_reexports() {
        let corpus = InvestigationCorpus::with_module_resolution(
            vec![
                IndexDocument::Code(Symbol {
                    name: "validateToken".to_owned(),
                    kind: SymbolKind::Function,
                    filepath: "src/auth.ts".to_owned(),
                    start_line: 3,
                    end_line: 8,
                    params: vec!["token".to_owned()],
                    signature_preview: Some("validateToken(token)".to_owned()),
                    comment: None,
                    export: ExportInfo::public_named(),
                }),
                IndexDocument::Reexport(Reexport {
                    filepath: "src/index.ts".to_owned(),
                    start_line: 1,
                    end_line: 1,
                    local_name: "validateToken".to_owned(),
                    exported_name: "createSession".to_owned(),
                    source_specifier: Some("@/auth".to_owned()),
                    is_type_only: false,
                }),
            ],
            ModuleResolutionConfig {
                tsconfig_base_url: None,
                tsconfig_paths: vec![PathAliasRule {
                    pattern: "@/*".to_owned(),
                    targets: vec!["src/*".to_owned()],
                }],
                scoped_configs: vec![],
            },
        );

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert!(
            overview
                .public_apis
                .iter()
                .any(|api| api.signature == "createSession(token)")
        );
    }

    #[test]
    fn build_overview_expands_wildcard_reexports_and_namespace_hints() {
        let corpus = InvestigationCorpus::new(vec![
            IndexDocument::Code(Symbol {
                name: "validateToken".to_owned(),
                kind: SymbolKind::Function,
                filepath: "src/auth.ts".to_owned(),
                start_line: 3,
                end_line: 8,
                params: vec!["token".to_owned()],
                signature_preview: Some("validateToken(token)".to_owned()),
                comment: None,
                export: ExportInfo::public_named(),
            }),
            IndexDocument::Code(Symbol {
                name: "Session".to_owned(),
                kind: SymbolKind::Interface,
                filepath: "src/auth.ts".to_owned(),
                start_line: 10,
                end_line: 12,
                params: vec![],
                signature_preview: Some("Session".to_owned()),
                comment: None,
                export: ExportInfo::public_named(),
            }),
            IndexDocument::Reexport(Reexport {
                filepath: "src/index.ts".to_owned(),
                start_line: 1,
                end_line: 1,
                local_name: "*".to_owned(),
                exported_name: "*".to_owned(),
                source_specifier: Some("./auth".to_owned()),
                is_type_only: false,
            }),
            IndexDocument::Reexport(Reexport {
                filepath: "src/index.ts".to_owned(),
                start_line: 2,
                end_line: 2,
                local_name: "*".to_owned(),
                exported_name: "authApi".to_owned(),
                source_specifier: Some("./auth".to_owned()),
                is_type_only: false,
            }),
        ]);

        let overview = build_overview(&corpus, "owner/repo", OverviewBudget::default());

        assert!(
            overview
                .public_apis
                .iter()
                .any(|api| api.signature == "validateToken(token)")
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
                .any(|public_type| public_type.signature == "authApi")
        );
    }
}
