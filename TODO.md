# TODO

実装を前に進めるために置き去りにした項目。

## セキュリティ

### コマンドインジェクション / 引数インジェクション

- [ ] **`clone.rs:68` — owner/repo が git URL に直接埋め込まれる。** `owner = "--upload-pack=malicious"` のような値が `Command::arg()` 経由で渡される。`Command::arg()` はシェル展開しないので shell injection は防げるが、git の引数として解釈される可能性がある。owner/repo を `^[a-zA-Z0-9._-]+$` でバリデーションすべき
- [ ] **`clone.rs:73-75` — ref_spec が `--branch` に未検証で渡される。** `--` で始まる ref_spec は git 引数として解釈される可能性。先頭が `-` でないことをチェックすべき

### パストラバーサル

- [ ] **`cache.rs:27-28` — owner/repo がファイルパスに直接結合される。** `owner = "../../../etc"` でキャッシュディレクトリ外に脱出可能。`join()` 後のパスが `cache_dir()` 配下であることを `canonicalize()` でチェックすべき
- [ ] **`cache.rs:52-55` — `cleanup_all()` が `REPOASK_CACHE_DIR` 環境変数のパスを `remove_dir_all` する。** `REPOASK_CACHE_DIR=/` が設定されていた場合、ルートディレクトリ削除を試みる。最低限「repoask」を含むパスであることをチェックすべき

### 信頼境界

- [ ] **`index_store.rs:62-67` — キャッシュから bincode デシリアライズする際にサイズ制限がない。** 改ざんされた `index.bin` が数GB のメモリ確保を要求する可能性。`decode_from_slice` 前にファイルサイズの上限チェック (例: 500MB) を入れるべき
- [ ] **`index_store.rs:78-81` — `index.meta.json` の JSON デシリアライズに入力長制限がない。** 悪意のある JSON でメモリ枯渇の可能性（低リスクだが防御的に制限すべき）
- [ ] **`parse_directory` — ファイルサイズ制限なし。** 悪意のあるリポジトリに 10GB の `.ts` ファイルが含まれていた場合、`read_to_string` で OOM。ファイルサイズ上限 (例: 10MB) を設けるべき

### ディスク消費

- [ ] **キャッシュサイズ制限なし。** 多数のリポジトリを検索すると clone + index でディスクが無制限に消費される。LRU eviction や合計サイズ上限がない
- [ ] **`clone_fresh` の tmp ディレクトリが PID ベースの命名。** PID リサイクルで古い tmp が残っている場合、同じ名前で衝突する可能性。UUID を使うか、作成前にクリーンアップを確実にすべき（現状クリーンアップはあるが `remove_dir_all` の失敗は黙殺される）

## パフォーマンス

### 検索 (hot path)

- [ ] **`index.rs:194-198` — `doc_freq` が検索時に毎回 `HashSet` 生成で計算される。** postings リスト長ではなくユニークな doc_id 数が必要だが、これをインデックス構築時に事前計算して `HashMap<String, u32>` としてキャッシュすべき。現状は O(postings_per_term) の追加コスト
- [ ] **`index.rs:215-216` — 全スコア付きドキュメントをフルソートしてから truncate。** top-k 選択に `BinaryHeap` を使えば O(n log k) に改善。k=10, n=数万の場合に差が出る
- [ ] **`index.rs:187` — `HashMap<DocId, f32>` が検索ごとに新規生成される。** 構造体にキャッシュして `clear()` で再利用するか、事前確保すべき
- [ ] **`bm25.rs:25-27` — `weights[field_id as usize]` に bounds check が入る。** `field_id` は `u8` で定数定義されているが、不正な値で panic する。`get()` + デフォルト値に変更すべき

### インデックス構築

- [ ] **`tokenizer.rs:49,62` — `Stemmer::create()` が `tokenize_text` / `tokenize_identifier` 呼び出しごとに生成される。** Stemmer の内部構造は軽いが、数万回呼ばれると累積する。呼び出し元でインスタンスを使い回すか、`thread_local!` でキャッシュすべき
- [ ] **`parse_directory` — シングルスレッドでファイルを逐次処理。** `WalkBuilder::build_parallel()` + `crossbeam::channel` でファイルウォークとパースを並列化すれば、マルチコアで構築時間を大幅短縮可能。仕様の「3秒以内」を大規模リポジトリで達成するために必要
- [ ] **`tree_sitter_parser.rs:11-14` — `Parser::new()` + `set_language()` がファイルごとに呼ばれる。** tree-sitter の Parser はステートフルなので再利用可能。言語ごとに Parser をプールすべき
- [ ] **`oxc.rs:309-354` — `extract_leading_comment` がシンボルごとにソース先頭から `offset` まで逆方向スキャンする。** 1 ファイルに 100 シンボルあれば 100 回スキャン。各スキャンは O(file_size)。oxc の `ret.comments` からコメント位置を事前取得して二分探索で対応付けすべき
- [ ] **`index.rs:161-176` — `add_field_tokens` 内で `HashMap<&str, u16>` を毎フィールドで新規作成。** 構造体に持たせて `clear()` で再利用すべき

### メモリ

- [ ] **`InvertedIndex` — postings が `HashMap<String, Vec<Posting>>` で String キーの heap 確保が多い。** 数万シンボル規模では問題にならないが、10万超では `Vec<(TermId, Vec<Posting>)>` + 別途 `String → TermId` マップに分離した方がメモリ効率が良い
- [ ] **`field_lengths` — `u16` で表現。** 65535 トークンを超えるフィールド長が黙って切り捨てられる。大きな markdown ドキュメントの `content` フィールドで発生し得る。`u32` に変更するか、飽和加算 (`saturating_cast`) を明示すべき
- [ ] **`StoredDoc::Doc.content_preview` — 200文字の preview が全ドキュメントに保存される。** インデックスのシリアライズサイズを膨らませる。検索時にファイルから読み直す方式なら不要
- [ ] **`index.rs:47` — `InvertedIndex::build()` が `docs: Vec<IndexDocument>` を値渡しで受け取る。** 呼び出し元の `parse_directory` が `Vec` を構築 → `build` に move → index 構築中に `doc` の内容を clone して `StoredDoc` に格納。`&[IndexDocument]` のスライス参照で受け取ればドキュメント本体のコピーを減らせる

### I/O

- [ ] **`parse_directory` — `read_to_string` はファイル全体をメモリに載せる。** 大きなファイル (生成されたコードやバンドル) で問題。ファイルサイズチェック→スキップの方が安全
- [ ] **`index_store::save_index` — `encode_to_vec` でバイト列全体をメモリに構築してから `write`。** 大きなインデックスでは `BufWriter` + streaming encode の方がメモリ効率が良い
- [ ] **`clone.rs:79` — `cmd.output()` は git の stdout/stderr を全てメモリにバッファする。** 巨大リポジトリの clone で git が大量の progress 出力を生成した場合にメモリ消費が増える。`stdout(Stdio::null())` + `stderr(Stdio::piped())` にすべき

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

## repoask-repo / CLI

- [x] ~~`InvertedIndex` の bincode ファイル保存/読み込みロジック~~ → `index_store.rs` で実装済み
- [x] ~~`IndexMeta` (commit hash, format version, timestamp)~~ → `index_store.rs` で実装済み
- [ ] `repo.rs` の `load_or_build_index()` 内で `unwrap_or(Path::new(""))` を使用 — `parent()` が `None` になるケースの適切なハンドリング
- [ ] `clone.rs` の atomic rename (`std::fs::rename`) はクロスファイルシステムで失敗する — `/tmp` とホームディレクトリが別パーティションのケース
- [ ] `clone.rs` で clone 失敗時の tmp ディレクトリ cleanup が `let _ =` で無視されている
- [ ] `repo.rs` のファイルロック解放が `let _ = lock_file.unlock()` で明示的にされているが、`Drop` で十分 — 冗長コード
- [ ] CLI の `main.rs` で `eprintln!` を直接使っている — `print_stderr` lint が deny に設定されているため、`std::io::Write` 経由に統一すべき
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

## 未実装の crate (Step 3 以降)

- [ ] `repoask-wasm`: wasm-bindgen エントリポイント
- [ ] `repoask-node`: napi-rs npm 配布
- [ ] `SKILL.md`: agent skill 定義ファイル
