# TODO

実装を前に進めるために置き去りにした項目。

## セキュリティ

### コマンドインジェクション / 引数インジェクション

- [x] ~~**`clone.rs:68` — owner/repo が git URL に直接埋め込まれる。**~~ → `ensure_clone()` 冒頭で `is_valid_name()` (`^[a-zA-Z0-9._-]+$` 相当) バリデーション追加済み
- [x] ~~**`clone.rs:73-75` — ref_spec が `--branch` に未検証で渡される。**~~ → `ensure_clone()` で先頭 `-` チェック追加済み

### パストラバーサル

- [x] ~~**`cache.rs:27-28` — owner/repo がファイルパスに直接結合される。**~~ → `repo_cache_dir()` に `is_safe_path_component()` assert 追加済み
- [x] ~~**`cache.rs:52-55` — `cleanup_all()` が `REPOASK_CACHE_DIR` 環境変数のパスを `remove_dir_all` する。**~~ → パスに "repoask" を含むかチェック追加済み

### 信頼境界

- [x] ~~**`index_store.rs:62-67` — キャッシュから bincode デシリアライズする際にサイズ制限がない。**~~ → `load_index()` に 500MB 上限チェック + `TooLarge` エラー追加済み
- [x] ~~**`index_store.rs:78-81` — `index.meta.json` の JSON デシリアライズに入力長制限がない。**~~ → `load_meta()` に 1MB 上限チェック追加済み
- [x] ~~**`parse_directory` — ファイルサイズ制限なし。**~~ → `parse_directory()` に 10MB 上限チェック + `oversized` レポート追加済み

### ディスク消費

- [ ] **キャッシュサイズ制限なし。** 多数のリポジトリを検索すると clone + index でディスクが無制限に消費される。LRU eviction や合計サイズ上限がない
- [ ] **`clone_fresh` の tmp ディレクトリが PID ベースの命名。** PID リサイクルで古い tmp が残っている場合、同じ名前で衝突する可能性。UUID を使うか、作成前にクリーンアップを確実にすべき（現状クリーンアップはあるが `remove_dir_all` の失敗は黙殺される）

## パフォーマンス

### 検索 (hot path)

- [ ] **`index.rs:194-198` — `doc_freq` が検索時に毎回 `HashSet` 生成で計算される。** postings リスト長ではなくユニークな doc_id 数が必要だが、これをインデックス構築時に事前計算して `HashMap<String, u32>` としてキャッシュすべき。現状は O(postings_per_term) の追加コスト
- [x] ~~**`index.rs:215-216` — 全スコア付きドキュメントをフルソートしてから truncate。**~~ → `BinaryHeap` + `ScoredDoc` による O(n log k) top-k 選択に置換済み
- [ ] **`index.rs:187` — `HashMap<DocId, f32>` が検索ごとに新規生成される。** 構造体にキャッシュして `clear()` で再利用するか、事前確保すべき（`search` が `&self` のため `&mut self` 変更 or `RefCell` が必要、API変更を伴う）
- [x] ~~**`bm25.rs:25-27` — `weights[field_id as usize]` に bounds check が入る。**~~ → `get().copied().unwrap_or(0.0)` に変更済み

### インデックス構築

- [x] ~~**`tokenizer.rs:49,62` — `Stemmer::create()` が `tokenize_text` / `tokenize_identifier` 呼び出しごとに生成される。**~~ → `thread_local!` で `Stemmer` をキャッシュ済み
- [ ] **`parse_directory` — シングルスレッドでファイルを逐次処理。** `WalkBuilder::build_parallel()` + `crossbeam::channel` でファイルウォークとパースを並列化すれば、マルチコアで構築時間を大幅短縮可能。仕様の「3秒以内」を大規模リポジトリで達成するために必要
- [x] ~~**`tree_sitter_parser.rs:11-14` — `Parser::new()` + `set_language()` がファイルごとに呼ばれる。**~~ → `thread_local!` で `Parser` インスタンスを再利用済み
- [ ] **`oxc.rs:309-354` — `extract_leading_comment` がシンボルごとにソース先頭から `offset` まで逆方向スキャンする。** 1 ファイルに 100 シンボルあれば 100 回スキャン。各スキャンは O(file_size)。oxc の `ret.comments` からコメント位置を事前取得して二分探索で対応付けすべき
- [x] ~~**`index.rs:161-176` — `add_field_tokens` 内で `HashMap<&str, u16>` を毎フィールドで新規作成。**~~ → `build()` で1つの `HashMap` を作り `add_field_tokens` に渡して `clear()` + `drain()` で再利用済み

### メモリ

- [ ] **`InvertedIndex` — postings が `HashMap<String, Vec<Posting>>` で String キーの heap 確保が多い。** 数万シンボル規模では問題にならないが、10万超では `Vec<(TermId, Vec<Posting>)>` + 別途 `String → TermId` マップに分離した方がメモリ効率が良い
- [ ] **`field_lengths` — `u16` で表現。** 65535 トークンを超えるフィールド長が黙って切り捨てられる。大きな markdown ドキュメントの `content` フィールドで発生し得る。`u32` に変更するか、飽和加算 (`saturating_cast`) を明示すべき
- [ ] **`StoredDoc::Doc.content_preview` — 200文字の preview が全ドキュメントに保存される。** インデックスのシリアライズサイズを膨らませる。検索時にファイルから読み直す方式なら不要
- [ ] **`index.rs:47` — `InvertedIndex::build()` が `docs: Vec<IndexDocument>` を値渡しで受け取る。** 呼び出し元の `parse_directory` が `Vec` を構築 → `build` に move → index 構築中に `doc` の内容を clone して `StoredDoc` に格納。`&[IndexDocument]` のスライス参照で受け取ればドキュメント本体のコピーを減らせる

### I/O

- [x] ~~**`parse_directory` — `read_to_string` はファイル全体をメモリに載せる。**~~ → 10MB 上限チェック追加済み（セキュリティ > 信頼境界と同一修正）
- [ ] **`index_store::save_index` — `encode_to_vec` でバイト列全体をメモリに構築してから `write`。** 大きなインデックスでは `BufWriter` + streaming encode の方がメモリ効率が良い
- [x] ~~**`clone.rs:79` — `cmd.output()` は git の stdout/stderr を全てメモリにバッファする。**~~ → `stdout(Stdio::null())` + `stderr(Stdio::piped())` 追加済み

## Lint / コード品質

- [ ] `missing_docs` warnings を解消する (84件: repoask-core 82件 + repoask-parser 2件。struct field 37, variant 14, struct 9, module 10, function 7, method 8, const 5, enum 3, type alias 2, crate 1)
- [ ] `unnecessary qualification` warnings を解消する (3件, `cargo fix` で自動修正可能)
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
- [ ] **`parse_file_lenient` の戻り値型を統一。** repoask-parser は `Vec<IndexDocument>`、repoask-treesitter は `Option<Vec<IndexDocument>>`。シグネチャを揃える
- [ ] **`ParseProvider` trait を `repoask-core` に定義。** `fn parse_file(&self, filepath, source) -> Result<Option<Vec<IndexDocument>>, ParseError>`
- [ ] **`NativeParseProvider` を `repoask-repo` に実装。** 現在の `parse.rs` のディスパッチロジック (repoask-parser → repoask-treesitter fallback) を移植
- [ ] **`WasmParseProvider` を `repoask-wasm` に実装。** 現在は `repoask_parser::parse_file_lenient()` 直接呼び出し → trait 経由に変更。Step 2 で動的ロード機構の拡張点になる

### Step 2 以降（今は実装しない）

- [ ] Web 側に動的ロード機構を追加。tree-sitter grammar を個別 `.wasm` として配布、`registerGrammar(ext, wasmBytes)` で登録
- [ ] CLI 側の言語パーサーを feature flag で選択可能にする

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

- [ ] `BoostSignals` 型を `types.rs` に追加 (`module_boosts`, `directory_boosts`, `file_boosts`, `symbol_boosts` の4つの `HashMap`)
- [ ] `Normalization` enum (`Log1p` / `Saturate` / `None`) を追加
- [ ] `RankingConfig` struct を追加
- [ ] `InvertedIndex::search_with_boost()` を `index.rs` に実装。スコア計算: `bm25 * (1.0 + normalized(sum_of_boosts))`
- [ ] 既存の `search()` は変更なし (後方互換)
- [ ] WASM API に `searchWithBoost(query, limit, signals: JsValue)` を追加 (`serde-wasm-bindgen` で直接デシリアライズ)
- [ ] テスト: ブースト有無で順位が変わることを検証

### Step 2: GitData 型 + compute_boost_signals

- [ ] `GitData` 型を定義: `Commit` (id, parents, author, timestamp, message), `DiffEntry` (status, path, old_path, additions, deletions), `CommitDiff`, `BlameBlock`, `FileBlame`, `Tag`
- [ ] `DiffStatus` enum (`Added`, `Modified`, `Deleted`, `Renamed`, `Copied`)
- [ ] `compute_boost_signals(git_data, now_timestamp) -> BoostSignals` を repoask-core に実装
- [ ] 信号計算ロジック: 変更頻度 (log), 鮮度 (recency decay), 著者多様性, churn
- [ ] **一貫性保証**: この関数が全プラットフォームで共通であることが必須。CLI/Web/Server で同一の Rust コード

### Step 3 以降

- [ ] repoask-repo に git subprocess → `GitData` アダプタ (Layer 4 CLI)
- [ ] repoask-web (別 repo) に bit/API → `GitData` アダプタ (Layer 4 Web)
- [ ] グラフノード設計: call graph / import graph / co-change graph のエッジ定義
- [ ] centrality (PageRank 等) → `symbol_boosts` 変換

## モジュール / インターフェース

### Field 定数 (BM25 実装詳細) が types.rs に混在している

- [ ] **`repoask-core/src/types.rs:116-120` — `FIELD_SYMBOL_NAME`, `FIELD_DOC_CONTENT`, `FIELD_PARAMS`, `FIELD_FILEPATH`, `NUM_FIELDS` が `Symbol`, `DocSection` 等のドメイン型と同じファイルに定義されている。** これらは転置インデックスのフィールド ID で、BM25 の実装詳細。`types.rs` を変更する理由が「ドメイン型の変更」と「インデックスフィールドの変更」の2つになっており、変更理由の分離 (SRP) に反する。`index.rs` か `bm25.rs` に移動すべき。同様に `Posting`, `FieldStats`, `DocId`, `FieldId` もインデックス実装のための型なので `index.rs` に移動が妥当

### SearchResult::Example が IndexDocument に対応するバリアントを持たない

- [ ] **`repoask-core/src/types.rs:26-28` — `IndexDocument` は `Code | Doc` の2バリアント。`repoask-core/src/types.rs:57-61` — `SearchResult` は `Code | Doc | Example` の3バリアント。** `Example` は `repoask-core/src/index.rs:274-277` の `is_example_path()` でファイルパスに `"example"`, `"sample"`, `"demo"` が含まれるかのヒューリスティックで決定される。ドメイン的な区別ではなく表示上の区別が core 型に混入している。対処法は2つ: (A) `IndexDocument` にも `Example(Symbol)` バリアントを追加してパーサー段階で区別する、(B) `SearchResult` から `Example` を除去して `CodeResult` に `is_example: bool` フィールドを追加する。(B) の方がシンプル — `ExampleResult` と `CodeResult` は `is_example` 以外のフィールドが完全に同一なので、別バリアントにする正当性がない

## 型設計

- [ ] `SearchResult` の `score()` / `filepath()` をトレイト化するか検討 — 各バリアントで同じフィールドを持つが、共通アクセスが match 必須

## パーサー

- [ ] oxc: `export default function() {}` (無名デフォルトエクスポート) が `"default"` という名前になる — より良いフォールバック名を検討
- [ ] oxc: クラス内の static メソッド / getter / setter / property declaration を抽出していない
- [ ] oxc: `declare module` / `declare namespace` (ambient declarations) 未対応
- [ ] tree-sitter: Rust の `impl` ブロックのメソッド抽出クエリがネストした `impl` (trait impl) で正しく動くか未検証
- [ ] tree-sitter: Python の decorator 情報、Go の receiver type 情報を `Symbol` に含めていない
- [ ] tree-sitter: C/C++ の function declarator がネストしたポインタ宣言で名前抽出に失敗する可能性
- [ ] tree-sitter: Ruby の `module` が `SymbolKind::Class` として返される — 専用の `Module` kind を追加するか検討
- [ ] markdown: MDX (JSX in markdown) のコンポーネント記法を認識していない
- [ ] markdown: 空セクション(見出し直後に次の見出し) がゼロ内容の `DocSection` として生成される
- [ ] doc comment 抽出: Python の docstring (`"""..."""`) を tree-sitter で取得していない — AST の `expression_statement > string` ノードを処理する必要あり

## BM25 / 検索品質

- [ ] `doc_freq` の計算が `search()` 呼び出しごとに `HashSet` を生成している — 事前計算してキャッシュすべき
- [ ] `node_type_boost` (probe のランキング手法) を未実装 — `function > class > enum > const` の重み付け
- [ ] クエリの全トークンがヒットしたドキュメントを優先する `coverage_boost` を未実装
- [ ] ストップワード除去なし — "the", "a", "is" 等がインデックスを膨らませている

## repoask-repo / CLI

- [x] ~~`InvertedIndex` の bincode ファイル保存/読み込みロジック~~ → `index_store.rs` で実装済み
- [x] ~~`IndexMeta` (commit hash, format version, timestamp)~~ → `index_store.rs` で実装済み
- [x] ~~`repo.rs` の `load_or_build_index()` 内で `unwrap_or(Path::new(""))` を使用~~ → `if let Some(parent)` パターンに修正済み
- [ ] `clone.rs` の atomic rename (`std::fs::rename`) はクロスファイルシステムで失敗する — `/tmp` とホームディレクトリが別パーティションのケース
- [ ] `clone.rs` で clone 失敗時の tmp ディレクトリ cleanup が `let _ =` で無視されている
- [ ] `repo.rs` のファイルロック解放が `let _ = lock_file.unlock()` で明示的にされているが、`Drop` で十分 — 冗長コード
- [x] ~~CLI の `main.rs` で `eprintln!` を直接使っている~~ → `#![allow(clippy::print_stderr, reason = "CLI binary")]` で許可済み
- [ ] CLI に `--dir` / `--ext` / `--type` フィルタオプションが未実装 (仕様 S2)
- [ ] CLI に `extract` サブコマンドが未実装 (仕様 S1)
- [ ] CLI に `overview` サブコマンドが未実装 (仕様 S6)
- [ ] `search` の text 出力で snippet が80文字で切られるが、マルチバイト文字で壊れる可能性
- [ ] キャッシュの staleness チェックがコミットハッシュ一致のみ — 時間ベースの invalidation (`IndexMeta.is_stale()`) が未実装

## テスト

- [ ] `parse_directory()` の統合テストが未作成 — fixtures ディレクトリにサンプルリポジトリを置いて E2E 検証する
- [ ] tree-sitter の Java / C / C++ / Ruby のテストが未作成
- [ ] 大規模ファイル (10,000行超) でのパフォーマンステストなし
- [ ] BM25 のランキング品質テスト — 期待する順序で結果が返るか検証するテストが不足
- [ ] `index_store` の save/load ラウンドトリップテストが未作成
- [ ] `clone.rs` / `repo.rs` のテストは repo spec パースのみ — clone 自体のテストはネットワーク依存で未作成

## SKILL.md / Agent Skills

原則: **SKILL.md は必要最低限の文量に保つ。** agent のコンテキストウィンドウを圧迫しないことが最優先。agentが行動を起こすのに不要な説明・背景・Tipsは削るか、別ファイル (README等) に移す。

- [x] ~~`SKILL.md` 作成~~ → 実装済み

### 削減対象 (現在122行 → 目標60行以下)

- [ ] **「What this tool does」セクション削除。** agentは「いつ使うか」がわかれば十分。内部技術 (BM25等) の説明は不要
- [ ] **「When to use」を1-2行に圧縮。** 4つの箇条書きは冗長。「外部GitHubリポジトリのコード・ドキュメントを自然言語で検索するとき」の1文で十分
- [ ] **「Supported languages」セクション削除。** agentの行動判断に影響しない。拡張子の列挙はツール内部の話
- [ ] **「Usage patterns」セクションの3つの例を削除。** コマンド構文と出力フォーマットがあれば agent は自分で使い方を導出できる。例は1つあれば十分
- [ ] **「Read a specific code range after search」セクション削除。** `cat | sed -n` はagent環境依存。agentは `start_line`/`end_line` を見れば自分で読める
- [ ] **「Tips」セクション削除またはOutput formatに統合。** 「score は相対値」だけが有用、他はagentにとって自明
- [ ] **cleanup の説明を1行に圧縮。** 独立セクション不要
- [ ] **Output format の3つのJSON例を1つの統合例に。** 3バリアントの構造は JSON キー名 (`Code`/`Doc`/`Example`) とフィールド一覧の表で十分

### 不足情報の追記 (最小限で)

- [ ] エラー時の振る舞い: exit code 1 + stderr にエラーメッセージ (1行で記載)
- [ ] JSON lines: 各行が独立したJSONオブジェクト (Output format に1文追記)

## ハーネス改善

### similarity-rs 統合ロードマップ

similarity-rs による重複検出。pre-commit で `--fail-on-duplicates` で常時ブロック。

**現在**: threshold 0.96 (関数) / 0.99 (型)。pre-commit gate 有効。

**解消済み**:
- [x] ~~`oxc.rs` — `extract_from_statement` ↔ `extract_from_declaration` (98%)~~ → `Statement::as_declaration()` で委譲、`ExtractCtx` 構造体 + `ctx.push()` で Symbol 構築を統一
- [x] ~~`tokenizer.rs` — `tokenize_text` ↔ `tokenize_identifier` (97%)~~ → 共通の `stem_tokens()` 内部関数に抽出
- [x] ~~`ParseOutcome` 重複 (100%)~~ → `repoask-core::types` に移動、両パーサーが `pub use` で re-export

**残存 (threshold 引き下げ時に対処)**:
- [ ] **`CodeResult` ↔ `ExampleResult` (98%)。** フィールド完全一致。`ExampleResult` を除去して `CodeResult` に `is_example: bool` を追加する方針（モジュール/インターフェースセクション参照）。対処後 → 型 threshold 0.95 に引き下げ
- [ ] **`oxc.rs` — `extract_from_var_decl` ↔ `extract_class_methods` (95%)。** AST パターンマッチ + ctx.push の構造的類似。ドメイン的に別処理なので無理な共通化は避ける。対処後 → 関数 threshold 0.90 に引き下げ
- [ ] **`#[cfg(test)]` 内ヘルパー (`make_symbol`/`make_doc`, 94%)。** `--skip-test` が `test_` prefix / `#[test]` attr のみ対応で漏れる。similarity-rs の改善を待つか、テストヘルパーを別ファイルに切り出して `--exclude` する

**FP 対策**: `--exclude benches` で bench コードを除外済み。

**目標**: 段階的に threshold 0.80 まで引き下げ、より攻めた重複検出

### CI / CD

- [ ] **SHA pin の定期更新。** Dependabot or Renovate で `.github/workflows/*.yml` の action SHA を自動更新する PR を生成させる
- [ ] **`missing_docs` を `warn` → `deny` に昇格。** 公開APIのドキュメントが揃ったら（目安: 0.5.0）切り替える
- [ ] **coverage ratchet の実装。** `.metrics/coverage-baseline` ファイルを用意し、CI で coverage が下がったら fail するスクリプトを追加
- [ ] **CodSpeed 連携。** divan ベンチマークを `codspeed-divan-compat` に切り替えて PR コメントで回帰検出を自動化
- [ ] **cargo-mutants の全体実行。** weekly schedule で `cargo mutants --workspace` を走らせ、surviving mutants を定期検出

### flake.nix / 開発環境

- [ ] **direnv 連携。** `.envrc` に `use flake` を書いて `cd` 時に自動で devShell に入る
- [ ] **flake.lock の定期更新。** `nix flake update` を月1で実行し、nixpkgs のパッケージバージョンを追従

### テスト強化

- [ ] **insta snapshot を repoask-parser にも追加。** oxc の extract 結果、markdown の parse 結果をスナップショット
- [ ] **proptest: index serialize/deserialize roundtrip。** `deserialize(serialize(index)) == index` の不変条件
- [ ] **proptest: BM25 score 非負。** 任意の入力で `SearchResult.score() >= 0.0`
- [ ] **integration test: parse → index → search E2E。** `tests/fixtures/` にサンプルリポジトリを置いて、`parse_directory` → `InvertedIndex::build` → `search` の全フロー検証

### エラーハンドリング

- [ ] **repoask-core にもエラー型を追加する検討。** 現状はエラーパスがないが、`InvertedIndex::build` にバリデーション（空ドキュメント、不正な行番号）を入れるなら必要になる
- [ ] **`ParseOutcome` → `ParseReport` の集約をCLIに露出。** `--verbose` フラグでスキップ/失敗ファイルの一覧を表示

### ベンチマーク

- [ ] **serialization ベンチの追加。** bincode encode/decode の速度を計測（仕様M4「インデックスキャッシュ読み込み0秒」の検証）
- [ ] **大規模リポジトリでのE2Eベンチ。** 実際のOSSリポジトリ（e.g. vercel/next.js）でインデックス構築3秒以内、検索100ms以内を検証

### リリース

- [ ] **`release-plz.toml` の動作検証。** 初回リリース前にドライラン（`release-plz release --dry-run`）で CHANGELOG 生成を確認
- [ ] **crates.io のパッケージ名予約。** `repoask`, `repoask-core`, `repoask-parser`, `repoask-treesitter`, `repoask-repo` を公開して名前を確保
- [ ] **`panic = "abort"` の検討。** CLI バイナリのサイズ削減（~100KB）。`catch_unwind` を使っていないなら安全

## 未実装の crate (Step 4 以降)

- [x] ~~`repoask-wasm`: wasm-bindgen エントリポイント~~ → `RepoIndex` (addFile/build/search/docCount) 実装済み、wasm-pack build 成功
- [ ] `repoask-node`: napi-rs npm 配布
