# TODO

実装を前に進めるために置き去りにした項目。

## Lint / コード品質

- [ ] `missing_docs` warnings を解消する (99件: struct field 37, variant 14, struct 9, module 8, function 7, method 6, const 5, enum 3, type alias 2, crate 2)
- [ ] `unnecessary qualification` warnings を解消する (3件, `cargo fix` で自動修正可能)
- [ ] clippy をインストールして pedantic/nursery/restriction lint を通す (`Cargo.toml` に設定済みだが未実行)
- [ ] `expect_used` / `unwrap_used` が `deny` に設定された — `tree_sitter_parser.rs:13` の `expect()` を `ok_or()` + `?` に置き換える
- [ ] テストコード内の `unwrap()` / `panic!()` に `#[cfg(test)]` 用の lint 除外を追加する

## 型設計

- [ ] `IndexDocument` に `Example` バリアントを追加する検討 — 現在は `is_example_path()` でファイルパスから推測しているが、パーサー段階で `examples/` 配下かどうかを判定して型レベルで区別する方が堅実
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

## インデックス永続化

- [ ] `InvertedIndex` に `Serialize`/`Deserialize` derive はあるが、bincode でのファイル保存/読み込みロジックが未実装
- [ ] `IndexMeta` (commit hash, format version, timestamp) が未定義 — キャッシュ互換性チェックに必要

## テスト

- [ ] `parse_directory()` の統合テストが未作成 — fixtures ディレクトリにサンプルリポジトリを置いて E2E 検証する
- [ ] tree-sitter の Java / C / C++ / Ruby のテストが未作成
- [ ] 大規模ファイル (10,000行超) でのパフォーマンステストなし
- [ ] BM25 のランキング品質テスト — 期待する順序で結果が返るか検証するテストが不足

## 未実装の crate (Step 2 以降)

- [ ] `repoask-repo`: git clone, キャッシュ管理, cleanup
- [ ] `repoask-cli`: clap CLI, JSON lines 出力, search/extract/overview/cleanup サブコマンド
- [ ] `repoask-wasm`: wasm-bindgen エントリポイント
- [ ] `repoask-node`: napi-rs npm 配布
- [ ] `SKILL.md`: agent skill 定義ファイル
