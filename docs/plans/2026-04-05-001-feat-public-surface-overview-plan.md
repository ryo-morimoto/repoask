---
title: feat: Add public-surface-first overview
type: feat
status: active
date: 2026-04-05
origin: docs/plans/repo-map.md
---

# feat: Add public-surface-first overview

## Overview

Add a concrete Step 1 `overview` surface that returns public APIs, public types, linked tests, and next-step hints without depending on README summaries or future graph work.

This spec fixes the high-leverage ambiguities that would otherwise reduce UX quality and constrain later `search` card / `inspect` work:

- stable symbol identity
- publicness modeling
- test linkage evidence
- budgeted rendering
- ownership of investigation metadata vs search index metadata

## Problem Frame

The broad repo-map plan already sets the product direction: `overview` must be the entrypoint for investigation, not a generic repo summary. What is still under-specified is the contract shape that implementation must preserve across parser, cache, repo, CLI, and later `inspect` work.

Without that contract, implementation is likely to regress in one of these ways:

- `overview` becomes README/tree-first again
- public/private/export ambiguity gets collapsed into lossy booleans
- test linkage becomes opaque and non-explainable
- search hot-path data gets bloated with investigation-only metadata
- future `inspect` work is forced to reverse-engineer unstable IDs and hints

This spec turns the current architecture intent into a concrete Step 1 implementation contract.

## Requirements Trace

- R1. `overview` must return a deterministic public-surface-first entrypoint: public APIs, public types, public API tests, and hints.
- R2. `overview` must expose uncertainty explicitly instead of silently omitting unsupported or partially supported cases.
- R3. The design must preserve search hot-path performance and cache reuse.
- R4. The contract must support future `search` card and `inspect` work without forcing incompatible rewrites.
- R5. CLI and JSON output must remain structured and agent-parseable.

## Scope Boundaries

- Not implementing `inspect` in this step.
- Not implementing graph boost, git boost, or personalized ranking.
- Not solving full re-export graph traversal for every language.
- Not requiring perfect comment normalization for every supported parser on day 1.
- Not turning `overview` into a README/tutorial summary.

## Context & Research

### Relevant Code and Patterns

- `crates/repoask-core/src/types.rs`: current shared IR stops at `IndexDocument`, `Symbol`, `DocSection`, and `SearchResult`. No structured investigation metadata exists yet.
- `crates/repoask-core/src/index.rs`: `InvertedIndex` currently persists only reduced `StoredDoc` metadata for search rendering, which is too thin for `overview`.
- `crates/repoask-repo/src/repo.rs`: clone/cache/load/build orchestration already exists and should be reused by `overview`.
- `crates/repoask-repo/src/parse.rs`: parser boundary is `Vec<IndexDocument> + ParseReport`, so investigation metadata must originate in parser output or parser-derived corpus building.
- `cli/src/main.rs`: CLI currently renders `SearchResult` only; `Explore` and `Trace` are placeholders and can still be reshaped without compatibility burden.
- `docs/plans/repo-map.md`: the intended product shape is already `overview / search / inspect`, with `overview` explicitly defined as public-surface-first.

### Institutional Learnings

- No relevant `docs/solutions/` entries exist yet.

### External References

- None. This design is grounded in current repo code and the existing repo-map plan.

## Key Technical Decisions

- **Decision: Persist investigation metadata separately from the BM25 search index.**
  Rationale: `crates/repoask-core/src/index.rs` currently stores a deliberately reduced `StoredDoc`. Expanding it to carry signatures, comment structures, export metadata, and test linkage would unnecessarily bloat the search hot path and make future investigation features harder to evolve independently.

- **Decision: `IndexDocument` becomes the rich parser IR, and `InvertedIndex` remains a derived search artifact.**
  Rationale: parser-originated metadata belongs in the shared IR. Search and `overview` should both derive from the same source, but they should not persist identical shapes for different purposes.

- **Decision: `Symbol.doc_comment: Option<String>` is replaced by structured `CommentInfo`, not supplemented by a second parallel field.**
  Rationale: dual comment fields would create long-lived model drift. Search indexing should derive text from `CommentInfo`, not from a duplicated raw string.

- **Decision: publicness is modeled as a small enum, not a boolean.**
  Rationale: Step 1 must distinguish `Public` from `Unknown`, otherwise missing parser support becomes false certainty in the UI.

- **Decision: `symbol_ref` is a stable repo snapshot identifier of the form `<surface-kind>:<filepath>#<name>@L<start_line>`.**
  Rationale: `inspect` needs a stable handoff key that is deterministic, renderer-independent, and collision-resistant within a repo snapshot.

- **Decision: Step 1 test linkage is evidence-based and heuristic, but only for explainable signals.**
  Rationale: `overview` must be deterministic and debuggable. If a test is surfaced, the response must say why.

- **Decision: `overview` gets a mode-specific budget type instead of sharing a generic section budget with future surfaces.**
  Rationale: `overview` needs independent control over APIs, types, tests, hints, and comment length. A single generic `max_items_per_section` would reduce control exactly where UX is most sensitive.

- **Decision: Step 1 CLI verb is `overview`; the current `Explore` placeholder should be removed or replaced rather than preserved as a separate long-term surface.**
  Rationale: repo docs and agent docs already converge on `overview / search / inspect`.

## Open Questions

### Resolved During Planning

- **Where should overview metadata live?** In a separate persisted investigation corpus, not inside `InvertedIndex`.
- **How should publicness be represented?** As `Publicness`, not `is_public: bool`.
- **How should hints be represented?** As structured actions, not pre-rendered command strings.
- **How should test linkage explain itself?** Through explicit `reasons`, not score-only linkage.

### Deferred to Implementation

- **How much re-export support can Step 1 ship per language?** Implementation should add deterministic support where parser coverage is straightforward and mark the rest `Unknown`.
- **Which exact parser surfaces can emit signature previews without bespoke formatting?** Implementation should prefer parser-native previews where cheap and fall back to deterministic rendering from `kind + name + params`.
- **Should Rust `pub use` and TS re-export-only files enter Step 1 immediately or be partially deferred?** This depends on implementation complexity inside parser crates, but the output model already supports `Reexported` and `Unknown`.

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

```text
parse_directory()
  -> Vec<IndexDocument>            # rich parser IR with comment/export metadata
  -> InvertedIndex::build(...)     # search artifact only
  -> InvestigationCorpus::build()  # overview/inspect artifact only

repoask-repo load_or_build_artifacts()
  -> load/save index.postcard
  -> load/save corpus.postcard
  -> share one cache validity decision via clone head + format versions

repoask_repo::overview(spec, config)
  -> load corpus
  -> aggregate public APIs/types/tests/hints
  -> apply OverviewBudget
  -> return InvestigationOverview
```

## Concrete Data Contract

### Shared IR additions in `crates/repoask-core/src/types.rs`

```rust
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub filepath: String,
    pub start_line: u32,
    pub end_line: u32,
    pub params: Vec<String>,
    pub signature_preview: Option<String>,
    pub comment: Option<CommentInfo>,
    pub export: ExportInfo,
}

pub struct CommentInfo {
    pub summary_line: Option<String>,
    pub body_preview: Option<String>,
    pub flags: Vec<CommentFlag>,
    pub source: CommentSource,
    pub normalization_status: CommentNormalizationStatus,
}

pub enum CommentFlag {
    Deprecated,
    Experimental,
    Internal,
    Unsafe,
}

pub enum CommentSource {
    JsDoc,
    RustDoc,
    PythonDocstring,
    PlainComment,
}

pub enum CommentNormalizationStatus {
    Missing,
    SummaryOnly,
    Structured,
    Failed,
}

pub struct ExportInfo {
    pub publicness: Publicness,
    pub export_kind: ExportKind,
    pub container: Option<String>,
}

pub enum Publicness {
    Public,
    Reexported,
    Package,
    Private,
    Unknown,
}

pub enum ExportKind {
    Named,
    Default,
    Reexport,
    ModuleMember,
    None,
    Unknown,
}
```

Step 1 population rules:

- `signature_preview` is bounded, human-readable, and deterministic.
- `comment.summary_line` is the only required comment-derived field for Step 1 cards.
- `flags` are populated only for easily recognized markers such as deprecation or experimental/internal markers.
- `Publicness::Unknown` is valid and must not be coerced to `Private`.

### Investigation-only types

```rust
pub struct InvestigationCorpus {
    pub documents: Vec<IndexDocument>,
}

pub struct InvestigationOverview {
    pub meta: OverviewMeta,
    pub public_apis: Vec<PublicApiCard>,
    pub public_types: Vec<PublicTypeCard>,
    pub public_api_tests: Vec<TestCard>,
    pub entry_hints: Vec<HintCard>,
}

pub struct OverviewMeta {
    pub repo: String,
    pub coverage: CoverageSummary,
    pub truncated: bool,
}

pub struct CoverageSummary {
    pub publicness: CoverageStatus,
    pub comment_normalization: CoverageStatus,
    pub test_linkage: CoverageStatus,
    pub notes: Vec<String>,
}

pub enum CoverageStatus {
    Complete,
    Partial,
    Unsupported,
}

pub struct PublicApiCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub signature: String,
    pub comment_summary: Option<String>,
    pub flags: Vec<CommentFlag>,
    pub publicness: Publicness,
    pub tests: Vec<TestLink>,
    pub score: f32,
}

pub struct PublicTypeCard {
    pub symbol_ref: String,
    pub filepath: String,
    pub kind: SymbolKind,
    pub signature: String,
    pub comment_summary: Option<String>,
    pub flags: Vec<CommentFlag>,
    pub publicness: Publicness,
    pub score: f32,
}

pub struct TestCard {
    pub test_ref: String,
    pub filepath: String,
    pub display_name: String,
    pub linked_symbols: Vec<String>,
    pub reasons: Vec<TestLinkReason>,
    pub score: f32,
}

pub struct TestLink {
    pub test_ref: String,
    pub filepath: String,
    pub display_name: String,
    pub reasons: Vec<TestLinkReason>,
    pub score: f32,
}

pub enum TestLinkReason {
    TestPathMatch,
    TestNameMatch,
    FileStemMatch,
    DirectoryProximity,
}

pub struct HintCard {
    pub action: HintAction,
    pub target: String,
    pub label: String,
    pub reason: String,
}

pub enum HintAction {
    InspectSymbol,
    SearchQuery,
    ReadPath,
}

pub struct OverviewBudget {
    pub max_apis: usize,
    pub max_types: usize,
    pub max_tests: usize,
    pub max_hints: usize,
    pub max_comment_chars: usize,
    pub max_total_chars: usize,
}
```

### Stable identity contract

`symbol_ref` and `test_ref` must follow this pattern:

```text
api:src/auth.ts#validateToken@L3
type:src/types.ts#Session@L12
test:tests/auth_test.rs#validate_token_returns_session@L8
```

Rules:

- path is repo-relative and `/` normalized
- `surface-kind` is one of `api`, `type`, `test`, or `symbol`
- `name` preserves the extracted symbol name verbatim
- `@L<start_line>` is required to avoid same-file name collisions

## Ranking and Selection Rules

### Public API candidates

Step 1 `public_apis` includes top-level symbols that satisfy both conditions:

- `kind == Function`
- `export.publicness` is `Public` or `Reexported`

Step 1 explicitly excludes these from `public_apis`:

- classes, structs, enums, interfaces, traits, and type aliases
- constants without a callable signature
- symbols with `Package`, `Private`, or `Unknown` publicness

Reason: Step 1 should prefer consumer-callable entrypoints over every exported symbol.

### Public type candidates

Step 1 `public_types` includes exported symbols where `kind` is one of:

- `Class`
- `Struct`
- `Enum`
- `Interface`
- `Type`
- `Trait`

and `export.publicness` is `Public` or `Reexported`.

### Ranking formula

Step 1 ranking remains graph-free and deterministic.

```text
overview_score = base_surface_score
               + comment_summary_bonus
               + flag_bonus
               + linked_test_bonus
               - example_path_penalty
```

Recommended initial weights:

- public API base: `+3.0`
- public type base: `+2.0`
- comment summary present: `+0.3`
- one or more linked tests: `+1.5`
- deprecated/internal flag: `+0.2` visibility bonus only, not a ranking penalty
- example/demo path: exclude from public surface unless it is the only candidate

Implementation note:

- keep exact weights in one `OverviewRankingConfig`
- test fixtures should lock ordering semantics, not just presence

## Test Linkage Contract

Step 1 test linkage is deterministic and evidence-based.

### Test candidate detection

A symbol or file is a test candidate when either condition is true:

- filepath matches known test path heuristics: `tests/`, `test/`, `__tests__/`, `spec/`, or filename stems containing `.test.` / `_test.` / `_spec.`
- symbol name looks like a Rust/Go/Python test function in a test file

### Linkage scoring

For each surfaced public API, score test candidates with these explainable signals only:

- `TestPathMatch`: target symbol name tokens appear in test file stem or test file path
- `TestNameMatch`: target symbol name tokens appear in extracted test symbol name
- `FileStemMatch`: test file stem matches the target source file stem
- `DirectoryProximity`: test file sits in the same or mirrored directory branch as the target source file

### Output behavior

- each `PublicApiCard.tests` returns top-N `TestLink`
- `public_api_tests` is the deduplicated union of tests linked to surfaced public APIs
- every surfaced test must carry at least one `reason`
- tests without evidence are not surfaced

### Non-goal for Step 1

- body-call-based linkage is explicitly deferred until symbol-reference extraction exists in parser output

## Rendering Contract

### JSON output

`overview --format json` returns one JSON object representing `InvestigationOverview`.

This differs from current search json-lines output intentionally. `overview` is a repo-level structured payload, not a list of independent results.

### Text output

Text output should render in this section order:

1. repo + coverage summary
2. public APIs
3. public types
4. public API tests
5. entry hints

Text output must never print fabricated prose. If a field is missing, omit that line rather than inventing a sentence.

### Truncation order for `OverviewBudget`

Low priority is removed first:

1. extra hints
2. extra tests
3. comment text beyond `max_comment_chars`
4. extra types
5. extra APIs

When truncation occurs:

- `meta.truncated` becomes `true`
- at least one `coverage.notes` or hint explains that additional items were omitted

### Default budget

```text
max_apis = 6
max_types = 6
max_tests = 6
max_hints = 4
max_comment_chars = 140
max_total_chars = 6000
```

## Repo and Cache Ownership

### New persistence model in `crates/repoask-repo`

Add a new investigation artifact alongside the existing index artifact.

- `index.postcard`: search artifact only
- `corpus.postcard`: investigation artifact only
- shared cache meta still keys off repo clone head + format compatibility

Implementation requirement:

- `index` and `corpus` format versions must be explicit and independent
- a cache hit is valid only when all artifacts required for the requested surface are present and compatible

### New repo APIs

Add:

```text
overview(spec, config) -> InvestigationOverview
overview_with_report(spec, config) -> { overview, parse_diagnostics }
```

Behavior requirements:

- reuse the same clone + parse + cache orchestration as search
- do not rebuild artifacts when compatible cached artifacts already exist
- preserve stderr-only parse diagnostics for verbose CLI behavior

## CLI Contract

### Command surface

- replace the current placeholder `Explore` command with `Overview`
- do not introduce a second synonym as part of Step 1

### CLI options

Step 1 CLI options:

- `repoask overview <repo-spec>`
- `--format json|text`
- `--verbose`

No query string is accepted for `overview` in Step 1.

### Hint rendering

Hints are rendered from structured fields, not stored as literal shell strings in core types.

Example text rendering:

```text
entry_hints:
  inspect validateToken (public API with linked tests)
  read tests/auth_test.rs (top linked test)
```

## Implementation Units

- [ ] **Unit 1: Add shared investigation and metadata contracts**

**Goal:** Introduce the shared types required for `overview` without yet wiring parser or CLI behavior.

**Requirements:** R1, R2, R4

**Dependencies:** None

**Files:**
- Modify: `crates/repoask-core/src/types.rs`
- Create: `crates/repoask-core/src/investigation.rs`
- Modify: `crates/repoask-core/src/lib.rs`
- Test: `crates/repoask-core/src/investigation.rs` or adjacent unit tests

**Approach:**
- Add `CommentInfo`, `ExportInfo`, `Publicness`, `InvestigationOverview`, `HintCard`, `OverviewBudget`, and related enums.
- Move future-facing `overview` aggregation behavior into a dedicated `investigation` module rather than overloading `index.rs`.
- Replace `Symbol.doc_comment` with `Symbol.comment` in the shared IR and provide a helper for deriving indexable comment text.

**Patterns to follow:**
- `crates/repoask-core/src/types.rs`
- `crates/repoask-core/src/index.rs`

**Test scenarios:**
- Happy path: serializing and deserializing `CommentInfo`, `ExportInfo`, and overview cards round-trips without loss.
- Edge case: `Publicness::Unknown` survives round-trip and is not collapsed to another variant.
- Edge case: `symbol_ref` generation for two same-named symbols in one file produces distinct refs because line anchors differ.
- Error path: none -- pure type/module introduction.

**Verification:**
- Shared types compile cleanly and can be consumed from both `repoask-core` and downstream crates.

- [ ] **Unit 2: Extend parser output with Step 1 overview metadata**

**Goal:** Populate `signature_preview`, structured comment summaries, and initial export/publicness metadata in parser output.

**Requirements:** R1, R2, R4

**Dependencies:** Unit 1

**Files:**
- Modify: `crates/repoask-parser/src/oxc.rs`
- Modify: `crates/repoask-parser/src/markdown.rs`
- Modify: `crates/repoask-parser/src/lib.rs`
- Modify: `crates/repoask-treesitter/src/parser.rs`
- Modify: `crates/repoask-treesitter/src/lib.rs`
- Test: `crates/repoask-parser/src/snapshots/*`
- Test: `crates/repoask-treesitter/src/lib.rs`

**Approach:**
- Normalize comment text into `CommentInfo.summary_line` first; treat richer tags as best-effort.
- Add cheap publicness detection where parser syntax already exposes it.
- Leave unsupported cases explicit via `Publicness::Unknown`.
- Do not attempt body reference extraction in this unit.

**Execution note:** Start with parser snapshot/fixture expectations before broad refactors to avoid silent metadata drift.

**Patterns to follow:**
- `crates/repoask-parser/src/oxc.rs`
- `crates/repoask-treesitter/src/parser.rs`

**Test scenarios:**
- Happy path: exported TS function yields `Publicness::Public`, a signature preview, and comment summary.
- Happy path: Rust `pub fn` with rustdoc yields publicness and comment summary.
- Edge case: unsupported export analysis yields `Publicness::Unknown`, not `Private`.
- Edge case: symbol with no comment yields `comment = None` and remains indexable.
- Error path: malformed or unstructured comments fall back to `CommentNormalizationStatus::Failed` or `Missing` without dropping the symbol.

**Verification:**
- Parser snapshots show stable metadata for representative TS and Rust examples.

- [ ] **Unit 3: Add an investigation corpus cache and repo overview API**

**Goal:** Persist rich investigation data separately from the search index and expose repo-level `overview(...)` APIs.

**Requirements:** R2, R3, R4

**Dependencies:** Units 1-2

**Files:**
- Create: `crates/repoask-repo/src/investigation_store.rs`
- Modify: `crates/repoask-repo/src/repo.rs`
- Modify: `crates/repoask-repo/src/lib.rs`
- Modify: `crates/repoask-repo/src/index_store.rs`
- Test: `crates/repoask-repo/src/index_store.rs`
- Test: `crates/repoask-repo/tests/*`

**Approach:**
- Introduce load/save support for the investigation corpus.
- Reuse current cache-validation flow from `repo.rs`.
- Ensure artifact compatibility is checked independently for search and overview needs.
- Keep `InvertedIndex` lean; do not backfill overview metadata into `StoredDoc` except for fields already required by search.

**Patterns to follow:**
- `crates/repoask-repo/src/repo.rs`
- `crates/repoask-repo/src/index_store.rs`

**Test scenarios:**
- Happy path: cache miss builds both index and corpus, then `overview()` returns structured data.
- Happy path: cache hit reuses compatible corpus without reparsing.
- Edge case: compatible search index but missing or incompatible corpus triggers only the corpus rebuild path.
- Error path: oversized or corrupt corpus artifact is rejected safely and rebuilt.
- Integration: search output remains unchanged after corpus support is added.

**Verification:**
- `overview()` and `search()` share clone/cache validity logic but load only the artifacts they need.

- [ ] **Unit 4: Implement deterministic overview aggregation and ranking**

**Goal:** Turn the cached corpus into `InvestigationOverview` using deterministic surface selection, test linkage, and budgets.

**Requirements:** R1, R2, R4, R5

**Dependencies:** Units 1-3

**Files:**
- Modify: `crates/repoask-core/src/investigation.rs`
- Modify: `crates/repoask-core/src/lib.rs`
- Test: `crates/repoask-core/src/investigation.rs`
- Test: `crates/repoask-repo/tests/e2e_fixture_repo.rs`
- Create or extend: `crates/repoask-repo/tests/fixtures/*`

**Approach:**
- Build public API and public type candidate sets from the corpus.
- Link tests with evidence-bearing heuristics only.
- Generate structured `HintCard` values from surfaced APIs/types/tests.
- Apply `OverviewBudget` after ranking, with explicit truncation metadata.

**Technical design:** *(directional guidance, not implementation specification)*

```text
collect exported symbols
  -> partition into api candidates and type candidates
  -> score linked tests for api candidates
  -> rank and truncate sections independently
  -> synthesize deduplicated public_api_tests + entry_hints
```

**Patterns to follow:**
- `crates/repoask-core/src/index.rs`
- `crates/repoask-repo/tests/e2e_fixture_repo.rs`

**Test scenarios:**
- Happy path: exported function ranks above exported type in `public_apis` and links top relevant tests.
- Happy path: exported class or interface appears only under `public_types`.
- Edge case: when all publicness values are `Unknown`, overview returns empty public sections plus coverage notes explaining why.
- Edge case: duplicate linked tests across two APIs are deduplicated in `public_api_tests` while preserving highest score and merged linked symbols.
- Error path: none -- pure deterministic aggregation over parsed data.
- Integration: budget truncation sets `meta.truncated = true` and preserves highest-priority cards.

**Verification:**
- Fixture-based overview snapshots are stable and explainable.

- [ ] **Unit 5: Implement CLI `overview` and renderers**

**Goal:** Expose `overview` as a user-facing CLI command with structured JSON and deterministic text rendering.

**Requirements:** R1, R5

**Dependencies:** Units 1-4

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `README.md`
- Test: `cli/src/main.rs`

**Approach:**
- Replace the `Explore` placeholder with a real `Overview` command.
- Keep stdout structured: one JSON object for json output, plain deterministic sections for text output.
- Preserve stderr-only parse diagnostics for verbose mode.

**Patterns to follow:**
- `cli/src/main.rs`

**Test scenarios:**
- Happy path: `overview --format json` emits a single valid `InvestigationOverview` object.
- Happy path: `overview --format text` prints sections in the specified order and omits absent optional fields cleanly.
- Edge case: truncated overview text includes a visible indication that more items were omitted.
- Error path: invalid repo spec returns a non-zero exit with stderr error text, matching current CLI behavior.
- Integration: existing `search` JSON and text output remain unchanged.

**Verification:**
- CLI behavior matches the new contract without regressing existing `search` and `cleanup` commands.

## System-Wide Impact

- **Interaction graph:** parser crates now feed two downstream artifacts: `InvertedIndex` and `InvestigationCorpus`.
- **Error propagation:** corrupt or incompatible artifacts should surface as safe rebuilds, not opaque overview failures.
- **State lifecycle risks:** index/corpus artifact drift is the main risk; cache validation must treat them as separate but coordinated artifacts.
- **API surface parity:** `overview` becomes the first investigation surface and must use the same metadata model that later `search` cards and `inspect` will consume.
- **Integration coverage:** fixture repos must validate parser metadata, corpus persistence, overview ranking, and CLI rendering together.
- **Unchanged invariants:** `search` remains BM25-based, graph-free, and agent-parseable; this step does not change its output contract.

## Risks & Dependencies

| Risk | Mitigation |
|------|------------|
| Investigation metadata bloats the search artifact | Persist corpus separately from `InvertedIndex` |
| Partial parser support makes overview misleading | Use `Publicness::Unknown` and `CoverageSummary` instead of silent omission |
| Test linkage produces false positives | Surface only evidence-bearing matches and expose `reasons` |
| CLI contract drifts from future `inspect` | Use structured hints and stable refs now, not literal command strings |
| Cache compatibility bugs create stale outputs | Version `index` and `corpus` artifacts independently and test mixed-validity cases |

## Documentation / Operational Notes

- Update `README.md` to document `overview` as the public-surface-first command.
- Keep `docs/plans/repo-map.md` as the broad architecture document; this plan is the concrete Step 1 implementation spec.
- Add fixture-backed overview snapshots before expanding `search` or `inspect` behavior.

## Validation Criteria

- WHEN a repo exposes deterministic top-level public functions, THEN `overview` returns them under `public_apis` with stable `symbol_ref` values.
- WHEN a repo exposes exported classes/interfaces/types, THEN `overview` returns them under `public_types` and does not duplicate them as APIs.
- WHEN linked tests are surfaced, THEN each surfaced test includes at least one explicit linkage reason.
- WHEN parser support is partial, THEN `overview.meta.coverage` explains the limitation instead of silently reporting an empty surface as authoritative.
- WHEN cached artifacts are compatible, THEN `overview` reuses the cache without reparsing.
- WHEN `overview --format json` is used, THEN stdout is a single structured JSON object.
- WHEN `overview --format text` is used, THEN the output is deterministic, sectioned, and budgeted.

## Sources & References

- Origin document: `docs/plans/repo-map.md`
- Related code: `crates/repoask-core/src/types.rs`
- Related code: `crates/repoask-core/src/index.rs`
- Related code: `crates/repoask-repo/src/repo.rs`
- Related code: `crates/repoask-repo/src/parse.rs`
- Related code: `cli/src/main.rs`
