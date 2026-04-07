# TODO

実装を前に進めるために置き去りにした項目。

2026-04-05 時点でコード・ドキュメント・GitHub/crates.io 状態を再確認し、未完了項目は「今どこが正しくて、何を直すべきか」を表す文に更新する。

未完了項目には `[confidence: high|medium|low]` を付ける。`low` は人間が triage してから進める候補。

## セキュリティ

### コマンドインジェクション / 引数インジェクション

- [x] ~~**`clone.rs:68` — owner/repo が git URL に直接埋め込まれる。**~~ → `ensure_clone()` 冒頭で `is_valid_name()` (`^[a-zA-Z0-9._-]+$` 相当) バリデーション追加済み
- [x] ~~**`clone.rs:73-75` — ref_spec が `--branch` に未検証で渡される。**~~ → `ensure_clone()` で先頭 `-` チェック追加済み
- [x] ~~**ref_spec バリデーションがキャッシュヒット時にスキップされる。**~~ → `parse_repo_spec()` で ref_spec の先頭 `-` と空文字列を拒否するようにした。`ensure_clone()` に到達する前にエントリポイントで弾かれる

### パストラバーサル

- [x] ~~**`cache.rs:27-28` — owner/repo がファイルパスに直接結合される。**~~ → `repo_cache_dir()` に `is_safe_path_component()` assert 追加済み
- [x] ~~**`is_valid_name()` が `..` を通すためパストラバーサルが `cache.rs` の `assert!` で panic する。**~~ → `parse_repo_spec()` に `is_safe_repo_component()` を追加し、`..` や不正文字を含む owner/repo をエントリポイントで拒否するようにした。`clone.rs` の `is_valid_name()` と `cache.rs` の `assert!` は defense-in-depth として維持
- [x] ~~**`cache.rs:52-55` — `cleanup_all()` が `REPOASK_CACHE_DIR` 環境変数のパスを `remove_dir_all` する。**~~ → パスに "repoask" を含むかチェック追加済み

### 信頼境界

- [x] ~~**`index_store.rs:62-67` — キャッシュから bincode デシリアライズする際にサイズ制限がない。**~~ → `load_index()` に 500MB 上限チェック + `TooLarge` エラー追加済み。現在は `postcard` に移行済み
- [x] ~~**`index_store.rs:78-81` — `index.meta.json` の JSON デシリアライズに入力長制限がない。**~~ → `load_meta()` に 1MB 上限チェック追加済み
- [x] ~~**`parse_directory` — ファイルサイズ制限なし。**~~ → `parse_directory()` に 10MB 上限チェック + `oversized` レポート追加済み

### ディスク消費

- [x] ~~**キャッシュサイズ制限なし。**~~ → `evict_if_needed()` で 2GB 上限 + mtime LRU eviction 実装済み。search 後に best-effort で実行
- [x] ~~**`clone_fresh` の tmp ディレクトリが PID ベースの命名。**~~ → ナノ秒タイムスタンプに変更済み。rename 失敗時の tmp cleanup も追加済み

## Repo Doctor フォローアップ (2026-04)

Repo Doctor で出た指摘のうち、既存 TODO に未記載だったものだけを追記する。2026-04-05 に repo / docs / CI / GitHub 設定を再確認し、文言を現状ベースに更新した。

- [x] ~~**SearchResult / example semantics の docs drift が残っている。**~~ → `DESIGN.md` と `wasm/src/lib.rs` を `CodeResult.is_example` ベースに更新し、`trace` / `related` / `preview` は planned と明記済み
- [ ] [confidence: medium] **docs freshness check がない。** `DESIGN.md`, `SETUP.md`, `docs/**/*.md` を対象に oldest-5 / path existence を確認する軽量スクリプトを local + CI に追加し、documentation の drift を定期検知する
- [ ] [confidence: high] **CI が flake を source-of-truth に使っていない。** まず `nix develop --command just ci` を CI に導入し、必要なら flake `checks` を定義して `nix flake check` に寄せる
- [ ] [confidence: high] **hook / local / CI の実行コマンドが分散している。** `fmt` / `clippy` / `test` / `doc` / `coverage` / `benchmark` を `just` recipe に集約し、`prek.toml` と workflow は `just` 経由で呼ぶ
- [ ] [confidence: high] **Nix / shell の静的解析が弱い。** `statix` / `deadnix` / `shellcheck` などを導入し、`flake.nix` と `scripts/*.sh` も local + CI で検査する
- [x] ~~**Dependabot security updates が無効。**~~ → `.github/dependabot.yml` で cargo + github-actions の両 ecosystem を weekly 更新する設定を追加済み
- [ ] [confidence: high] **release workflow に SBOM がない。** `Syft` などで SPDX/CycloneDX SBOM を生成し、release asset として公開する
- [ ] [confidence: medium] **`cargo deny` warning ノイズが残っている。** duplicate `hashbrown` と unmatched license allowlist を整理し、dependency audit 出力を warning-free に近づける
- [ ] [confidence: high] **コラボレーション防御が未整備。** `.github/CODEOWNERS` と PR template を追加し、main の branch protection で required checks / review 必須を有効化する
- [ ] [confidence: medium] **PR 上の品質差分が見えにくい。** coverage は Codecov の diff/comment まで有効化し、benchmark は下記の CodSpeed 連携で reviewer に見える形にする
- [ ] [confidence: medium] **汎用 SAST とは別に、アーキテクチャ適合テストがない。** Semgrep custom rule などで crate boundary / banned API / layering rule を機械検証する

## パフォーマンス

### 検索 (hot path)

- [x] ~~**`index.rs:194-198` — `doc_freq` が検索時に毎回 `HashSet` 生成で計算される。**~~ → `doc_freqs: HashMap<String, u32>` をインデックス構築時に事前計算して O(1) lookup に変更済み
- [x] ~~**`index.rs:215-216` — 全スコア付きドキュメントをフルソートしてから truncate。**~~ → `BinaryHeap` + `ScoredDoc` による O(n log k) top-k 選択に置換済み
- [ ] [confidence: low] **`search()` が毎回 `HashMap<DocId, f32>` を確保する。** まずプロファイルで寄与を確認し、必要なら reusable searcher / scratch buffer API を追加して再利用する
- [x] ~~**`bm25.rs:25-27` — `weights[field_id as usize]` に bounds check が入る。**~~ → `get().copied().unwrap_or(0.0)` に変更済み

### インデックス構築

- [x] ~~**`tokenizer.rs:49,62` — `Stemmer::create()` が `tokenize_text` / `tokenize_identifier` 呼び出しごとに生成される。**~~ → `thread_local!` で `Stemmer` をキャッシュ済み
- [x] ~~**`parse_directory` — シングルスレッドでファイルを逐次処理。**~~ → `WalkBuilder::build_parallel()` + `crossbeam-channel` で並列化済み
- [x] ~~**`tree_sitter_parser.rs:11-14` — `Parser::new()` + `set_language()` がファイルごとに呼ばれる。**~~ → `thread_local!` で `Parser` インスタンスを再利用済み
- [x] ~~**`oxc.rs:309-354` — `extract_leading_comment` がシンボルごとにソース先頭から `offset` まで逆方向スキャンする。**~~ → oxc `Program.comments` から `HashMap<u32, String>` を事前構築し O(1) lookup に変更済み
- [x] ~~**`index.rs:161-176` — `add_field_tokens` 内で `HashMap<&str, u16>` を毎フィールドで新規作成。**~~ → `build()` で1つの `HashMap` を作り `add_field_tokens` に渡して `clear()` + `drain()` で再利用済み

### メモリ

- [ ] [confidence: low] **`InvertedIndex` は term 文字列の重複保持が多い。** `postings` と `doc_freqs` が同じ term を別々に持つので、大規模コーパスが問題になったら `TermId` への intern を検討する
- [x] ~~**`field_lengths` — `u16` で表現。**~~ → `.len().min(u16::MAX as usize) as u16` で飽和キャストに変更済み
- [ ] [confidence: low] **`StoredDoc::Doc.content_preview` を全 doc section に保存している。** 現状の `DocResult.snippet` はここから生成されるため必要だが、インデックスサイズが重ければ snippet 生成責務を見直す
- [x] ~~**`index.rs:47` — `InvertedIndex::build()` が `docs: Vec<IndexDocument>` を値渡しで受け取る。**~~ → 現在は `build(docs: &[IndexDocument])` に変更済み。呼び出し側もスライス参照を渡している

### I/O

- [x] ~~**`parse_directory` — `read_to_string` はファイル全体をメモリに載せる。**~~ → 10MB 上限チェック追加済み（セキュリティ > 信頼境界と同一修正）
- [ ] [confidence: low] **`index_store::save_index` は index 全体を `Vec<u8>` にシリアライズしてから書く。** まずピークメモリを計測し、必要なら `BufWriter` / streaming encode へ切り替える
- [x] ~~**`clone.rs:79` — `cmd.output()` は git の stdout/stderr を全てメモリにバッファする。**~~ → `stdout(Stdio::null())` + `stderr(Stdio::piped())` 追加済み

## Lint / コード品質

- [x] ~~`missing_docs` warnings を解消する~~ → 全 77 pub items にドキュメント追加済み。警告ゼロ
- [x] ~~`unnecessary qualification` warnings を解消する (3件, `cargo fix` で自動修正可能)~~ → 警告ゼロ確認済み
- [x] ~~clippy をインストールして pedantic/nursery/restriction lint を通す~~ → `[workspace.lints]` + `clippy.toml` で設定、flake.nix に clippy 追加済み
- [x] ~~`expect_used` / `unwrap_used` が `deny` に設定された — `tree_sitter_parser.rs:13` の `expect()` を `ok_or()` + `?` に置き換える~~ → repoask-treesitter 分離時に修正済み
- [x] ~~テストコード内の `unwrap()` / `panic!()` に `#[cfg(test)]` 用の lint 除外を追加する~~ → 全テストモジュールに `#[allow(..., reason = "...")]` 追加済み

## ParseProvider アーキテクチャ（優先度: 高）

設計: `docs/plans/parser-provider.md`

パーサー選択を trait で抽象化し、platform ごとに最適な実装を差し替え可能にする。
WASM サイズ肥大化を防ぎつつ、対応言語を段階的に拡張できる構造にする。

### Step 1: ParseProvider trait 導入

以下の既存課題をまとめて解決する:

- [x] ~~**`ParseOutcome` / `ParseError` の重複を解消。**~~ → `ParseOutcome` を `repoask-core::types` に定義、両パーサーが `pub use` で re-export する形に変更済み
- [x] ~~**lenient parse API の戻り値を統一する。**~~ → `repoask-core::types::ParseOutcome::into_lenient()` を追加し、`repoask-parser` / `repoask-treesitter` の `parse_file_lenient()` をともに `Option<Vec<IndexDocument>>` に統一済み
- [ ] [confidence: medium] **共有 parse abstraction を `repoask-core` に定義する。** `ParseProvider::parse_file(filepath, source)` の戻り値とエラー表現を platform 非依存に揃える
- [ ] [confidence: medium] **`NativeParseProvider` を `repoask-repo` に実装。** 現在の `parse.rs` のディスパッチロジック (repoask-parser → repoask-treesitter fallback) を移植
- [ ] [confidence: medium] **`WasmParseProvider` を `repoask-wasm` に実装。** 現在は `repoask_parser::parse_file_lenient()` 直接呼び出し → trait 経由に変更。Step 2 で動的ロード機構の拡張点になる

### Step 2 以降（今は実装しない）

- [ ] [confidence: medium] Web 側に動的ロード機構を追加。tree-sitter grammar を個別 `.wasm` として配布、`registerGrammar(ext, wasmBytes)` で登録
- [ ] [confidence: medium] CLI 側の言語パーサーを feature flag で選択可能にする

## Boost Signals アーキテクチャ（優先度: 高）

設計: `docs/plans/boost-signals.md`

4階層 (module/directory/file/symbol) の外部ランキング信号で BM25 スコアを補強する。
git history, call graph 等のデータから信号を計算し、プラットフォーム間でランキングを一致させる。

### 4層アーキテクチャ

```
L1+2: 階層的重み付け + 4層具体     (repoask-core)
L3:   git 抽象 → 重み計算ロジック  (repoask-core)
L4:   git 具体 → git 抽象          (platform-specific)
```

### Step 1: BoostSignals + search_with_boost

- [ ] [confidence: medium] `BoostSignals` 型を `types.rs` に追加 (`module_boosts`, `directory_boosts`, `file_boosts`, `symbol_boosts` の4つの `HashMap`)
- [ ] [confidence: medium] `Normalization` enum (`Log1p` / `Saturate` / `None`) を追加
- [ ] [confidence: medium] `RankingConfig` struct を追加
- [ ] [confidence: medium] `InvertedIndex::search_with_boost()` を `index.rs` に実装。スコア計算: `bm25 * (1.0 + normalized(sum_of_boosts))`
- [ ] [confidence: medium] 既存の `search()` は変更なし (後方互換)
- [ ] [confidence: medium] WASM API に `searchWithBoost(query, limit, signals: JsValue)` を追加 (`serde-wasm-bindgen` で直接デシリアライズ)
- [ ] [confidence: medium] テスト: ブースト有無で順位が変わることを検証

### Step 2: GitData 型 + compute_boost_signals

- [ ] [confidence: medium] `GitData` 型を定義: `Commit` (id, parents, author, author_timestamp, message), `DiffEntry` (status, path, old_path, additions, deletions), `CommitDiff`, `BlameBlock`, `FileBlame`, `Tag`
- [ ] [confidence: medium] `DiffStatus` enum (`Added`, `Modified`, `Deleted`, `Renamed`, `Copied`)
- [ ] [confidence: medium] `compute_boost_signals(git_data, now_timestamp) -> BoostSignals` を repoask-core に実装
- [ ] [confidence: medium] 信号計算ロジック: 変更頻度 (log), 鮮度 (recency decay), 著者多様性, churn
- [ ] [confidence: medium] **一貫性保証**: この関数が全プラットフォームで共通であることが必須。CLI/Web/Server で同一の Rust コード

### Step 3 以降

- [ ] [confidence: medium] repoask-repo に git subprocess → `GitData` アダプタ (Layer 4 CLI)
- [ ] [confidence: medium] Web 実装 (`wasm` または将来の `repoask-web`) に bit/API → `GitData` アダプタ (Layer 4 Web)
- [ ] [confidence: medium] グラフノード設計: call graph / import graph / co-change graph のエッジ定義
- [ ] [confidence: medium] centrality (PageRank 等) → `symbol_boosts` 変換

## Repo Map / Investigation Surface アーキテクチャ（優先度: 高）

設計: `docs/plans/repo-map.md`

coding agent が bug / 想定外挙動を調べる入口として、`overview` は public API / type / public API tests を、`search` は次の一手が分かる result card を、symbol inspect は implementation tree / raw code / tests / hints を返す。すべて non-LLM, deterministic, token-budget aware にする。

### Step 1: public-surface-first `overview` 導入

- [x] ~~`repoask-core` に `InvestigationOverview`, `PublicApiCard`, `PublicTypeCard`, `TestCard`, `HintCard`, `RenderBudget` を追加する~~ → `crates/repoask-core/src/investigation.rs` に `InvestigationOverview` / `PublicApiCard` / `PublicTypeCard` / `TestCard` / `HintCard` / `OverviewBudget` / stable `SurfaceKind` ref を追加済み。generic `RenderBudget` は Step 3 の truncation 項目で継続管理
- [ ] [confidence: medium] parser 出力に `CommentInfo` / `ExportInfo` 相当の metadata は入ったが、現状は `summary_line` / `flags` / `signature_preview` / cheap publicness 判定まで。`examples`, `throws/errors/panics`, `returns`, richer tag normalization は未実装
- [ ] [confidence: medium] `CommentNormalizationStatus` 型は入ったが、coverage/report 集計はまだない。現状の parser は `summary_line` を返すだけで、language/source/style ごとの比較や ratchet には未接続
- [x] ~~`repoask-core` に overview 集約ロジックを追加し、public API / type / linked tests を deterministic に返せるようにする~~ → `build_overview()` を追加し、stable `symbol_ref` / linked test reasons / `CoverageSummary` / `OverviewBudget` で deterministic overview を返すようにした
- [x] ~~`repoask-repo` に `overview(spec, config)` / `overview_with_report(...)` は追加済みだが、`load_or_build_index` と `load_or_build_corpus` がまだ別実装で `parse_directory()` を重複実行する。search と overview の artifact build path は今後統合する~~ → `load_or_build_index()` は canonical artifact として `corpus` を再利用して index を再構築するように変更済み。index miss + corpus hit では parse を再実行しない
- [x] ~~CLI に `overview` を追加し、README 要約ではなく public API / type / public API tests を優先表示する~~ → `repoask overview <repo-spec>` を追加し、JSON 1 object / text renderer と `--verbose` corpus parse report を実装済み
- [ ] [confidence: medium] WASM API に overview JSON を返す入口を追加する
- [x] ~~`index_store` の format version を bump し、overview 系 metadata の roundtrip test を追加する~~ → search index とは別に `investigation_store` を追加し、`CORPUS_FORMAT_VERSION` + `corpus_save_load_roundtrip` で overview/corpus 側の roundtrip を持たせた
- [ ] [confidence: medium] fixture repo で `overview` の E2E selection test は追加済みだが、snapshot と explicit ranking fixture はまだない。public API priority / linked test priority / comment fallback を snapshot で固定化する

### Step 2: search card と graph-ready ranking

- [ ] [confidence: medium] `search` の出力を `SearchResult` 配列から investigation card に拡張し、`why surfaced` / `next` / `linked tests` を返す
- [ ] [confidence: medium] search ranking に public API / public type / linked test / comment summary bonus を追加する
- [ ] [confidence: medium] `BoostSignals` / graph boost を search ranking に差し込める API を追加する

### Step 3: symbol inspect

- [ ] [confidence: high] 独立 `extract` コマンドは増やさず、symbol inspect mode で raw code / implementation tree / callee cards / relevant tests / next hints を返す設計に置き換える
- [ ] [confidence: medium] direct callee / imported symbol / referenced type を抽出し、depth 1-2 の implementation tree を構築する
- [ ] [confidence: medium] callee summary は LLM 生成ではなく `CommentInfo.summary_line` / `signature_preview` / linked tests の structured card から deterministic に render する
- [ ] [confidence: medium] generic `RenderBudget` は未実装。現状は `OverviewBudget` で section 件数と comment 長だけ打ち切っており、`max_total_chars` と search/inspect 共通の budget/truncation ルールはまだない

### Step 3.5: comment coverage instrumentation

- [ ] [confidence: medium] `CommentCoverageReport` を追加し、language/source/status ごとの comment normalization coverage を reference parser の正規化出力との差分として計測できるようにする
- [ ] [confidence: medium] `just comment-coverage` のような開発用レポート生成フローを追加し、JSON/Markdown artifact を出せるようにする
- [ ] [confidence: medium] CI に comment coverage artifact と ratchet を追加し、summary/structured coverage の回帰を検出する
- [ ] [confidence: medium] 各言語で採用する reference parser と、その出力を `ReferenceCommentInfo` に正規化する adapter を決める

### Step 4 以降

- [ ] [confidence: medium] graph centrality / reverse dependency count を取り込み、search / inspect に graph boost と blast radius 表示を追加する
- [ ] [confidence: medium] optional query/context personalization を追加し、agent の会話文脈に応じて search / inspect を再ランクできるようにする
- [ ] [confidence: medium] public-surface-first `overview` は主要言語で investigation entrypoint として使える基礎ができたが、まだ「主要言語で同じ baseline facts を揃える」段階にいる。今後は TS 個別最適化ではなく、`symbol identity` / `kind` / `container` / `signature_preview` / `comment summary` / `publicness` / `re-export or alias surface` / `linked test candidates` を主要言語 (TS/JS, Rust, Python, Go, Java, Ruby, C/C++) でどこまで揃えられるかを軸に quality を上げる

### Step 4.5: Cross-Language Baseline Quality

- [ ] [confidence: high] overview 用の共通 facts contract に対する language coverage matrix がない。主要言語ごとに `symbol identity` / `kind` / `container` / `signature_preview` / `comment summary` / `publicness` / `re-export or alias surface` / `linked test candidates` の取得可否と degrade 方針を明文化する
- [ ] [confidence: high] 主要言語の representative fixture corpus が不足していて、overview の baseline quality を横断比較できない。TS/JS, Rust, Python, Go, Java, Ruby, C/C++ で representative fixture を揃え、surface card の snapshot と coverage expectation を固定化する
- [ ] [confidence: medium] `overview` の degrade contract は `CoverageSummary` に寄り始めたが、card-level / language-level で何が `Complete | Partial | Unsupported` なのかをまだ十分に表現できていない。language coverage を UI/JSON contract に露出する

### Step 4.6: Language-Specific Enhancement

- [ ] [confidence: medium] TypeScript/JavaScript module resolution は root/nested `tsconfig.json` / `jsconfig.json` の `baseUrl` / `paths`、JSONC comment/trailing comma、相対 `extends` と ancestor `node_modules` package-style `extends` までは見られる。残りは `references`、より完全な npm/package `extends` 解決、複数 config の TS precedence を詰める
- [ ] [confidence: medium] language-specific publicness / access semantics は cheap heuristic が中心。主要言語で `overview` の entrypoint quality を上げるため、言語ごとの厳密な access semantics と exported surface 判定を段階的に詰める

## モジュール / インターフェース

### Field 定数 (BM25 実装詳細) が types.rs に混在している

- [x] ~~**Field 定数を types.rs から index.rs に移動。**~~ → `DocId`, `FieldId`, `FIELD_*`, `NUM_FIELDS`, `Posting`, `FieldStats` を `index.rs` に移動済み。`bm25.rs` は `crate::index::*` から import

### SearchResult::Example が IndexDocument に対応するバリアントを持たない

- [x] ~~**`ExampleResult` を除去して `CodeResult` に `is_example: bool` 追加。**~~ → `SearchResult` を `Code | Doc` の 2 variant に削減済み。CLI は `is_example` フラグでラベルを切り替え。similarity 型 threshold を 0.95 に引き下げ済み

## 型設計

- [x] ~~`SearchResult` の `score()` / `filepath()` をトレイト化するか検討 — 各バリアントで同じフィールドを持つが、共通アクセスが match 必須~~ → `SearchResult` に inherent メソッド `score()` / `filepath()` を実装済み。現時点で追加 trait は不要

## パーサー / Language Coverage

- [ ] [confidence: medium] oxc: `export default function() {}` (無名デフォルトエクスポート) が `"default"` という名前になる — より良いフォールバック名を検討
- [ ] [confidence: high] oxc: class property declaration を抽出していない。static/getter/setter は抽出されるが、種別情報を結果に保持していない
- [ ] [confidence: high] oxc: `declare module` / `declare namespace` (ambient declarations) 未対応
- [ ] [confidence: low] tree-sitter: Rust の `impl` ブロックのメソッド抽出クエリがネストした `impl` (trait impl) で正しく動くか未検証
- [ ] [confidence: high] tree-sitter: Python の decorator 情報、Go の receiver type 情報を `Symbol` に含めていない
- [ ] [confidence: low] tree-sitter: C/C++ の function declarator がネストしたポインタ宣言で名前抽出に失敗する可能性
- [x] ~~tree-sitter: Ruby の `module` が `SymbolKind::Class` として返される — 専用の `Module` kind を追加するか検討~~ → `SymbolKind::Module` を追加し、tree-sitter Ruby query / parser / overview namespace card を `Module` kind に更新済み
- [ ] [confidence: medium] markdown: MDX (JSX in markdown) のコンポーネント記法を認識していない
- [x] ~~oxc: wildcard re-export (`export * from`, `export * as ns from ...`) を `IndexDocument::Reexport` に落としていない。現状の overview は named re-export のみ解決する~~ → `ExportAllDeclaration` を `IndexDocument::Reexport` に落とし、overview 側も wildcard / namespace re-export を展開または namespace card として扱うようにした
- [x] ~~markdown: 空セクション(見出し直後に次の見出し) がゼロ内容の `DocSection` として生成される~~ → `push_section()` で body/code-symbol が空の heading-only section を skip するように修正し、snapshot と unit test を更新済み
- [ ] [confidence: high] doc comment 抽出: Python の docstring (`"""..."""`) を tree-sitter で取得していない — AST の `expression_statement > string` ノードを処理する必要あり
- [ ] [confidence: medium] comment fixture corpus はまだ各言語を横断して揃っていない。JSDoc / rustdoc / Python docstring / plain comment の representative case を主要言語 fixture corpus に統合し、reference parser 比較 coverage の基盤にする

## BM25 / 検索品質

- [x] ~~`doc_freq` の計算が `search()` 呼び出しごとに `HashSet` を生成している~~ → `doc_freqs` フィールドに事前計算済み
- [ ] [confidence: medium] `node_type_boost` (probe のランキング手法) を未実装 — `function > class > enum > const` の重み付け
- [ ] [confidence: medium] クエリの全トークンがヒットしたドキュメントを優先する `coverage_boost` を未実装
- [ ] [confidence: low] 明示的な stopword 除去がない — 1文字トークンは落ちるが、"the", "is", "with" などの一般語はインデックスに残る

### 複合識別子のトークン分割問題（優先度: 高）

2026-04-07 dogfooding で発見。agent が「型名で定義を見つける」ユースケースで致命的。

- [ ] [confidence: high] **完全修飾名での検索が効かない。** `lbug_database_init` で検索すると0件、`database init` ならヒットする (LadybugDB dogfooding)。snake_case 識別子が `lbug` + `database` + `init` に分割され、完全一致ボーナスがないため。複合識別子の原形保持と同一の対策で解決可能
- [ ] [confidence: high] **複合識別子 (`ParserReturn`, `QueryCursor`, `SourceType` 等) がトークン分割されてBM25精度が崩壊する。** `ParserReturn` → `parser` + `return` に分割され、`parse_return_statement` 等のノイズが大量ヒットし、構造体定義が埋もれる。対策: (1) 分割前の原形をフルマッチ用フィールドとして別途インデックスに保持し、完全一致にボーナスを与える (2) camelCase/snake_case 分割トークンの proximity boost（分割元が同一識別子なら高スコア）
- [ ] [confidence: high] **`struct` / `enum` / `trait` / `class` 等のキーワードがBM25で汎用トークン扱いされ、`struct ParserReturn` で検索すると無関係な定義がノイズになる。** C++ では `enum class` が大量ヒットする問題も確認 (LadybugDB dogfooding)。対策: `--kind struct,class` フィルタを追加
- [ ] [confidence: high] **文字列リテラル内のテキストがインデックスされない。** `conn->query("MATCH (a:person) RETURN ...")` のCypher/SQLクエリが検索対象外 (LadybugDB dogfooding)。テストコード内のクエリ例を見つけるのはコード理解ツールの重要ユースケース。対策: tree-sitter AST の string literal ノードの内容もトークナイズしてインデックスに含める。フィールド重みは低く設定
- [ ] [confidence: medium] **`--ext "d.ts"` 等のドット付き拡張子が効かない。** `.d.ts` と `.ts` を区別できない (LadybugDB dogfooding)。対策: 拡張子マッチングで `.d.ts` のような複合拡張子をサポート
- [ ] [confidence: medium] **ドキュメントスニペットが80文字で切られて内容が判断できない。** 検索結果のdocスニペットが短すぎて、ファイルを読みに行くかの判断材料にならない (LadybugDB dogfooding)。対策: doc snippet を 200文字程度に拡張、または `--expand` でセクション全文を返す

## repoask-repo / CLI

- [x] ~~`InvertedIndex` の bincode ファイル保存/読み込みロジック~~ → `index_store.rs` で実装済み。現在は `postcard` ベース
- [x] ~~`IndexMeta` (commit hash, format version, timestamp)~~ → `index_store.rs` で実装済み
- [x] ~~`repo.rs` の `load_or_build_index()` 内で `unwrap_or(Path::new(""))` を使用~~ → `if let Some(parent)` パターンに修正済み
- [x] ~~`clone.rs` の atomic rename はクロスファイルシステムで失敗する~~ → rename 失敗時に `copy_dir_recursive` + 削除にフォールバック
- [x] ~~`clone.rs` で clone 失敗時の tmp ディレクトリ cleanup が `let _ =` で無視されている~~ → rename 失敗時にも tmp cleanup を追加済み
- [x] ~~`repo.rs` のファイルロック解放が `let _ = lock_file.unlock()` で明示的にされているが、`Drop` で十分~~ → `drop(lock_file)` に変更済み
- [x] ~~CLI の `main.rs` で `eprintln!` を直接使っている~~ → `#![allow(clippy::print_stderr, reason = "CLI binary")]` で許可済み
- [x] ~~CLI に `--dir` / `--ext` / `--type` フィルタオプションが未実装 (仕様 S2)~~ → `repoask search` に directory / extension / result-type filter を追加済み。`--dir` / `--ext` は repeat/comma 両対応、`--type` は `code|doc`
- [ ] [confidence: high] 独立 `extract` サブコマンドは追加しない。raw code 取得は symbol inspect mode に統合する
- [x] ~~CLI に `overview` が未実装 — README 要約ではなく、公開 API / 型 / 公開 API のテストを優先して返す~~ → `overview` subcommand を実装し、README 要約ではなく `InvestigationOverview` を JSON/text で返すようにした
- [ ] [confidence: medium] search の出力が investigation card ではなく単純 result list のまま — `why surfaced` / `next` / linked tests / future graph boost を載せる

### search 出力の情報密度問題（優先度: 高）

2026-04-07 dogfooding で発見。現状の search 出力は agent の tool call 数を増やすだけで、「1回の検索で仕様がわかる」にはほど遠い。

**問題の具体例:**
- `Parser::parse()` の返り値型を知りたい → 現状: search で行番号だけ得る → sed/read でソース読み → 型の定義が別ファイル → 再検索... → 4-6 tool calls
- `StreamingIterator::next()` の型が知りたい → `next` が4件ヒットし、どの構造体のメソッドかわからない
- Context7 なら同じ調査が 2 tool calls で完結する

**改善案:**
- [ ] [confidence: high] **search 結果にコードスニペットを含める。** 現状は `filepath`, `start_line`, `end_line` のみ。シグネチャ行 + 構造体フィールド + doc comment の先頭数行を含めれば、1回の search で型の構造がわかる。トークン予算を考慮し、`--context N` オプションでスニペット行数を制御
- [ ] [confidence: high] **search 結果に親コンテナ情報を含める。** `next` メソッドがヒットしたら、それがどの `impl` ブロックのメソッドか (`impl StreamingIterator for QueryMatches`) を付与する。現状の `Symbol.export.container` はクラス名のみ。impl target type / trait も含める
- [ ] [confidence: medium] **overview の公開API選定精度が低い。** oxc の overview で `stripInternalFunction`（テストfixture）が最重要APIとして出る。test fixture / example のパスパターンを負のブーストとして扱うか、`tests/fixtures/` 配下を public API 候補から除外する
- [ ] [confidence: medium] **`--kind` フィルタの追加。** `--kind struct,trait` のようにシンボル種別でフィルタできれば、`struct ParserReturn` で検索して型定義だけに絞れる
- [x] ~~`search` の text 出力で snippet が80文字で切られるが、マルチバイト文字で壊れる可能性~~ → `.chars().take(80)` で文字単位切り出し済み。バイト切断ではないためマルチバイト安全
- [ ] [confidence: medium] キャッシュの staleness チェックがコミットハッシュ一致のみ — 時間ベースの invalidation (`IndexMeta.is_stale()`) が未実装
- [ ] [confidence: high] **0件ヒット時のフィードバックがない。** `--dir tools/rust_api` で検索しても0件、「クエリが悪いのか」「ディレクトリが空なのか」「ファイルが対象外なのか」判断できない (LadybugDB dogfooding)。対策: 0件時に `hint: N files indexed in this directory` / `hint: directory is empty` / `hint: no files matched extensions` 等のダイアグノスティクスを stderr に出す
- [ ] [confidence: medium] **clone失敗時のエラーメッセージが不親切。** `git clone failed` だけでは 404 なのかネットワークエラーなのか判断できない (LadybugDB dogfooding)。対策: git stderr をパースして `repository not found` / `network error` 等の解釈を追加
- [x] ~~index と corpus の cache metadata が同じ `index.meta.json` 依存のまま。現状は `corpus.bin` 側の format mismatch を self-heal rebuild するが、artifact ごとの compatibility / staleness state はまだ分離されていない~~ → `index.meta.json` と `corpus.meta.json` を分離し、index/corpus ごとに commit/format metadata を持つ形へ更新済み。cache path の distinct test も追加

## テスト

- [x] ~~fixtures ベースの E2E テストがない — サンプルリポジトリで `parse_directory` → `InvertedIndex::build` → `search` を通しで検証する~~ → `crates/repoask-repo/tests/e2e_fixture_repo.rs` と sample fixture repo を追加し、`parse_directory` → `InvertedIndex::build` → `search` に加えて `build_overview()` の selection まで通しで検証済み
- [ ] [confidence: high] tree-sitter の Java / C / C++ の representative test がまだ薄い。Ruby は `Module` kind まで入ったが、主要言語 fixture corpus の一部として cross-language baseline test に吸収して強化する
- [ ] [confidence: medium] 大規模ファイル (10,000行超) でのパフォーマンステストなし
- [ ] [confidence: medium] BM25 のランキング品質テストを拡充する — 基本順位テストはあるが、複数トークン / code-doc 混在 / field-weight 差分の期待順を fixture で固定化したい
- [x] ~~`index_store` の save/load ラウンドトリップテストが未作成~~ → `index_save_load_roundtrip`, `meta_save_load_roundtrip`, `empty_index_roundtrip` 追加済み
- [ ] [confidence: high] `clone.rs` / `repo.rs` のテストは repo spec パースのみ — clone 自体のテストはネットワーク依存で未作成

## SKILL.md / Agent Skills

原則: **SKILL.md は必要最低限の文量に保つ。** agent のコンテキストウィンドウを圧迫しないことが最優先。agentが行動を起こすのに不要な説明・背景・Tipsは削るか、別ファイル (README等) に移す。

- [x] ~~`SKILL.md` 作成~~ → 実装済み

### ~~削減対象 (現在122行 → 目標60行以下)~~ → 129行→60行に削減済み

- [x] ~~**「What this tool does」セクション削除。**~~ → 1文のサマリーに圧縮
- [x] ~~**「When to use」を1-2行に圧縮。**~~ → 1文に圧縮
- [x] ~~**「Supported languages」セクション削除。**~~ → 削除済み
- [x] ~~**「Usage patterns」セクションの3つの例を削除。**~~ → 1例に削減
- [x] ~~**「Read a specific code range after search」セクション削除。**~~ → 削除済み
- [x] ~~**「Tips」セクション削除またはOutput formatに統合。**~~ → score注記のみOutput formatに統合
- [x] ~~**cleanup の説明を1行に圧縮。**~~ → Commands セクション内に統合
- [x] ~~**Output format の3つのJSON例を1つの統合例に。**~~ → フィールド表 + 統合例1つに変更

### ~~不足情報の追記 (最小限で)~~ → 追記済み

- [x] ~~エラー時の振る舞い: exit code 1 + stderr にエラーメッセージ (1行で記載)~~ → Commands セクションに追記
- [x] ~~JSON lines: 各行が独立したJSONオブジェクト (Output format に1文追記)~~ → Output format セクションに追記

## ハーネス改善

### similarity-rs 統合ロードマップ

similarity-rs による重複検出。pre-commit で `--fail-on-duplicates` で常時ブロック。

**現在**: threshold 0.96 (関数) / 0.95 (型)。pre-commit gate 有効。

**解消済み**:
- [x] ~~`oxc.rs` — `extract_from_statement` ↔ `extract_from_declaration` (98%)~~ → `Statement::as_declaration()` で委譲、`ExtractCtx` + `ctx.push()` で統一
- [x] ~~`tokenizer.rs` — `tokenize_text` ↔ `tokenize_identifier` (97%)~~ → 共通の `stem_tokens()` に抽出
- [x] ~~`ParseOutcome` 重複 (100%)~~ → `repoask-core::types` に移動、両パーサーが `pub use` で re-export
- [x] ~~`CodeResult` ↔ `ExampleResult` (98%)~~ → `ExampleResult` 除去、`CodeResult.is_example: bool` に統合。型 threshold 0.95 に引き下げ

**残存 (threshold 引き下げ時に対処)**:
- [ ] [confidence: low] **`oxc.rs` の `extract_from_var_decl` と `extract_class_methods` が similarity hook で再度重複扱いされるか監視する。** 現状はドメインが別なので、重複率だけを理由に無理な共通化はしない
- [ ] [confidence: low] **test helper (`make_symbol` / `make_doc`) が similarity hook のノイズ源になるなら整理する。** 共通 helper ファイルへの切り出しか、hook 側の除外設定見直しで対処する

**FP 対策**: `--exclude benches` で bench コードを除外済み。

**目標**: 段階的に threshold 0.80 まで引き下げ、より攻めた重複検出

### CI / CD

- [x] ~~**SHA pin の定期更新。**~~ → `.github/dependabot.yml` を追加し、`github-actions` ecosystem を weekly に自動更新
- [x] ~~**`missing_docs` を `warn` → `deny` に昇格。**~~ → `Cargo.toml` で `missing_docs = "deny"` に変更済み
- [x] ~~**coverage ratchet の実装。**~~ → `.metrics/coverage-baseline` + `scripts/check-coverage-ratchet.sh` を追加し、CI Coverage job で baseline 未満を fail
- [ ] [confidence: medium] **CodSpeed 連携。** divan ベンチマークを `codspeed-divan-compat` に切り替えて PR コメントで回帰検出を自動化
- [x] ~~**cargo-mutants の全体実行。**~~ → CI Gate 5 で PR diff に対する mutation testing 実装済み (`cargo mutants --in-diff`)。weekly 全体実行は未実装だが diff ベースで十分カバーされている
- [x] ~~**cargo-machete を CI に追加。**~~ → 現在は CI Gate 1 (check) で `cargo shear` に移行済み
- [x] ~~**similarity-rs を pre-commit に追加。**~~ → prek.toml に `similarity-fn` (threshold 0.96) + `similarity-types` (threshold 0.95) hook 追加済み。`--fail-on-duplicates` で常時ブロック
- [x] ~~**ベンチマーク CI。**~~ → `benchmarks.yml` で main push / PR 時に divan ベンチマーク自動実行済み (tokenizer, indexing, search の3ファイル)
- [x] ~~**release-plz workflow。**~~ → `release-plz.yml` で main push 時に自動 Release PR 作成 + crates.io publish 実装済み
- [x] ~~**バイナリ配布 workflow。**~~ → `release.yml` で tag push 時に linux/macos/windows の5ターゲットにクロスビルド + GitHub Release アップロード実装済み

### flake.nix / 開発環境

- [x] ~~**direnv 連携。**~~ → `.envrc` に `use flake` 追加済み
- [ ] [confidence: low] **flake.lock の定期更新。** `nix flake update` を月1で実行し、nixpkgs のパッケージバージョンを追従
- [x] ~~**cargo-modules を devShell に追加。**~~ → `flake.nix` に `cargo-modules` 追加済み
- [x] ~~**cargo-machete を devShell に追加。**~~ → 現在は `cargo-shear` / `cargo-udeps` を devShell に含めている

### テスト強化

- [x] ~~**insta snapshot を repoask-parser にも追加。**~~ → `snapshot_mixed_typescript` (oxc: class/method/interface/type/arrow fn/enum 網羅) + `snapshot_readme_like` (markdown: 階層見出し/コードブロック/ネスト) 追加済み
- [x] ~~**proptest: index serialize/deserialize roundtrip。**~~ → `repoask-repo/src/index_store.rs` に `index_save_load_roundtrip` + `meta_save_load_roundtrip` + `empty_index_roundtrip` 追加済み (tempfile 使用)
- [x] ~~**proptest: BM25 score 非負。**~~ → `repoask-core/src/index.rs` に proptest 3件追加: `scores_are_non_negative`, `result_count_within_limit`, `results_sorted_by_score`
- [x] ~~**integration test: parse → index → search E2E。**~~ → fixtures ベース E2E 項目に統合し、sample repo で `search` / `overview` まで通し検証する形に更新済み

### エラーハンドリング

- [ ] [confidence: low] **repoask-core のエラーモデルを明示的に決める。** core を今のまま infallible に保つか、入力バリデーションを入れるなら最小限の `BuildError` / `ValidationError` を導入する
- [x] ~~**`ParseOutcome` → `ParseReport` の集約をCLIに露出。**~~ → `repoask search --verbose` で再構築時の parse summary と unsupported/failed/oversized file 一覧を stderr 表示するようにした。cache hit 時は parse report unavailable を表示

### ベンチマーク

- [x] ~~**tokenizer / indexing / search ベンチ。**~~ → `crates/repoask-core/benches/` に divan ベンチ3ファイル実装済み。CI で自動実行
- [ ] [confidence: medium] **serialization ベンチの追加。** postcard encode/decode の速度を計測（仕様M4「インデックスキャッシュ読み込み0秒」の検証）
- [ ] [confidence: medium] **大規模リポジトリでのE2Eベンチ。** 実際のOSSリポジトリ（e.g. vercel/next.js）でインデックス構築3秒以内、検索100ms以内を検証

### リリース

- [x] ~~**`release-plz.toml` の設定。**~~ → Conventional Commits パーサー、version_group によるワークスペース統一バージョニング、CHANGELOG テンプレート、semver_check 設定済み。ドライランは初回 push 時に自動検証される
- [x] ~~**crates.io のパッケージ名予約。**~~ → `repoask`, `repoask-core`, `repoask-parser`, `repoask-treesitter`, `repoask-repo` は crates.io に `0.1.0` を publish 済み
- [ ] [confidence: low] **`panic = "abort"` の検討。** CLI バイナリのサイズ削減（~100KB）。`catch_unwind` を使っていないなら安全

## Type-Aware Search（優先度: 最高）

2026-04-07 dogfooding で導出。repoask が Context7 に勝つための核心機能。

### 背景

Context7 は「README/docs からの使い方例」を返す。repoask は「実際のソースコードのAST」にアクセスできる。
この差を活かして、「1回のsearchで型の構造も依存関係も全部分かる」出力を実現する。

### ユースケースと現状のギャップ

| ユースケース | Context7 | repoask (現状) | repoask (理想) |
|---|---|---|---|
| 「Parser::parse() の戻り値型のフィールドは？」 | ✘ (例だけ) | 4-6 calls | **1 call** |
| 「next() メソッドはどのimplの？」 | ✘ | 2-3 calls | **1 call** |
| 「このエラーの原因型は？」 | ✘ | 3-4 calls | **1 call** |
| 「この型を使ってる関数は？」 | ✘ | 不可能 | **1 call** |

### Step 1: Symbol 型の拡張 [confidence: high]

```rust
// params: Vec<String> → Vec<ParamInfo> に拡張
pub struct ParamInfo {
    pub name: String,
    pub type_text: Option<String>,  // "SourceType", "&str", "Vec<u8>"
}

// NEW: 構造体/インターフェースのフィールド
pub struct FieldInfo {
    pub name: String,
    pub type_text: String,
    pub visibility: Publicness,
}

// NEW: impl/class コンテナ
pub struct ContainerInfo {
    pub name: String,               // "Parser"
    pub kind: ContainerKind,        // Impl / Class / Module
    pub trait_name: Option<String>, // "StreamingIterator" for trait impls
}

pub struct Symbol {
    // 既存...
    pub return_type: Option<String>,      // NEW: 戻り値型テキスト
    pub fields: Vec<FieldInfo>,           // NEW: struct/interface のフィールド
    pub container: Option<ContainerInfo>, // NEW: export.container を置換
    pub params: Vec<ParamInfo>,           // CHANGE: 名前+型に拡張
}
```

技術検証済み:
- tree-sitter: `function_item → -> → type_identifier` で戻り値型取得可能
- tree-sitter: `struct_item → field_declaration_list → field_declaration` でフィールド名+型取得可能
- tree-sitter: `impl_item → type: type_identifier` でimplターゲット型取得可能
- oxc: `Function.return_type`, `FormalParameter.type_annotation` でTS/JSも同等

### Step 2: tree-sitter クエリ拡張 [confidence: high]

Rust クエリに追加キャプチャ:
```
(function_item
  name: (identifier) @name
  parameters: (parameters) @params
  return_type: (_)? @return_type) @definition.function

(struct_item
  name: (type_identifier) @name
  body: (field_declaration_list)? @fields) @definition.struct

(impl_item
  type: (type_identifier) @impl_target
  trait: (type_identifier)? @impl_trait
  body: (declaration_list
    (function_item
      name: (identifier) @name
      parameters: (parameters) @params
      return_type: (_)? @return_type) @definition.method))
```

同様に Python (type hint), Go (return type), Java (return type), TS/JS (oxc) も拡張

### Step 3: 型依存グラフの構築と自動展開 [confidence: high]

インデックス構築後に、型名 → 定義シンボルの逆引きマップを構築:

```rust
// インデックス内の全 Symbol から:
type_definitions: HashMap<String, Vec<SymbolRef>>  // "ParserReturn" → [struct@lib.rs:280]
type_usages: HashMap<String, Vec<SymbolRef>>       // "ParserReturn" → [parse()@lib.rs:332, ...]
```

search の post-processing で自動展開:
1. 関数がヒット → `return_type` を取得
2. `return_type` の名前で `type_definitions` を O(1) lookup
3. 見つかった Struct の `fields` を結果に添付

これで「`Parser::parse()` の戻り値は `ParserReturn { program: Program, panicked: bool, errors: Vec<OxcDiagnostic> }`」が **1 call** で分かる。

### Step 4: 2モード出力設計 [confidence: high]

デフォルトは省略モード（型シグネチャ + フィールド一覧 + doc summary）。`--expand` で全ソースコード + フィールドのdocまで展開。

**使い分け:**
- デフォルト: 「APIの仕様を把握したい」— シグネチャ、型構造、依存先がわかればいい
- `--expand`: 「内部ロジックを読みたい」— バグ調査、実装の理解、エッジケース確認

#### text 出力例

**デフォルト（省略）:**
```
$ repoask search oxc-project/oxc "Parser parse" --dir crates/oxc_parser -f text

[method] Parser::parse(self) -> ParserReturn<'a>
  crates/oxc_parser/src/lib.rs:333-345  score: 43.0
  doc: Main entry point. Returns an empty Program on unrecoverable error.
  └─ ParserReturn<'a> (struct, :145)
       program: Program<'a>
       module_record: ModuleRecord<'a>
       errors: Vec<OxcDiagnostic>
       panicked: bool
       is_flow_language: bool

[method] Parser::parse_expression(self) -> Expression<'a>
  crates/oxc_parser/src/lib.rs:364-375  score: 36.3
  doc: Parse a single Expression.
  └─ Expression<'a> (enum, crates/oxc_ast/src/ast/js.rs:250)
```

**`--expand`（全量展開）:**
```
$ repoask search oxc-project/oxc "Parser parse" --dir crates/oxc_parser --expand -f text

[method] Parser::parse(self) -> ParserReturn<'a>
  crates/oxc_parser/src/lib.rs:333-345  score: 43.0
  doc: Main entry point. Returns an empty Program on unrecoverable error,
       Recoverable errors are stored inside `errors`.
  ┌─────────────────────────────────────────────
  │ pub fn parse(self) -> ParserReturn<'a> {
  │     let unique = UniquePromise::new();
  │     let parser = ParserImpl::new(
  │         self.allocator,
  │         self.source_text,
  │         self.source_type,
  │         self.options,
  │         self.config,
  │         unique,
  │     );
  │     parser.parse()
  │ }
  └─────────────────────────────────────────────
  └─ ParserReturn<'a> (struct, :145)
       /// The parsed AST. Will be empty if the parser panicked.
       pub program: Program<'a>
       /// Syntax errors encountered while parsing.
       pub errors: Vec<OxcDiagnostic>
       /// Whether the parser panicked and terminated early.
       pub panicked: bool
       /// Whether the file is flow.
       pub is_flow_language: bool
```

#### JSON 出力例

**デフォルト（省略）— `code` フィールドなし、フィールドdocなし:**
```json
{
  "kind": "Method",
  "name": "parse",
  "container": {"name": "Parser", "trait": null},
  "filepath": "crates/oxc_parser/src/lib.rs",
  "start_line": 333, "end_line": 345,
  "signature": "pub fn parse(self) -> ParserReturn<'a>",
  "return_type": "ParserReturn<'a>",
  "params": [{"name": "self", "type": "Self"}],
  "doc_summary": "Main entry point. Returns an empty Program on unrecoverable error.",
  "type_deps": [{
    "name": "ParserReturn",
    "kind": "Struct",
    "filepath": "crates/oxc_parser/src/lib.rs",
    "line": 145,
    "fields": [
      {"name": "program", "type": "Program<'a>"},
      {"name": "module_record", "type": "ModuleRecord<'a>"},
      {"name": "errors", "type": "Vec<OxcDiagnostic>"},
      {"name": "panicked", "type": "bool"},
      {"name": "is_flow_language", "type": "bool"}
    ]
  }],
  "score": 43.0
}
```

**`--expand` — `code` フィールド追加、フィールドにdoc追加:**
```json
{
  "kind": "Method",
  "name": "parse",
  "container": {"name": "Parser", "trait": null},
  "filepath": "crates/oxc_parser/src/lib.rs",
  "start_line": 333, "end_line": 345,
  "signature": "pub fn parse(self) -> ParserReturn<'a>",
  "return_type": "ParserReturn<'a>",
  "params": [{"name": "self", "type": "Self"}],
  "doc_summary": "Main entry point. Returns an empty Program on unrecoverable error.",
  "code": "pub fn parse(self) -> ParserReturn<'a> {\n    let unique = ...\n    parser.parse()\n}",
  "type_deps": [{
    "name": "ParserReturn",
    "kind": "Struct",
    "filepath": "crates/oxc_parser/src/lib.rs",
    "line": 145,
    "fields": [
      {"name": "program", "type": "Program<'a>", "doc": "The parsed AST. Will be empty if the parser panicked."},
      {"name": "errors", "type": "Vec<OxcDiagnostic>", "doc": "Syntax errors encountered while parsing."},
      {"name": "panicked", "type": "bool", "doc": "Whether the parser panicked and terminated early."}
    ]
  }],
  "score": 43.0
}
```

#### デフォルト vs --expand の差分

| | デフォルト（省略） | `--expand`（全量） |
|---|---|---|
| **コード本体** | なし | 関数ボディ全量 |
| **シグネチャ** | ✓ 完全シグネチャ | ✓ 完全シグネチャ |
| **doc comment** | summary 1行 | 全文 |
| **type_deps fields** | 名前+型のみ | 名前+型+フィールドdoc |
| **トークン消費** | ~200 tokens/result | ~500-2000 tokens/result |
| **用途** | API仕様把握、型構造の確認 | バグ調査、実装理解 |

### 個別タスク

- [ ] [confidence: high] `Symbol` に `return_type: Option<String>` を追加
- [ ] [confidence: high] `Symbol` に `fields: Vec<FieldInfo>` を追加
- [ ] [confidence: high] `Symbol.params` を `Vec<ParamInfo>` （名前+型）に拡張
- [ ] [confidence: high] `Symbol.export.container` を `ContainerInfo` に置換（impl target + trait）
- [ ] [confidence: high] tree-sitter Rust クエリに `@return_type`, `@fields`, `@impl_target`, `@impl_trait` キャプチャ追加
- [ ] [confidence: high] oxc 抽出器に `Function.return_type`, `FormalParameter.type_annotation` の抽出追加
- [ ] [confidence: high] インデックスに `type_definitions` / `type_usages` 逆引きマップを構築
- [ ] [confidence: high] search の post-processing で `return_type` → 型定義 → fields の自動展開
- [ ] [confidence: high] `--expand` フラグ: ヒットしたシンボルのソースコード全量を clone 元から読み出して `code` フィールドに含める
- [ ] [confidence: high] `--expand` 時に type_deps のフィールドに doc comment も添付
- [ ] [confidence: medium] 逆引き: 型名で検索したとき、その型を引数/戻り値に使う関数リストを添付
- [ ] [confidence: medium] 主要言語 (Python/Go/Java/Ruby/C++) でも型情報抽出を拡張

## Code Graph & Trace（優先度: 高）

2026-04-07 dogfooding で導出。Type-Aware Search の上位層。

### 背景: 3層構造

repoask の情報提供は 3層で構成される。各層は `--expand` で全量展開可能。

| Layer | コマンド | 問い | デフォルト（省略） | `--expand`（全量） |
|---|---|---|---|---|
| **1** | `search` | 何がどこにあるか + 型の契約 | シグネチャ + 型フィールド + doc summary | + 関数ボディ全量 + フィールドdoc |
| **2** | `trace` | この関数/型から何が影響を受けるか | callee/caller一覧（シグネチャのみ） | + calleeのソース全量 |
| **3** | `overview` | このrepoの入口はどこか | 公開API/型/テスト一覧 | (同左) |

### ユースケースと各層の対応

| ユースケース | 層 | 例 |
|---|---|---|
| API仕様の把握 | search | `repoask search oxc-project/oxc "Parser parse"` |
| 内部ロジックの理解 | search --expand | `repoask search ... --expand` |
| エラーの原因型を追跡 | trace | `repoask trace ... "#validate_token"` |
| 型変更の影響範囲 | trace | `repoask trace ... "#ParserReturn"` |
| repoの全体像 | overview | `repoask overview oxc-project/oxc` |

### trace が構築するグラフ

tree-sitter / oxc の AST から 3種のエッジを抽出:

1. **Call graph** — 関数Aが関数Bを呼ぶ
   - tree-sitter: `call_expression` の `function` フィールドで呼び出し先名取得 (検証済み: Rust/Python)
   - oxc: `CallExpression.callee` で TS/JS の呼び出し先取得

2. **Type dependency graph** — 型Aが型Bを参照
   - 構造体フィールドの型、関数の引数/戻り値型から抽出
   - Type-Aware Search の Step 1-3 で既に構築される `type_definitions` / `type_usages` マップをそのまま使う

3. **Import graph** — ファイルAがファイルBを import
   - tree-sitter: `use_declaration` (Rust), `import_statement` (Python/JS)
   - oxc: `ImportDeclaration` で TS/JS の import 解析

技術検証:
- tree-sitter Rust: `call_expression` → `function` フィールドで `decode_jwt`, `verify_signature` 等の呼び出し先名を取得確認済み
- tree-sitter Python: `call` → `function` フィールドで `authenticate`, `validate_payload` 等取得確認済み
- メソッドコール (`obj.method()`) のレシーバ型解決は非ゴール。名前ベースのヒューリスティックマッチで十分

### trace 出力設計

#### デフォルト（省略）
```
$ repoask trace oxc-project/oxc "crates/oxc_parser/src/lib.rs#Parser::parse"

[trace] Parser::parse(self) -> ParserReturn<'a>
  crates/oxc_parser/src/lib.rs:333-345

  callees:
    ParserImpl::new(...) -> ParserImpl        :420
    ParserImpl::parse(self) -> ParserReturn   :562

  callers:
    parse_program_smoke_test()                :693
    (+ 84 more in 23 files)

  type_deps:
    ParserReturn<'a> (struct, :145) — return type
      program: Program<'a>
      errors: Vec<OxcDiagnostic>
      panicked: bool
```

#### `--expand`（全量展開）
```
$ repoask trace oxc-project/oxc "...#Parser::parse" --expand

[trace] Parser::parse(self) -> ParserReturn<'a>
  ┌─────────────────────────────
  │ pub fn parse(self) -> ParserReturn<'a> {
  │     let unique = UniquePromise::new();
  │     let parser = ParserImpl::new(...);
  │     parser.parse()
  │ }
  └─────────────────────────────

  callees:
    [1] ParserImpl::new(...) -> ParserImpl
        :420-445  doc: Create a new parser implementation.
        ┌─ pub(crate) fn new(...) -> Self { ... } ─┐

    [2] ParserImpl::parse(self) -> ParserReturn<'a>
        :562-593  doc: Main parse loop.
        ┌─ pub(crate) fn parse(mut self) -> ParserReturn<'a> { ... } ─┐

  callers (top 5 / 85):
    parse_program_smoke_test()  :693
    test_parse_basic()          tests/basic.rs:15
    ...

  type_deps:
    ParserReturn<'a> (struct, :145)
      /// The parsed AST.
      pub program: Program<'a>
      /// Syntax errors.
      pub errors: Vec<OxcDiagnostic>
      /// Whether the parser panicked.
      pub panicked: bool
```

### 実測データ: 呼び出しパターン分布 (2026-04-07 repoask 自身で計測)

repoask の Rust crates 全体 (1,976 呼び出し) を tree-sitter で解析した結果:

| パターン | 例 | 割合 | 名前解決難度 |
|---|---|---|---|
| `bare_function` | `tokenize_query(...)` | 21.5% | 簡単: 同スコープ/import の関数名で一致 |
| `scoped_static` | `Bm25Scorer::new(...)` | 16.3% | 簡単: 型名::method で一意に特定 |
| `self_method` | `self.matches_filters(...)` | 2.8% | 簡単: impl ブロックの Self 型から特定 |
| `variable_method` | `scorer.score(...)` | 58.5% | 難: 変数の型推論が必要 |

### 実測データ: メソッド名の一意性 (2026-04-07)

repoask crates 全体で関数/メソッド名の重複度を計測:

- **総数**: 315 個の異なる関数/メソッド名
- **一意 (1定義)**: 267 個 (**84.8%**)
- **曖昧 (2+定義)**: 48 個 (15.2%)

曖昧な名前の上位:
- `new` (12件) → `Type::new()` のコンテナ名で区別可能
- `default` (6件) → 同上
- `score` (4件) → `Bm25Scorer::score` / `SearchResult::score` 等、コンテナで区別

**結論**: メソッド名だけのヒューリスティックで 84.8% 解決。コンテナ名 (`Type::method`) を併用すれば 95%+ の見込み。

### 実測データ: 名前解決アプローチ別カバレッジ

| アプローチ | 解決率 | 実装量 | 備考 |
|---|---|---|---|
| easy only (bare + scoped + self) | 38.5% | ~100行 | 最低限だが有用 |
| + メソッド名一意解決 | 84.8% | ~180行 | **推奨: 十分実用的** |
| + コンテナ名 (Type::method) | 95%+ | ~260行 | 曖昧な名前も解決 |
| + ローカル型推論 (let x = T::new()) | 98%+ | ~400行 | 将来の拡張 |

### 実測データ: グラフストレージの選択 (2026-04-07)

LadybugDB (Kuzu fork, embedded graph DB) と HashMap 自前実装を実測比較:

| | HashMap (自前) | LadybugDB | 差 |
|---|---|---|---|
| グラフ構築 (10K nodes, 30K edges) | **1.8ms** | 6.0s | 3,300× |
| 直接 callee クエリ | **60ns** | 4.9ms | 80,000× |
| 2-hop 走査 | **601ns** | 4.7ms | 8,000× |
| 3-hop 走査 | **2µs** | 4.7ms | 2,000× |
| caller 逆引き | **30ns** | 3.1ms | 100,000× |
| バイナリサイズ増分 | **+0 KB** | +6 MB | - |
| ビルド時間増分 | **+0s** | +7min (初回) | - |
| 追加依存 | なし | cmake + C++ | - |
| WASM | そのまま動く | Emscripten別ビルド | - |

**結論**: repoask のスケール (1 repo = 数千〜数万ノード) では HashMap が全次元で圧勝。LadybugDB は数百万ノード+永続化+複雑な Cypher が必要な場合の将来の選択肢として記録。

### 実装ステップ

#### Step 1: Call extraction の基盤 [confidence: high]

実装量見積り: ~100行

- [ ] `CallInfo` 型を `repoask-core::types` に追加:
  ```rust
  pub struct CallInfo {
      pub callee_name: String,             // "decode_jwt" or "self.parse"
      pub callee_receiver: Option<String>, // "parser" for parser.parse()
      pub callee_container: Option<String>, // "Bm25Scorer" for Bm25Scorer::new()
      pub kind: CallKind,                  // BareFunction / ScopedStatic / SelfMethod / VariableMethod
      pub line: u32,
  }

  pub enum CallKind {
      BareFunction,     // tokenize_query(...)
      ScopedStatic,     // Bm25Scorer::new(...)
      SelfMethod,       // self.matches_filters(...)
      VariableMethod,   // scorer.score(...)
  }
  ```
- [ ] `Symbol` に `calls: Vec<CallInfo>` を追加（その関数が呼ぶ関数のリスト）
- [ ] tree-sitter Rust: `call_expression` の `function` フィールドから callee 名抽出。`identifier` (bare), `scoped_identifier` (static), `field_expression` (method) を分類
- [ ] tree-sitter Python: `call` の `function` フィールドから callee 名抽出
- [ ] oxc: `CallExpression.callee` / `MemberExpression` から TS/JS の callee 名抽出
- [ ] std メソッドのフィルタリング: `.len()`, `.is_empty()`, `.unwrap()`, `.collect()` 等の汎用メソッドはノイズなので除外リストを用意

#### Step 2: 名前解決とグラフ構築 [confidence: high]

実装量見積り: ~180行

名前解決の戦略 (実測根拠あり):
1. **BareFunction** (21.5%): 同ファイル内 → 同モジュール内 → import 先の順で検索
2. **ScopedStatic** (16.3%): `Type::method` の Type 名でインデックス内を lookup、その impl ブロックの method を特定
3. **SelfMethod** (2.8%): impl ブロックの Self 型から特定
4. **VariableMethod** (58.5%): メソッド名でインデックス内を lookup。**84.8% が名前だけで一意に解決**。曖昧な場合は候補リストを返す

グラフストレージ:
```rust
// グラフ構築 O(n)、クエリ O(1)+O(k)
// 10K nodes + 30K edges で 1.8ms で構築、クエリ 60ns〜2µs (実測済み)
call_graph: HashMap<SymbolRef, Vec<SymbolRef>>          // caller → callees
reverse_call_graph: HashMap<SymbolRef, Vec<SymbolRef>>  // callee → callers
name_index: HashMap<String, Vec<SymbolRef>>             // 名前 → 定義シンボル群
```

- [ ] インデックス構築後に `name_index` を作成 (全 Symbol の名前 → SymbolRef)
- [ ] `call_graph` / `reverse_call_graph` を構築: 各 Symbol の `calls` を走査し、callee_name で `name_index` を lookup
- [ ] 曖昧解消: 同名メソッドが複数ある場合、`callee_container` が一致するものを優先。それでも曖昧なら全候補をエッジとして登録
- [ ] 型依存グラフは Type-Aware Search の `type_definitions` / `type_usages` をそのまま流用
- [ ] import グラフ: `use`/`import` 文からファイル間の依存関係を抽出

#### Step 3: trace コマンド実装 [confidence: high]

実装量見積り: ~200行

- [ ] `trace` サブコマンドの実装: target シンボルを起点に callees/callers/type_deps を展開
- [ ] `--expand` フラグ: callee/caller のソースコード全量を clone 元から読み出し
- [ ] `--depth N` フラグ: グラフ展開の深さ制御（デフォルト: 1）
- [ ] JSON 出力: callees/callers をシグネチャ+場所付きで返却
- [ ] text 出力: デフォルトはシグネチャのみ、`--expand` でコードブロック付き

#### Step 4: search との統合 [confidence: medium]

- [ ] search の type_deps に callee/caller count を追加（「この型は 85 箇所から使われている」）
- [ ] overview に「最も呼び出される関数 TOP-5」を追加（graph centrality）
- [ ] Boost Signals に graph centrality を組み込み、search ranking を改善

### 制約と割り切り

- メソッドコールのレシーバ型解決はしない。`obj.method()` は名前 `method` でインデックス内を lookup するヒューリスティック。実測で 84.8% が名前だけで一意解決可能であることを確認済み
- 動的ディスパッチ、コールバック、クロージャの解析はスコープ外
- LSP 等の外部ツールに依存しない。tree-sitter + oxc の AST だけで完結
- グラフストレージは `HashMap` 自前実装。LadybugDB 等の外部 graph DB は現時点では不要 (実測根拠あり)。複数 repo 横断グラフ (数十万ノード規模) が必要になった時点で再検討
- 名前ベースのマッチングは偽陽性があるが、「完全を目指さず 95% のケースをカバー」の方針 (実測で裏付け済み)

### 全体実装量見積り

| Step | 内容 | 行数 |
|---|---|---|
| Step 1 | Call extraction (tree-sitter/oxc) | ~100行 |
| Step 2 | 名前解決 + グラフ構築 | ~180行 |
| Step 3 | trace コマンド + 出力 | ~200行 |
| Step 4 | search/overview 統合 | ~100行 |
| **合計** | | **~580行** |

## 未実装の crate (Step 4 以降)

- [x] ~~`repoask-wasm`: wasm-bindgen エントリポイント~~ → `RepoIndex` (addFile/build/search/docCount) 実装済み
- [x] ~~**WASM ビルドの修正。**~~ → `wasm/Cargo.toml` の `wasm-opt` を `["-O", "--enable-bulk-memory", "--enable-nontrapping-float-to-int"]` に設定。Rust コンパイラが生成する `memory.copy`/`memory.fill`/`i32.trunc_sat` に対応する feature フラグを明示的に有効化し、binaryen 125 でも wasm-opt 最適化が通るようにした
- [ ] [confidence: medium] `repoask-node`: napi-rs npm 配布
