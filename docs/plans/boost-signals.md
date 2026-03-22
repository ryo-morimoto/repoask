# Boost Signals: 外部ランキング信号による検索品質向上

## 概要

BM25 スコアに外部信号 (git history, call graph 等) を乗せてランキングを改善する。
repoask-core は信号の出所を知らない。`HashMap` を受け取るだけ。

## 4階層モデル

```
module  (パッケージ / crate / npm package)
  └── directory
        └── file
              └── symbol  (AST: function, class, type, doc section)
```

各階層に `f32` のブースト値を付与できる。下位は上位の信号を加算で継承する。

## 想定される信号ソース

| 信号 | 階層 | ソース | 例 |
|---|---|---|---|
| リポジトリ人気度 | module | GitHub API (stars) | zod: 1.5 |
| 変更頻度 | file | git log --follow | よく変更されるファイル: 2.0 |
| 最終変更の新しさ | file | git log -1 | 1週間以内: 1.8 |
| テスト/生成コード判定 | directory | パスパターン | test/: -0.5 |
| 呼び出し元の数 | symbol | call graph (AST) | 50箇所から参照: 3.0 |
| export / public 可視性 | symbol | AST | pub fn: 1.2 |

## 型定義

```rust
/// 4階層の外部ランキング信号。
/// 全フィールドは Optional — 信号がない階層はスキップされる。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoostSignals {
    /// パッケージ/モジュール名 → boost
    pub module_boosts: HashMap<String, f32>,
    /// ディレクトリパス → boost (e.g. "src/auth")
    pub directory_boosts: HashMap<String, f32>,
    /// ファイルパス → boost (e.g. "src/auth/jwt.ts")
    pub file_boosts: HashMap<String, f32>,
    /// DocId → boost (インデックス内のドキュメントID)
    pub symbol_boosts: HashMap<DocId, f32>,
}
```

## 正規化

| 方式 | 式 | 特性 |
|---|---|---|
| **Log1p** (デフォルト) | `weight * ln(1 + raw)` | monotonic, bounded, 統計情報不要 |
| Saturate | `weight * raw / (raw + pivot)` | (0, weight) に収束、pivot で感度制御 |
| None | `weight * raw` | 線形、テスト用 |

```rust
pub enum Normalization {
    Log1p { weight: f32 },
    Saturate { weight: f32, pivot: f32 },
    None { weight: f32 },
}
```

## スコア計算

```
# 1. 各階層の信号を加算
combined_raw = module_boost + dir_boost + file_boost + symbol_boost
               (missing = 0.0)

# 2. 正規化
normalized = normalization.apply(combined_raw)

# 3. BM25 に乗算
final = bm25 * (1.0 + normalized)
```

加算 → 正規化 → 乗算。1階層だけ極端に高くても Log1p で抑制される。
`normalized = 0` のとき `final = bm25` (中立、後方互換)。

## API

### Rust (repoask-core)

```rust
impl InvertedIndex {
    // 既存 — 変更なし
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult>;

    // 新規
    pub fn search_with_boost(
        &self,
        query: &str,
        limit: usize,
        signals: &BoostSignals,
        config: &RankingConfig,
    ) -> Vec<SearchResult>;
}
```

### WASM (repoask-wasm)

`serde-wasm-bindgen` で `JsValue` を直接 `BoostSignals` にデシリアライズする。
JSON 文字列化は不要。

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl RepoIndex {
    // 既存
    pub fn search(&self, query: &str, limit: usize) -> Result<String, JsError>;

    // 新規 — signals は JsValue (プレーンオブジェクト) で受け取る
    #[wasm_bindgen(js_name = "searchWithBoost")]
    pub fn search_with_boost(
        &self,
        query: &str,
        limit: usize,
        signals: JsValue,
    ) -> Result<String, JsError> {
        let signals: BoostSignals = serde_wasm_bindgen::from_value(signals)
            .map_err(|e| JsError::new(&e.to_string()))?;
        // ...
    }
}
```

JS 側:
```js
const results = index.searchWithBoost("validate token", 10, {
  file_boosts: { "src/auth.ts": 2.0 },
  directory_boosts: { "src/auth": 1.5 },
  symbol_boosts: {},
  module_boosts: {},
});
```

`serde-wasm-bindgen` を使う理由:
- JS のプレーンオブジェクトをそのまま渡せる (`JSON.stringify` 不要)
- `HashMap` を含む Rust struct に直接デシリアライズ
- 既に `repoask-wasm` の依存に含まれている

## BoostSignals の合成

複数の信号ソースからの信号をマージできる:

```rust
impl BoostSignals {
    /// 別の信号ソースの値を加算マージする。
    pub fn merge(&mut self, other: &BoostSignals) {
        for (k, &v) in &other.module_boosts {
            *self.module_boosts.entry(k.clone()).or_default() += v;
        }
        // directory_boosts, file_boosts, symbol_boosts も同様
    }
}
```

使用例:
```rust
let mut signals = BoostSignals::default();
signals.merge(&git_history_signals);  // git log 由来
signals.merge(&call_graph_signals);   // AST 解析由来
let results = index.search_with_boost(query, 10, &signals, &config);
```

## 一貫性保証: 信号計算の共通化

### 問題

信号の「取得」はプラットフォームごとに異なる (git subprocess / bit / libgit2)。
信号の「計算」(生データ → BoostSignals) がプラットフォームごとにバラバラだと、
同じリポジトリ・同じクエリでもプラットフォームごとに検索順位が変わる。

### 解法

```
Platform-specific (diverge OK):           Shared (identical):
  CLI:    git subprocess ─┐
  Web:    bit             ├→ GitData ─→ compute_boost_signals() ─→ BoostSignals
  Server: libgit2         ┘               ↑ repoask-core (Rust, WASM 両方で動く)
```

repoask-core に:
1. git backend 非依存の **入力データ型** を定義
2. 入力データ → BoostSignals の **変換関数** を実装

各プラットフォームは git backend から入力データ型を埋めるだけ。

### Git データ型 (repoask-core)

git のオブジェクトモデルに基づく:

```rust
/// git commit object の要約。
/// git log --format="%H %at %ae %s" または bit の log() の出力に対応。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    /// Commit SHA-1 (40 hex chars)。git の commit object ID。
    pub id: String,
    /// Author email (git の author identity)。
    pub author_email: String,
    /// Author timestamp (unix seconds)。
    /// git は author date と committer date を区別するが、
    /// ブースト計算には author date を使う (実際の変更時刻)。
    pub author_timestamp: u64,
    /// Commit message の1行目。
    pub summary: String,
}

/// git log --numstat 由来のファイル単位の変更統計。
/// 1つの commit における1つのファイルの変更量を表す。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// 対象ファイルの相対パス (git の diff tree のパス)。
    pub path: String,
    /// 追加行数 (numstat の additions)。
    pub additions: u32,
    /// 削除行数 (numstat の deletions)。
    pub deletions: u32,
}

/// git log + numstat を組み合わせたコミット単位の変更記録。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitWithChanges {
    pub commit: CommitSummary,
    pub changes: Vec<FileChange>,
}

/// git blame --porcelain 由来の行単位の帰属情報。
/// blame は行ごとに「その行を最後に変更した commit」を返す。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    /// 元の commit の SHA-1。
    pub commit_id: String,
    /// Author timestamp (unix seconds)。
    pub author_timestamp: u64,
    /// 対象ファイルでの行番号 (1-based)。
    pub line_number: u32,
}

/// git blame の結果をファイル単位でまとめたもの。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBlame {
    pub path: String,
    pub lines: Vec<BlameLine>,
}

/// ブースト信号計算に必要な git データの全体。
/// 各プラットフォームがこれを埋めて compute_boost_signals() に渡す。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitData {
    /// リポジトリの最近のコミット履歴 (新しい順)。
    /// git log --numstat に相当。
    pub commits: Vec<CommitWithChanges>,
    /// ファイルごとの blame 情報 (オプション)。
    /// blame は計算コストが高いため、必要なファイルのみ提供可能。
    pub blame: Vec<FileBlame>,
}
```

### データ取得方法のプラットフォーム対応表

| フィールド | CLI (git subprocess) | Web (bit) | Server (libgit2) |
|---|---|---|---|
| `CommitSummary` | `git log --format="%H %at %ae %s"` | `bit.log(backend, root, N)` → `{id, author, message, timestamp}` | `Repository::log()` |
| `FileChange` | `git log --numstat --format=""` | 未実装 (diff 系 API から構築可能) | `Diff::stats()` |
| `BlameLine` | `git blame --porcelain` | `bit blame` (実装済み) | `Repository::blame_file()` |

### 変換関数 (repoask-core)

```rust
/// git データからブースト信号を計算する。
/// この関数は全プラットフォームで共通。repoask-core に実装。
pub fn compute_boost_signals(
    git_data: &GitData,
    now_timestamp: u64,
) -> BoostSignals {
    let mut signals = BoostSignals::default();

    // file_boosts: ファイルの変更頻度 + 新しさ
    let mut file_change_count: HashMap<String, u32> = HashMap::new();
    let mut file_last_modified: HashMap<String, u64> = HashMap::new();

    for commit in &git_data.commits {
        for change in &commit.changes {
            *file_change_count.entry(change.path.clone()).or_default() += 1;
            file_last_modified
                .entry(change.path.clone())
                .and_modify(|t| *t = (*t).max(commit.commit.author_timestamp))
                .or_insert(commit.commit.author_timestamp);
        }
    }

    for (path, count) in &file_change_count {
        let change_boost = (*count as f32).ln_1p(); // log(1 + count)
        let recency_boost = file_last_modified.get(path).map_or(0.0, |&t| {
            let age_days = (now_timestamp.saturating_sub(t)) as f32 / 86400.0;
            1.0 / (1.0 + age_days / 30.0) // 30日で半減
        });
        signals.file_boosts.insert(path.clone(), change_boost + recency_boost);
    }

    // directory_boosts: ディレクトリ内の変更密度
    // ... (file_boosts の集約)

    // symbol_boosts: blame ベースのシンボル鮮度
    // ... (blame lines とシンボル行範囲のオーバーラップ)

    signals
}
```

### WASM での利用

```js
// JS 側: bit (sync) または API で git データを取得
const gitData = {
  commits: bitLogEntries.map(entry => ({
    commit: {
      id: entry.id,
      author_email: parseEmail(entry.author),
      author_timestamp: entry.timestamp,
      summary: entry.message,
    },
    changes: [], // bit の log() には numstat がないため、将来の拡張
  })),
  blame: [],
};

// Rust WASM 側: 共通ロジックで BoostSignals を計算
const signals = index.computeBoostSignals(gitData);
const results = index.searchWithBoost("validate token", 10, signals);
```

`computeBoostSignals()` は Rust (repoask-core) で実装され、WASM でも native でも同一コード。
JS 側は `GitData` を埋める責務のみ。

## 実装ステップ

1. **Now**: `BoostSignals`, `Normalization`, `RankingConfig` を `types.rs` に追加。`search_with_boost()` を `index.rs` に実装。テスト
2. **Next**: `GitData` 型と `compute_boost_signals()` を repoask-core に追加
3. **When git 統合**: `repoask-repo` に git subprocess → `GitData` 変換ロジック
4. **When call graph**: AST 解析で参照カウント → `symbol_boosts` 生成
5. **When bit upstream 修正**: WASM 版でも bit から `GitData` を生成可能に

## 参考

- Elasticsearch function_score: multiplicative boosting `final = bm25 * (1 + boost)`
- Sourcegraph/Zoekt: tiered additive model、file category / repo rank / match quality
- Tantivy: CustomSegmentScorer trait (repoask では WASM 互換性のため data-oriented に)
- git log format: `%H` (commit hash), `%at` (author timestamp), `%ae` (author email), `%s` (summary)
- git blame --porcelain: `commit_id orig_line final_line` + `author-time` per block
- bit JS API: `log()` → `{id, author, message, timestamp}[]`, `diffIndex()` → `string[]`
