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

```rust
#[wasm_bindgen]
impl RepoIndex {
    // 既存
    pub fn search(&self, query: &str, limit: usize) -> Result<String, JsError>;

    // 新規 — signals は JSON 文字列で受け取る
    #[wasm_bindgen(js_name = "searchWithBoost")]
    pub fn search_with_boost(
        &self,
        query: &str,
        limit: usize,
        signals_json: &str,
    ) -> Result<String, JsError>;
}
```

JS 側:
```js
const signals = {
  file_boosts: { "src/auth.ts": 2.0 },
  directory_boosts: { "src/auth": 1.5 },
  symbol_boosts: {},
  module_boosts: {},
};
const results = index.searchWithBoost("validate token", 10, JSON.stringify(signals));
```

JSON 文字列で渡す理由:
- `HashMap` は wasm-bindgen で直接渡せない
- `serde_json::from_str` で Rust 側でデシリアライズ
- WASM FFI boundary を超えるのにもっとも簡潔

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

## 実装ステップ

1. **Now**: `BoostSignals`, `Normalization`, `RankingConfig` を `types.rs` に追加。`search_with_boost()` を `index.rs` に実装。テスト
2. **When git 統合**: `repoask-repo` に `git log` → `file_boosts` 生成ロジック
3. **When call graph**: AST 解析で参照カウント → `symbol_boosts` 生成
4. **When bit upstream 修正**: WASM 版でも git boost が使える

## 参考

- Elasticsearch function_score: multiplicative boosting `final = bm25 * (1 + boost)`
- Sourcegraph/Zoekt: tiered additive model、file category / repo rank / match quality
- Tantivy: CustomSegmentScorer trait (repoask では WASM 互換性のため data-oriented に)
