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
- [x] ~~`search` の text 出力で snippet が80文字で切られるが、マルチバイト文字で壊れる可能性~~ → `.chars().take(80)` で文字単位切り出し済み。バイト切断ではないためマルチバイト安全
- [ ] [confidence: medium] キャッシュの staleness チェックがコミットハッシュ一致のみ — 時間ベースの invalidation (`IndexMeta.is_stale()`) が未実装
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

## 未実装の crate (Step 4 以降)

- [x] ~~`repoask-wasm`: wasm-bindgen エントリポイント~~ → `RepoIndex` (addFile/build/search/docCount) 実装済み
- [x] ~~**WASM ビルドの修正。**~~ → `wasm/Cargo.toml` の `wasm-opt` を `false` に設定し `wasm-pack build` を復旧。binaryen >= 126 で `wasm-opt = ["-O"]` を再有効化する
- [ ] [confidence: medium] `repoask-node`: napi-rs npm 配布
