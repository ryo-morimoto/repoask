# Boost Signals: 外部ランキング信号による検索品質向上

## アーキテクチャ (4層)

```
Layer 1: 階層的重み付けの抽象          (repoask-core)
Layer 2: module/dir/file/symbol の具体  (repoask-core)
Layer 3: git 抽象 → 重み計算           (repoask-signals? or repoask-core?)
Layer 4: git 具体 → git 抽象           (platform-specific)
```

### Layer 1: 階層的重み付けの抽象 (repoask-core)

「N 階層の木構造に沿ってスコアを伝播する」汎用エンジン。
git を知らない。AST を知らない。ノードに `f32` のブーストがあり、子は親を継承する、それだけ。

```rust
/// 階層パスをキーにしたブーストマップ。
/// パスの深さが階層を表す。子は親のブーストを加算で継承。
pub struct HierarchicalBoosts {
    boosts: HashMap<String, f32>,  // "module/dir/file/symbol" → boost
}

impl HierarchicalBoosts {
    /// あるノードの実効ブーストを計算。
    /// 祖先のブーストを全て加算。
    pub fn effective_boost(&self, path: &str) -> f32;
}
```

**やりすぎ？**: 4層以外の階層構造を使う具体的なユースケースが今はない。
Layer 1 と Layer 2 を分離する価値は「4層以外の階層が将来出てきたとき」。
今は Layer 1 + 2 を統合して、後で分離可能な設計にしておくのが妥当。

### Layer 2: module/dir/file/symbol の具体 (repoask-core)

4階層の意味を定義し、BM25 スコアと結合する。

```rust
pub struct BoostSignals {
    pub module_boosts: HashMap<String, f32>,
    pub directory_boosts: HashMap<String, f32>,
    pub file_boosts: HashMap<String, f32>,
    pub symbol_boosts: HashMap<DocId, f32>,
}

pub fn search_with_boost(
    &self, query: &str, limit: usize,
    signals: &BoostSignals, config: &RankingConfig,
) -> Vec<SearchResult>;
```

**やりすぎ？**: やりすぎではない。module/dir/file/symbol は
コード検索で普遍的な階層。repoask の全ターゲット (CLI/Web/Server) で使う。

### Layer 3: git 抽象 → 重み計算 (配置未決定)

git の導出データから BoostSignals を計算するロジック。
**全プラットフォームで共通のコード** でなければランキングが発散する。

配置候補:
- `repoask-core` に入れる → core が git 概念を知ることになる
- `repoask-signals` 新 crate → core と git 両方に依存しない中間層
- `repoask-repo` に入れる → WASM で使えない

**推奨: `repoask-core` に入れる。** 理由:
- git データ型は入力データでしかない。core が git コマンドを呼ぶわけではない
- 別 crate にすると WASM ビルドで追加の依存管理が必要
- `GitData → BoostSignals` は BM25 と同レベルの「スコアリングロジック」

### Layer 4: git 具体 → git 抽象 (platform-specific)

| プラットフォーム | git 具体 | 担当 crate |
|---|---|---|
| CLI | `git` subprocess | repoask-repo |
| Web | bit (sync) + API | repoask-web (別 repo) |
| Server | libgit2 / gitoxide | 組み込み先が実装 |

Layer 4 は repoask 本体の外にあってよい。
`GitData` 型を埋めるアダプタを書くだけ。

**やりすぎ？**: やりすぎではない。
プラットフォーム分離は既に存在する (repoask-repo vs repoask-wasm)。

---

## Git から導出可能なデータ一覧

git のプリミティブ (4 object + refs) から導出可能な全データと、
ランキングへの利用可能性:

### Commit object から

| 導出データ | 計算方法 | 付与先階層 | ランキングへの効果 |
|---|---|---|---|
| ファイル変更頻度 | `git log --follow <path>` のコミット数 | file | よく変更される = アクティブなコード |
| ファイル最終変更日 | `git log -1 --format=%at -- <path>` | file | 最近変更 = 関連性高い |
| churn (変動量) | `git log --numstat` の additions + deletions 合計 | file | 高 churn = 不安定 or 活発 |
| 著者多様性 | ファイルに触ったユニーク著者数 | file | 多い = 共有知識 or 重要コード |
| commit DAG (親子) | commit.parents | (グラフ構造用) | merge 頻度、ブランチ分岐度 |
| co-change (同時変更) | 同一 commit 内の変更ファイル群 | directory | 関連ファイル群の特定 |
| commit message パターン | `feat:` / `fix:` / `refactor:` 等 | file | fix が多い = バグが多い箇所 |

### Tree object から

| 導出データ | 計算方法 | 付与先階層 | ランキングへの効果 |
|---|---|---|---|
| ファイルサイズ | tree entry の blob サイズ | file | 大きすぎるファイルはペナルティ |
| ディレクトリ深度 | パスの `/` 数 | directory | 浅い = エントリポイント寄り |
| ファイル数密度 | ディレクトリ内のファイル数 | directory | 密度が高い = 重要なモジュール |

### Diff (tree 比較) から

| 導出データ | 計算方法 | 付与先階層 | ランキングへの効果 |
|---|---|---|---|
| rename 追跡 | `git log --follow -M` | file | ファイル同一性の維持 |
| 追加/削除行数 | `git diff --numstat` | file | churn の内訳 |
| hunk 範囲 | `git diff` の `@@` ヘッダ | symbol | 特定関数の変更頻度 |

### Blame から

| 導出データ | 計算方法 | 付与先階層 | ランキングへの効果 |
|---|---|---|---|
| 行単位の最終変更日 | `git blame --porcelain` | symbol | シンボル内の鮮度 |
| 行単位の著者 | blame の author フィールド | symbol | 著者集中度 (bus factor) |
| 行の年齢分布 | blame timestamp の分散 | symbol | 古い行ばかり = 安定 / 新しい行 = アクティブ |

### Refs / Tags から

| 導出データ | 計算方法 | 付与先階層 | ランキングへの効果 |
|---|---|---|---|
| リリース近接度 | tag から HEAD までの距離 | module | 最近リリース = アクティブ |
| ブランチ存在 | ref のパターン | (フィルタ用) | feature branch のコードは除外 |

---

## GitData 型 (Layer 3 の入力)

git のプリミティブに近い最小限のデータ型。
全ての導出データはこれらから計算可能。

```rust
/// Git の commit object に対応。
/// git cat-file commit <sha> の内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// SHA-1 (40 hex chars)
    pub id: String,
    /// 親 commit の SHA-1。マージコミットは複数。
    pub parents: Vec<String>,
    /// Author identity。git format: "Name <email>"
    pub author: String,
    /// Author timestamp (unix seconds)。
    /// commit date ではなく author date を使う (rebase で変わらない)。
    pub author_timestamp: u64,
    /// Commit message 全文。
    pub message: String,
}

/// 2つの tree 間の1ファイルの差分。
/// git diff-tree --numstat -M の1行に対応。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    /// 変更種別。git の diff status letter。
    pub status: DiffStatus,
    /// 変更後のファイルパス。
    pub path: String,
    /// rename/copy の場合、変更前のパス。
    pub old_path: Option<String>,
    /// 追加行数。binary なら None。
    pub additions: Option<u32>,
    /// 削除行数。binary なら None。
    pub deletions: Option<u32>,
}

/// git diff の status letter。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DiffStatus {
    Added,     // A
    Modified,  // M
    Deleted,   // D
    Renamed,   // R
    Copied,    // C
}

/// Commit とその変更ファイルの組。
/// git log --numstat --diff-filter=AMDRC -M の1コミット分。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDiff {
    pub commit: Commit,
    pub entries: Vec<DiffEntry>,
}

/// git blame --porcelain の1ブロック。
/// 同じ commit に属する連続行は1ブロック。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameBlock {
    /// 帰属先の commit SHA-1。
    pub commit_id: String,
    /// Author timestamp (unix seconds)。
    pub author_timestamp: u64,
    /// Author identity。
    pub author: String,
    /// 開始行 (1-based, 対象ファイル内)。
    pub start_line: u32,
    /// 行数。
    pub num_lines: u32,
}

/// 1ファイルの blame 結果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBlame {
    pub path: String,
    pub blocks: Vec<BlameBlock>,
}

/// Tag (annotated)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// タグ名 (e.g. "v2.0.0")。
    pub name: String,
    /// タグが指す commit SHA-1。
    pub target_commit: String,
    /// Tagger timestamp (unix seconds)。lightweight tag は None。
    pub timestamp: Option<u64>,
}

/// Layer 4 が埋める入力データ。
/// 全プラットフォーム共通。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitData {
    /// コミット履歴 + ファイル変更 (新しい順)。
    pub history: Vec<CommitDiff>,
    /// ファイルごとの blame (オプション、計算コスト高)。
    pub blame: Vec<FileBlame>,
    /// タグ一覧 (オプション)。
    pub tags: Vec<Tag>,
    /// HEAD の commit SHA-1。
    pub head: String,
}
```

### プラットフォーム対応表

| GitData フィールド | CLI (git subprocess) | Web (bit sync) | 備考 |
|---|---|---|---|
| `CommitDiff` | `git log --numstat -M --format=...` | `bit.log()` + 拡張必要 | bit の log は numstat を含まない |
| `BlameBlock` | `git blame --porcelain` | `bit blame` | bit は blame 実装済み |
| `Tag` | `git tag -l --format=...` | `bit.tagList()` | bit は lightweight のみ |
| `head` | `git rev-parse HEAD` | `bit.revParse(backend, root, "HEAD")` | |

---

## 未決定事項

### グラフのノード設計

BoostSignals は「スコアに信号を乗せる」だけ。
将来の graph ranking ではノード間の関係 (エッジ) が必要:

- **call graph**: function A → function B (呼び出し)
- **import graph**: file A → module B (依存)
- **type reference**: type A → type B (型参照)
- **co-change graph**: file A ↔ file B (同時変更の頻度)

これらのグラフ構造をどう持つか、graph 上の centrality (PageRank 等) を
どう BoostSignals に変換するかは、graph ranking の設計フェーズで決める。

### compute_boost_signals の具体ロジック

`GitData → BoostSignals` の変換で、各導出データをどの重みで結合するかは
チューニングが必要。初期値の例:

```
file_boost = 0.3 * ln(1 + change_count)       // 変更頻度
           + 0.4 * recency_decay(last_modified) // 鮮度 (30日半減)
           + 0.2 * ln(1 + unique_authors)       // 著者多様性
           + 0.1 * churn_factor(additions, deletions) // 変動量
```

最適な重みはベンチマークで測定する。ハードコードではなく `RankingConfig` で外から渡す。

---

## 実装ステップ

1. **Now**: `BoostSignals`, `RankingConfig`, `Normalization` → types.rs。`search_with_boost()` → index.rs。テスト
2. **Next**: `GitData` 型一式 → types.rs。`compute_boost_signals()` → 新モジュール
3. **When git 統合**: repoask-repo に git subprocess → `GitData` アダプタ
4. **When graph**: グラフノード設計、centrality → symbol_boosts 変換
5. **When bit 修正**: WASM 版でも bit から `GitData` 生成

## 参考

- git object format: blob, tree, commit, tag (4 primitives)
- git diff-tree --numstat -M: rename 検出付き numstat
- git blame --porcelain: ブロック単位の帰属情報
- Elasticsearch function_score: `final = bm25 * (1 + normalized_boost)`
- Sourcegraph/Zoekt: tiered additive model with file category / repo rank
- serde-wasm-bindgen: WASM FFI で JsValue → Rust struct 直接変換
